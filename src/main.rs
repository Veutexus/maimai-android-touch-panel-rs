mod config;
mod serial_manager;
mod touch;
mod zone;

use anyhow::{Context, Result};
use image::ImageReader;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    let config = config::Config::load(Path::new(&config_path))?;

    let abs_multi_x = config.android.monitor_size[0] as f64 / config.android.input_size[0] as f64;
    let abs_multi_y = config.android.monitor_size[1] as f64 / config.android.input_size[1] as f64;

    println!("Config loaded from: {}", config_path);
    println!("Serial port: {}", config.serial.port);
    println!("Touch area X multiplier: {}", abs_multi_x);
    println!("Touch area Y multiplier: {}", abs_multi_y);
    println!(
        "Screen reverse: {}",
        if config.android.reverse_monitor {
            "enabled"
        } else {
            "disabled"
        }
    );

    // Load overlay image
    let img = ImageReader::open(&config.image_path)
        .with_context(|| format!("Failed to open image: {}", config.image_path))?
        .decode()
        .with_context(|| "Failed to decode image")?
        .to_rgb8();

    // Initialize zone lookup
    let zone_lookup = zone::ZoneLookup::new(
        img,
        config.zone_colors,
        config.detection.area_scope,
        config.detection.area_point_num,
    );

    // Start ADB server
    println!("Starting ADB server...");
    let _ = Command::new("adb")
        .arg("start-server")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status();
    println!("ADB server ready.");

    // Initialize serial manager
    let serial_manager = serial_manager::SerialManager::new(
        &config.serial.port,
        config.serial.baudrate,
        &config.performance,
    )?;

    // Shared reverse_monitor toggle
    let reverse_monitor = Arc::new(AtomicBool::new(config.android.reverse_monitor));

    // Spawn getevent thread
    let getevent_reverse = Arc::clone(&reverse_monitor);
    let max_slot = config.android.max_slot;
    let monitor_size = config.android.monitor_size;
    let input_size = config.android.input_size;
    let specified_device = config.android.specified_device.clone();

    // Leak zone_lookup and serial_manager into 'static references for the spawned thread.
    // They live for the entire program lifetime anyway.
    let zone_lookup: &'static zone::ZoneLookup = Box::leak(Box::new(zone_lookup));
    let serial_manager: &'static serial_manager::SerialManager =
        Box::leak(Box::new(serial_manager));

    // Handle Ctrl+C: kill ADB and serial before exiting
    ctrlc::set_handler(move || {
        eprintln!("\nCtrl+C received, cleaning up...");
        touch::kill_adb();
        std::process::exit(0);
    })
    .ok();

    thread::spawn(move || {
        touch::run_getevent(
            serial_manager,
            zone_lookup,
            max_slot,
            monitor_size,
            input_size,
            getevent_reverse,
            &specified_device,
        );
    });

    // Interactive console
    let stdin = io::stdin();
    let reader = stdin.lock();
    print!("> ");
    io::stdout().flush().ok();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let cmd = line.trim();

        match cmd {
            "" => {}
            "start" => {
                serial_manager.set_started(true);
                println!("Connected to game");
            }
            "reverse" => {
                let prev = reverse_monitor.load(Ordering::Relaxed);
                reverse_monitor.store(!prev, Ordering::Relaxed);
                println!(
                    "Screen reverse: {}",
                    if !prev { "enabled" } else { "disabled" }
                );
            }
            "restart" => {
                println!("Restarting...");
                touch::kill_adb();
                serial_manager.stop();
                std::process::exit(42);
            }
            "exit" => {
                println!("Exiting...");
                touch::kill_adb();
                serial_manager.stop();
                std::process::exit(0);
            }
            _ => {
                println!("Unknown command. Available: start, reverse, restart, exit");
            }
        }

        print!("> ");
        io::stdout().flush().ok();
    }

    Ok(())
}
