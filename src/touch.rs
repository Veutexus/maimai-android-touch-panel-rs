use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::serial_manager::SerialManager;
use crate::zone::ZoneLookup;

struct TouchSlot {
    pressed: bool,
    x: f64,
    y: f64,
}

impl Default for TouchSlot {
    fn default() -> Self {
        Self {
            pressed: false,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// Shared ADB process ID so main thread can kill it on exit.
pub static ADB_PID: AtomicU32 = AtomicU32::new(0);

/// Kill the ADB child process if it's still running.
pub fn kill_adb() {
    let pid = ADB_PID.load(Ordering::Relaxed);
    if pid != 0 {
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        #[cfg(not(windows))]
        {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        ADB_PID.store(0, Ordering::Relaxed);
    }
}

fn spawn_adb(specified_device: &str) -> Option<Child> {
    let mut adb_args: Vec<&str> = vec![];
    if !specified_device.is_empty() {
        adb_args.extend_from_slice(&["-s", specified_device]);
    }
    adb_args.extend_from_slice(&["shell", "getevent", "-l"]);

    // Match Python: subprocess.Popen(adb_cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    let result = Command::new("adb")
        .args(&adb_args)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn();

    match result {
        Ok(child) => {
            ADB_PID.store(child.id(), Ordering::Relaxed);
            Some(child)
        }
        Err(e) => {
            eprintln!("Failed to start adb: {}. Make sure adb is installed and in PATH.", e);
            None
        }
    }
}

/// Spawns `adb shell getevent -l` and parses multi-touch Type B events.
/// Converts touch positions to zones and sends updates to the serial manager.
pub fn run_getevent(
    serial_manager: &SerialManager,
    zone_lookup: &ZoneLookup,
    max_slot: usize,
    monitor_size: [u32; 2],
    input_size: [u32; 2],
    reverse_monitor: Arc<AtomicBool>,
    specified_device: &str,
) {
    let abs_multi_x = monitor_size[0] as f64 / input_size[0] as f64;
    let abs_multi_y = monitor_size[1] as f64 / input_size[1] as f64;

    let mut touch_data: Vec<TouchSlot> = (0..max_slot).map(|_| TouchSlot::default()).collect();
    let mut touch_index: usize = 0;
    let mut key_is_changed = false;

    let mut child = match spawn_adb(specified_device) {
        Some(c) => c,
        None => return,
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            eprintln!("Failed to capture adb stdout");
            return;
        }
    };

    let reader = BufReader::new(stdout);

    println!("ADB getevent started (PID {}), waiting for touch events...", child.id());

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            if line.contains("name") {
                println!("{}", line);
            }
            continue;
        }

        let event_type = parts[2];
        let event_value_hex = parts[3];

        let event_value = match i64::from_str_radix(event_value_hex, 16) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match event_type {
            "ABS_MT_POSITION_X" => {
                key_is_changed = true;
                let reverse = reverse_monitor.load(Ordering::Relaxed);
                if touch_index < touch_data.len() {
                    touch_data[touch_index].x = if reverse {
                        monitor_size[0] as f64 - event_value as f64 * abs_multi_x
                    } else {
                        event_value as f64 * abs_multi_x
                    };
                }
            }
            "ABS_MT_POSITION_Y" => {
                key_is_changed = true;
                let reverse = reverse_monitor.load(Ordering::Relaxed);
                if touch_index < touch_data.len() {
                    touch_data[touch_index].y = if reverse {
                        monitor_size[1] as f64 - event_value as f64 * abs_multi_y
                    } else {
                        event_value as f64 * abs_multi_y
                    };
                }
            }
            "SYN_REPORT" => {
                if key_is_changed {
                    convert(&touch_data, serial_manager, zone_lookup);
                    key_is_changed = false;
                }
            }
            "ABS_MT_SLOT" => {
                key_is_changed = true;
                touch_index = event_value as usize;
            }
            "ABS_MT_TRACKING_ID" => {
                key_is_changed = true;
                if touch_index < touch_data.len() {
                    if event_value_hex == "ffffffff" {
                        touch_data[touch_index].pressed = false;
                    } else {
                        touch_data[touch_index].pressed = true;
                    }
                }
            }
            _ => {}
        }
    }

    // Clean up
    let _ = child.kill();
    let _ = child.wait();
    ADB_PID.store(0, Ordering::Relaxed);
    eprintln!("ADB getevent ended. Is the device still connected?");
}

/// Maps current touch positions to zones and sends the packet to serial manager.
fn convert(touch_data: &[TouchSlot], serial_manager: &SerialManager, zone_lookup: &ZoneLookup) {
    let mut all_zones = std::collections::HashSet::new();

    for slot in touch_data {
        if !slot.pressed {
            continue;
        }
        let zones = zone_lookup.lookup_zones(slot.x as i32, slot.y as i32);
        all_zones.extend(zones);
    }

    let touch_keys: Vec<String> = all_zones.iter().cloned().collect();
    let grid = ZoneLookup::zones_to_grid(&all_zones);
    serial_manager.change_touch(&grid, touch_keys);
}
