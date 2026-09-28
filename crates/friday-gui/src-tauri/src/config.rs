use std::fs;
use std::path::PathBuf;
use crate::types::SettingsDto;

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

    pub fn load() -> SettingsDto {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str::<SettingsDto>(&content) {
                    return settings;
                }
            }
        }
        SettingsDto::default()
    }

    pub fn save(settings: &SettingsDto) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Failed to write config file to {:?}: {}", path, e))?;
        Ok(())
    }
}
