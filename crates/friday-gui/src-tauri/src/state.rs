use std::sync::{Arc, Mutex};
use friday_core::{DisplayBounds, ScreenLayout, CircularTopology};
use crate::config::ConfigManager;
use crate::types::{DeviceInfo, DiscoveredDevice, EngineStatus, LogEntryDto, SettingsDto, TelemetryDto};

pub struct AppState {
    pub engine_running: bool,
    pub engine_paused: bool,
    pub local_device_id: String,
    pub active_device_id: String,
    pub topology: CircularTopology,
    pub devices: Vec<DeviceInfo>,
    pub discovered_devices: Vec<DiscoveredDevice>,
    pub settings: SettingsDto,
    pub telemetry: TelemetryDto,
    pub logs: Vec<LogEntryDto>,
    pub start_time: std::time::Instant,
}

pub fn detect_local_hostname() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        let trimmed = name.trim();
        if !trimmed.is_empty() { return trimmed.to_string(); }
    }
    if let Ok(name) = std::env::var("HOSTNAME") {
        let trimmed = name.trim();
        if !trimmed.is_empty() { return trimmed.to_string(); }
    }
    if let Ok(out) = std::process::Command::new("hostname").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() { return s; }
    }
    "Local-PC".to_string()
}

pub fn detect_local_ip() -> String {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                let ip = local_addr.ip();
                if !ip.is_unspecified() && !ip.is_loopback() {
                    return ip.to_string();
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

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

impl AppState {
    pub fn new() -> Self {
        let config = ConfigManager::load_config();
        let local_id = detect_local_hostname();
        let local_ip = detect_local_ip();
        let (os, arch) = detect_os_info();

        let local_device = DeviceInfo {
            id: local_id.clone(),
            name: format!("{} (This Machine)", local_id),
            os,
            arch,
            ip_address: local_ip.clone(),
            port: config.settings.peer_port,
            is_local: true,
            is_active: true,
            is_connected: true,
            latency_ms: 0.0,
            capabilities: vec!["mouse_capture".into(), "mouse_injection".into(), "edge_detection".into()],
        };

        let mut devices = vec![local_device];
        for mut peer in config.paired_devices {
            if peer.id != local_id {
                peer.is_local = false;
                peer.is_active = false;
                devices.push(peer);
            }
        }

        let mut ring = config.ring_topology;
        if ring.is_empty() || !ring.contains(&local_id) {
            ring = devices.iter().map(|d| d.id.clone()).collect();
        }

        let layouts: Vec<ScreenLayout> = devices.iter().map(|d| {
            ScreenLayout {
                device_id: d.id.clone(),
                name: d.name.clone(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, d.is_local),
            }
        }).collect();

        let mut ct = CircularTopology::from_ring(layouts);
        ct.set_ring(ring.clone());
        ct.set_active_device(&local_id);

        let mut logs = Vec::new();
        logs.push(LogEntryDto {
            timestamp: chrono_now(),
            level: "INFO".into(),
            target: "friday_core::engine".into(),
            message: format!("FRIDAY initialized on local machine: {} ({})", local_id, local_ip),
        });
        if ring.len() >= 2 {
            logs.push(LogEntryDto {
                timestamp: chrono_now(),
                level: "INFO".into(),
                target: "friday_core::topology".into(),
                message: format!("Restored circular ring: {} → {}", ring.join(" → "), ring[0]),
            });
        }

        Self {
            engine_running: true,
            engine_paused: false,
            local_device_id: local_id.clone(),
            active_device_id: local_id,
            topology: ct,
            devices,
            discovered_devices: Vec::new(),
            settings: config.settings,
            telemetry: TelemetryDto {
                latency_ms: 0.0,
                packet_loss_pct: 0.0,
                packets_per_sec: 0,
                bytes_per_sec: 0,
                total_transfers: 0,
                uptime_seconds: 0,
            },
            logs,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn persist_config(&self) {
        let app_cfg = crate::config::AppConfig {
            settings: self.settings.clone(),
            paired_devices: self.devices.iter().filter(|d| !d.is_local).cloned().collect(),
            ring_topology: self.topology.ring.clone(),
        };
        let _ = ConfigManager::save_config(&app_cfg);
    }


    pub fn get_engine_status(&self) -> EngineStatus {
        let state = if !self.engine_running {
            "stopped"
        } else if self.engine_paused {
            "paused"
        } else {
            "running"
        };

        let summary = if self.topology.ring.is_empty() {
            "No devices configured".to_string()
        } else {
            format!("{} → {}", self.topology.ring.join(" → "), self.topology.ring[0])
        };

        EngineStatus {
            state: state.to_string(),
            active_device_id: self.active_device_id.clone(),
            local_device_id: self.local_device_id.clone(),
            connected_count: self.devices.iter().filter(|d| d.is_connected).count(),
            network_state: if self.engine_running { "Connected".into() } else { "Disconnected".into() },
            latency_ms: self.telemetry.latency_ms,
            packet_loss_pct: self.telemetry.packet_loss_pct,
            packets_transferred: self.telemetry.total_transfers * 128,
            topology_summary: summary,
        }
    }

    pub fn add_log(&mut self, level: &str, target: &str, msg: &str) {
        self.logs.push(LogEntryDto {
            timestamp: chrono_now(),
            level: level.to_string(),
            target: target.to_string(),
            message: msg.to_string(),
        });
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }
}

fn chrono_now() -> String {
    // Standard timestamp without external chrono dependency
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (now / 3600) % 24;
    let minutes = (now / 60) % 60;
    let seconds = now % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub type SharedAppState = Arc<Mutex<AppState>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_initialization() {
        let state = AppState::new();
        assert!(state.engine_running);
        assert!(!state.engine_paused);
        assert!(!state.local_device_id.is_empty());
        assert_eq!(state.active_device_id, state.local_device_id);
        assert!(!state.devices.is_empty());
        assert_eq!(state.devices[0].id, state.local_device_id);
        assert!(state.devices[0].is_local);

        let status = state.get_engine_status();
        assert_eq!(status.state, "running");
        assert_eq!(status.active_device_id, state.local_device_id);
        assert!(status.connected_count >= 1);
    }

    #[test]
    fn test_log_rotation() {
        let mut state = AppState::new();
        for i in 0..1050 {
            state.add_log("INFO", "test", &format!("Log message {}", i));
        }
        assert_eq!(state.logs.len(), 1000);
    }
}

