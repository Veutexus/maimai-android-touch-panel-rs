use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub image_path: String,
    pub serial: SerialConfig,
    pub android: AndroidConfig,
    pub detection: DetectionConfig,
    pub performance: PerformanceConfig,
    pub zone_colors: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct SerialConfig {
    pub port: String,
    pub baudrate: u32,
}

#[derive(Debug, Deserialize)]
pub struct AndroidConfig {
    pub max_slot: usize,
    pub monitor_size: [u32; 2],
    pub input_size: [u32; 2],
    pub reverse_monitor: bool,
    pub specified_device: String,
}

#[derive(Debug, Deserialize)]
pub struct DetectionConfig {
    pub area_scope: u32,
    pub area_point_num: usize,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceConfig {
    pub sleep_mode: bool,
    pub sleep_delay_us: u64,
    pub time_compensation: f64,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config TOML")?;
        Ok(config)
    }
}
