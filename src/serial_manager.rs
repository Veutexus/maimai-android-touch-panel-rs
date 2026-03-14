use anyhow::{Context, Result};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::PerformanceConfig;
use crate::zone::ZoneLookup;

/// Touch update: the 9-byte packet and the list of touched zone names (for display).
pub type TouchUpdate = (Vec<u8>, Vec<String>);

/// Shared state protected by a Mutex for the write thread.
struct SharedState {
    current_packet: Vec<u8>,
    started: bool,
}

pub struct SerialManager {
    tx: Sender<TouchUpdate>,
    shared: Arc<Mutex<SharedState>>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl SerialManager {
    pub fn new(port_name: &str, baudrate: u32, perf: &PerformanceConfig) -> Result<Self> {
        let port = serialport::new(port_name, baudrate)
            .timeout(Duration::from_millis(100))
            .open()
            .with_context(|| format!("Failed to open serial port {}", port_name))?;

        let (tx, rx): (Sender<TouchUpdate>, Receiver<TouchUpdate>) = mpsc::channel();

        let exit_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let shared = Arc::new(Mutex::new(SharedState {
            current_packet: Self::build_empty_packet(),
            started: false,
        }));

        // Match Python's ping_touch_thread(): send initial empty packet
        let _ = tx.send((Self::build_empty_packet(), vec![]));

        // Spawn touch thread
        let touch_port = port.try_clone().context("Failed to clone serial port")?;
        let touch_shared = Arc::clone(&shared);
        let touch_exit = Arc::clone(&exit_flag);
        let sleep_mode = perf.sleep_mode;
        let sleep_delay_us = perf.sleep_delay_us;
        let time_compensation = perf.time_compensation;
        thread::spawn(move || {
            Self::touch_thread(
                touch_port,
                rx,
                touch_shared,
                touch_exit,
                sleep_mode,
                sleep_delay_us,
                time_compensation,
            );
        });

        // Spawn write thread
        let write_port = port;
        let write_shared = Arc::clone(&shared);
        let write_exit = Arc::clone(&exit_flag);
        thread::spawn(move || {
            Self::write_thread(write_port, write_shared, write_exit);
        });

        println!("Listening on serial port {}...", port_name);

        Ok(Self {
            tx,
            shared,
            exit_flag,
        })
    }

    /// Queue a touch update from the getevent thread.
    pub fn change_touch(&self, grid: &[Vec<u8>], touch_keys: Vec<String>) {
        let packet = Self::build_touch_packet(grid);
        let _ = self.tx.send((packet, touch_keys));
    }

    /// Force the started flag (used by the `start` console command).
    pub fn set_started(&self, started: bool) {
        if let Ok(mut state) = self.shared.lock() {
            state.started = started;
        }
    }

    pub fn stop(&self) {
        println!("Stopping...");
        self.exit_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        thread::sleep(Duration::from_millis(100));
        println!("Stopped.");
    }

    /// Build the 9-byte touch packet: 0x28 + 7 bit-packed bytes + 0x29
    fn build_touch_packet(grid: &[Vec<u8>]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(9);
        packet.push(0x28);
        for group in grid.iter().take(7) {
            let mut byte_val: u8 = 0;
            for (j, &val) in group.iter().enumerate() {
                if val == 1 {
                    byte_val += 1 << j;
                }
            }
            packet.push(byte_val);
        }
        // Pad if fewer than 7 groups
        while packet.len() < 8 {
            packet.push(0);
        }
        packet.push(0x29);
        packet
    }

    fn build_empty_packet() -> Vec<u8> {
        let grid = ZoneLookup::zones_to_grid(&std::collections::HashSet::new());
        Self::build_touch_packet(&grid)
    }

    fn touch_thread(
        mut port: Box<dyn serialport::SerialPort>,
        rx: Receiver<TouchUpdate>,
        shared: Arc<Mutex<SharedState>>,
        exit_flag: Arc<std::sync::atomic::AtomicBool>,
        sleep_mode: bool,
        sleep_delay_us: u64,
        time_compensation: f64,
    ) {
        let mut read_buf = [0u8; 6];
        let mut setting_packet = vec![40, 0, 0, 0, 0, 41]; // 0x28, 0, 0, 0, 0, 0x29

        while !exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
            // Check for handshake data (Python: if ser.in_waiting == 6)
            if let Ok(bytes_available) = port.bytes_to_read() {
                if bytes_available == 6 {
                    if let Ok(6) = port.read(&mut read_buf) {
                        let byte3 = read_buf[3];
                        if byte3 == 76 || byte3 == 69 {
                            // Disconnected
                            if let Ok(mut state) = shared.lock() {
                                state.started = false;
                            }
                        } else if byte3 == 114 || byte3 == 107 {
                            // Settings exchange — echo back
                            for i in 1..5 {
                                setting_packet[i] = read_buf[i];
                            }
                            let _ = port.write(&setting_packet);
                        } else if byte3 == 65 {
                            // Connected
                            if let Ok(mut state) = shared.lock() {
                                state.started = true;
                            }
                            println!("Connected to game");
                        }
                    }
                }
            }

            // Process queued touch updates
            if let Ok(update) = rx.try_recv() {
                if let Ok(mut state) = shared.lock() {
                    state.current_packet = update.0.clone();
                    let _ = port.write(&update.0);
                }
                if !update.1.is_empty() {
                    println!("Touch Keys: {:?}", update.1);
                }
            }

            if sleep_mode {
                microsecond_sleep(sleep_delay_us, time_compensation);
            }
        }
    }

    fn write_thread(
        mut port: Box<dyn serialport::SerialPort>,
        shared: Arc<Mutex<SharedState>>,
        exit_flag: Arc<std::sync::atomic::AtomicBool>,
    ) {
        while !exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
            thread::sleep(Duration::from_micros(1));
            let data = {
                let state = match shared.lock() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if !state.started {
                    continue;
                }
                state.current_packet.clone()
            };
            let _ = port.write(&data);
        }
    }
}

/// Busy-wait microsecond sleep
fn microsecond_sleep(delay_us: u64, time_compensation: f64) {
    let adjusted = (delay_us as f64 - time_compensation) / 1_000_000.0;
    if adjusted <= 0.0 {
        return;
    }
    let end = Instant::now() + Duration::from_secs_f64(adjusted);
    while Instant::now() < end {
        std::hint::spin_loop();
    }
}
