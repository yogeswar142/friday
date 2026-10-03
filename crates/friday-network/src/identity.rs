use crate::protocol::DeviceCapabilities;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use uuid::Uuid;

pub const DEFAULT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Stable device identity that persists across reboots, IP changes, and DHCP reassignments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceIdentity {
    /// Persistent UUIDv4 identifying this physical installation
    pub device_id: String,
    /// User-configurable human-friendly name (e.g. "Yoga", "G50", "MacBook")
    pub display_name: String,
    /// Native OS hostname
    pub hostname: String,
    /// Operating system name (Windows, Linux, macOS)
    pub os: String,
    /// Hardware architecture (x64, ARM64)
    pub arch: String,
    /// FRIDAY software version
    pub version: String,
    /// Supported hardware capabilities
    pub capabilities: DeviceCapabilities,
}

impl DeviceIdentity {
    /// Resolves the default directory for FRIDAY configuration files
    pub fn default_config_dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("friday");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir
    }

    /// Default identity file path
    pub fn identity_file_path(config_dir: Option<&Path>) -> PathBuf {
        config_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_config_dir)
            .join("identity.json")
    }

    /// Load existing identity or generate a new stable one on first boot
    pub fn load_or_create(config_dir: Option<&Path>) -> Self {
        let path = Self::identity_file_path(config_dir);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut identity) = serde_json::from_str::<DeviceIdentity>(&content) {
                    // Update dynamic fields if OS or version changed
                    let (current_os, current_arch) = Self::detect_os_info();
                    let current_hostname = Self::detect_hostname();
                    identity.os = current_os;
                    identity.arch = current_arch;
                    identity.hostname = current_hostname;
                    identity.version = DEFAULT_VERSION.to_string();
                    let _ = identity.save(config_dir);
                    return identity;
                }
            }
        }

        // Generate brand new stable identity
        let device_id = Uuid::new_v4().to_string();
        let hostname = Self::detect_hostname();
        let display_name = Self::sanitize_display_name(&hostname);
        let (os, arch) = Self::detect_os_info();

        let identity = Self {
            device_id,
            display_name,
            hostname,
            os,
            arch,
            version: DEFAULT_VERSION.to_string(),
            capabilities: DeviceCapabilities::default(),
        };

        let _ = identity.save(config_dir);
        info!(
            "Generated new stable FRIDAY device identity: {} ('{}')",
            identity.device_id, identity.display_name
        );
        identity
    }

    /// Save identity to disk
    pub fn save(&self, config_dir: Option<&Path>) -> Result<(), std::io::Error> {
        let path = Self::identity_file_path(config_dir);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Update display name and save to disk
    pub fn set_display_name(
        &mut self,
        name: &str,
        config_dir: Option<&Path>,
    ) -> Result<(), std::io::Error> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Device display name cannot be empty",
            ));
        }
        self.display_name = trimmed.to_string();
        self.save(config_dir)
    }

    /// Sanitize hostname into a human-readable computer name (e.g. "yoga.local" -> "Yoga")
    pub fn sanitize_display_name(hostname: &str) -> String {
        let trimmed = hostname
            .trim_end_matches(".local")
            .trim_end_matches(".lan")
            .trim_end_matches(".home")
            .trim();

        if trimmed.is_empty() {
            return "Friday-Device".to_string();
        }

        // Capitalize first letter if all lowercase
        let mut chars = trimmed.chars();
        match chars.next() {
            None => "Friday-Device".to_string(),
            Some(first) => {
                if first.is_lowercase() {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                } else {
                    trimmed.to_string()
                }
            }
        }
    }

    /// Detect native host name
    pub fn detect_hostname() -> String {
        #[cfg(target_os = "windows")]
        {
            if let Ok(name) = std::env::var("COMPUTERNAME") {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
        if let Ok(name) = std::env::var("HOSTNAME") {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Ok(out) = std::process::Command::new("hostname").output() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        "Local-PC".to_string()
    }

    /// Detect operating system and CPU architecture
    pub fn detect_os_info() -> (String, String) {
        let os_str = match std::env::consts::OS {
            "windows" => "Windows",
            "linux" => "Linux",
            "macos" => "macOS",
            other => other,
        };
        let arch_str = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "ARM64",
            other => other,
        };
        (os_str.to_string(), arch_str.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_identity_persistence() {
        let unique_dir = std::env::temp_dir().join(format!("friday_id_test_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&unique_dir);
        let path = &unique_dir;

        // 1. First run creates a stable identity
        let id1 = DeviceIdentity::load_or_create(Some(path));
        assert!(!id1.device_id.is_empty());
        assert!(!id1.display_name.is_empty());

        // 2. Second run must load the exact same identity
        let id2 = DeviceIdentity::load_or_create(Some(path));
        assert_eq!(id1.device_id, id2.device_id);
        assert_eq!(id1.display_name, id2.display_name);

        // 3. Modifying display name updates file
        let mut id3 = id2.clone();
        id3.set_display_name("My Custom Yoga", Some(path)).unwrap();
        assert_eq!(id3.display_name, "My Custom Yoga");

        let id4 = DeviceIdentity::load_or_create(Some(path));
        assert_eq!(id4.device_id, id1.device_id);
        assert_eq!(id4.display_name, "My Custom Yoga");

        let _ = fs::remove_dir_all(&unique_dir);
    }

    #[test]
    fn test_sanitize_display_name() {
        assert_eq!(DeviceIdentity::sanitize_display_name("yoga.local"), "Yoga");
        assert_eq!(DeviceIdentity::sanitize_display_name("desktop"), "Desktop");
        assert_eq!(DeviceIdentity::sanitize_display_name("G50"), "G50");
        assert_eq!(DeviceIdentity::sanitize_display_name(""), "Friday-Device");
    }
}
