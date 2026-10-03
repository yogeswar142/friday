use crate::protocol::DeviceCapabilities;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::info;

/// Record of a trusted, paired device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub display_name: String,
    pub auth_token: String,
    pub last_known_ip: String,
    pub last_known_port: u16,
    pub paired_at: u64,
    pub capabilities: DeviceCapabilities,
    /// Pinned Noise static public key (base64-encoded Curve25519 public key).
    /// Set after the first successful Noise_XX handshake.
    /// If `Some`, future connections MUST present the same key.
    #[serde(default)]
    pub noise_static_pubkey_b64: Option<String>,
}

/// Persistent store of trusted devices
#[derive(Debug, Clone)]
pub struct TrustStore {
    config_dir: PathBuf,
    devices: Arc<RwLock<HashMap<String, TrustedDevice>>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct TrustStoreFile {
    pub trusted_devices: Vec<TrustedDevice>,
}

impl TrustStore {
    pub fn new(config_dir: Option<&Path>) -> Self {
        let dir = config_dir.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("friday")
        });
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }

        let store = Self {
            config_dir: dir,
            devices: Arc::new(RwLock::new(HashMap::new())),
        };
        store.load();
        store
    }

    fn file_path(&self) -> PathBuf {
        self.config_dir.join("trust_store.json")
    }

    /// Load trusted devices from file
    pub fn load(&self) {
        let path = self.file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(file_data) = serde_json::from_str::<TrustStoreFile>(&content) {
                    if let Ok(mut map) = self.devices.write() {
                        map.clear();
                        for d in file_data.trusted_devices {
                            map.insert(d.device_id.clone(), d);
                        }
                    }
                }
            }
        }
    }

    /// Save trusted devices to file
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = self.file_path();
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let devices: Vec<TrustedDevice> = {
            let map = self.devices.read().unwrap();
            map.values().cloned().collect()
        };
        let file_data = TrustStoreFile {
            trusted_devices: devices,
        };
        let json = serde_json::to_string_pretty(&file_data)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Add or update a trusted device
    pub fn add_trusted(&self, device: TrustedDevice) -> Result<(), std::io::Error> {
        {
            let mut map = self.devices.write().unwrap();
            info!(
                "Adding device to trust store: {} ('{}')",
                device.device_id, device.display_name
            );
            map.insert(device.device_id.clone(), device);
        }
        self.save()
    }

    /// Remove a device from trust store (Forget / Unpair)
    pub fn remove_trusted(&self, device_id: &str) -> Result<bool, std::io::Error> {
        let removed = {
            let mut map = self.devices.write().unwrap();
            map.remove(device_id).is_some()
        };
        if removed {
            info!("Removed device from trust store: {}", device_id);
            self.save()?;
        }
        Ok(removed)
    }

    /// Check if a device ID is in the trust store
    pub fn is_trusted(&self, device_id: &str) -> bool {
        let map = self.devices.read().unwrap();
        map.contains_key(device_id)
    }

    /// Get a trusted device by device_id
    pub fn get_trusted(&self, device_id: &str) -> Option<TrustedDevice> {
        let map = self.devices.read().unwrap();
        map.get(device_id).cloned()
    }

    /// Verify an authentication token against a trusted device
    pub fn verify_token(&self, device_id: &str, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        let map = self.devices.read().unwrap();
        if let Some(dev) = map.get(device_id) {
            dev.auth_token == token
        } else {
            false
        }
    }

    /// Update the last known IP/port of a trusted device (e.g. after DHCP change)
    pub fn update_endpoint(&self, device_id: &str, ip: &str, port: u16) -> bool {
        let mut map = self.devices.write().unwrap();
        if let Some(dev) = map.get_mut(device_id) {
            dev.last_known_ip = ip.to_string();
            dev.last_known_port = port;
            true
        } else {
            false
        }
    }

    /// List all trusted devices
    pub fn list_trusted(&self) -> Vec<TrustedDevice> {
        let map = self.devices.read().unwrap();
        map.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_trust_store_persistence_and_auth() {
        let unique_dir = std::env::temp_dir().join(format!("friday_trust_test_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&unique_dir);

        let store = TrustStore::new(Some(&unique_dir));
        assert!(store.list_trusted().is_empty());

        let dev = TrustedDevice {
            device_id: "dev-123".to_string(),
            display_name: "Yoga 9i".to_string(),
            auth_token: "secret_token_abc".to_string(),
            last_known_ip: "192.168.1.50".to_string(),
            last_known_port: 48700,
            paired_at: 1000,
            capabilities: DeviceCapabilities::default(),
            noise_static_pubkey_b64: None,
        };

        store.add_trusted(dev.clone()).unwrap();
        assert!(store.is_trusted("dev-123"));
        assert!(!store.is_trusted("dev-456"));
        assert!(store.verify_token("dev-123", "secret_token_abc"));
        assert!(!store.verify_token("dev-123", "wrong_token"));

        // Re-load in a separate instance from disk
        let store2 = TrustStore::new(Some(&unique_dir));
        assert!(store2.is_trusted("dev-123"));
        let retrieved = store2.get_trusted("dev-123").unwrap();
        assert_eq!(retrieved.display_name, "Yoga 9i");
        assert_eq!(retrieved.auth_token, "secret_token_abc");

        // Remove device
        assert!(store2.remove_trusted("dev-123").unwrap());
        assert!(!store2.is_trusted("dev-123"));

        let _ = fs::remove_dir_all(&unique_dir);
    }
}
