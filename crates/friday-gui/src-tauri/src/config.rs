use crate::types::{DeviceInfo, SettingsDto};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub settings: SettingsDto,
    #[serde(default)]
    pub paired_devices: Vec<DeviceInfo>,
    #[serde(default)]
    pub ring_topology: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            settings: SettingsDto::default(),
            paired_devices: Vec::new(),
            ring_topology: Vec::new(),
        }
    }
}

pub struct ConfigManager;

impl ConfigManager {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("friday");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir.join("config.json")
    }

    pub fn load_config() -> AppConfig {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
                // Backwards-compatible load if older file contained only SettingsDto
                if let Ok(settings) = serde_json::from_str::<SettingsDto>(&content) {
                    return AppConfig {
                        settings,
                        paired_devices: Vec::new(),
                        ring_topology: Vec::new(),
                    };
                }
            }
        }
        AppConfig::default()
    }

    pub fn save_config(config: &AppConfig) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Failed to write config file to {:?}: {}", path, e))?;
        Ok(())
    }

    pub fn load() -> SettingsDto {
        Self::load_config().settings
    }

    pub fn save(settings: &SettingsDto) -> Result<(), String> {
        let mut config = Self::load_config();
        config.settings = settings.clone();
        Self::save_config(&config)
    }
}
