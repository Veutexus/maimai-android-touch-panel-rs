//! Simplified standalone example: parses ADB touch events and prints detected zones.
//! Uses single-pixel red-channel grayscale lookup (no circular sampling, no serial).
//!
//! Usage: cargo run --example getevent

use image::ImageReader;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Instant;

const MAX_SLOT: usize = 12;

fn main() {
    let img = ImageReader::open("./image/image_monitor.png")
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image")
        .to_rgb8();

    let (img_w, img_h) = (img.width() as i32, img.height() as i32);

    // Red-channel grayscale lookup (simplified mapping from example/getevent.py)
    let exp_image_dict: HashMap<&str, &str> = HashMap::from([
        ("61", "A1"),
        ("65", "A2"),
        ("71", "A3"),
        ("75", "A4"),
        ("81", "A5"),
        ("85", "A6"),
        ("91", "A7"),
        ("95", "A8"),
        ("101", "B1"),
        ("105", "B2"),
        ("111", "B3"),
        ("115", "B4"),
        ("121", "B5"),
        ("125", "B6"),
        ("130", "B7"),
        ("135", "B8"),
        ("140", "C1"),
        ("145", "C2"),
        ("150", "D1"),
        ("155", "D2"),
        ("160", "D3"),
        ("165", "D4"),
        ("170", "D5"),
        ("175", "D6"),
        ("180", "D7"),
        ("185", "D8"),
        ("190", "E1"),
        ("195", "E2"),
        ("200", "E3"),
        ("205", "E4"),
        ("210", "E5"),
        ("215", "E6"),
        ("220", "E7"),
        ("225", "E8"),
    ]);

    struct TouchSlot {
        pressed: bool,
        x: i32,
        y: i32,
    }

    let mut touch_data: Vec<TouchSlot> = (0..MAX_SLOT)
        .map(|_| TouchSlot {
            pressed: false,
            x: 0,
            y: 0,
        })
        .collect();
    let mut touch_index: usize = 0;

    let mut child = Command::new("adb")
        .args(["shell", "getevent", "-l"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start adb");

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
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
                if touch_index < touch_data.len() {
                    touch_data[touch_index].x = event_value as i32;
                }
            }
            "ABS_MT_POSITION_Y" => {
                if touch_index < touch_data.len() {
                    touch_data[touch_index].y = event_value as i32;
                }
            }
            "SYN_REPORT" => {
                let start = Instant::now();
                let mut touch_keys = Vec::new();
                for slot in &touch_data {
                    if !slot.pressed {
                        continue;
                    }
                    let x = slot.x;
                    let y = slot.y;
                    if x >= 0 && x < img_w && y >= 0 && y < img_h {
                        let pixel = img.get_pixel(x as u32, y as u32);
                        let r_str = pixel[0].to_string();
                        if let Some(&zone) = exp_image_dict.get(r_str.as_str()) {
                            touch_keys.push(zone);
                        }
                    } else {
                        println!("Coordinates ({}, {}) are out of image bounds.", x, y);
                    }
                }
                let elapsed = start.elapsed();
                println!("Touch Keys: {:?}", touch_keys);
                println!("Execution time: {:.6}s", elapsed.as_secs_f64());
            }
            "ABS_MT_SLOT" => {
                touch_index = event_value as usize;
            }
            "ABS_MT_TRACKING_ID" => {
                if touch_index < touch_data.len() {
                    touch_data[touch_index].pressed = event_value_hex != "ffffffff";
                }
            }
            _ => {}
        }
    }
}
