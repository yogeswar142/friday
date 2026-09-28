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

impl AppState {
    pub fn new() -> Self {
        let settings = ConfigManager::load();
        let local_id = "G50".to_string();
        let mut devices = Vec::new();

        devices.push(DeviceInfo {
            id: "G50".into(),
            name: "Lenovo G50 (Local)".into(),
            os: "Linux".into(),
            arch: "x64".into(),
            ip_address: "192.168.1.11".into(),
            port: 48700,
            is_local: true,
            is_active: true,
            is_connected: true,
            latency_ms: 0.0,
            capabilities: vec!["mouse_capture".into(), "mouse_injection".into(), "edge_detection".into()],
        });

        devices.push(DeviceInfo {
            id: "Yoga".into(),
            name: "Lenovo Yoga".into(),
            os: "Windows 11".into(),
            arch: "x64".into(),
            ip_address: "192.168.1.2".into(),
            port: 48700,
            is_local: false,
            is_active: false,
            is_connected: true,
            latency_ms: 0.72,
            capabilities: vec!["mouse_capture".into(), "mouse_injection".into(), "edge_detection".into()],
        });

        devices.push(DeviceInfo {
            id: "MacBook".into(),
            name: "MacBook Air".into(),
            os: "macOS Sonoma".into(),
            arch: "arm64".into(),
            ip_address: "192.168.1.45".into(),
            port: 48700,
            is_local: false,
            is_active: false,
            is_connected: true,
            latency_ms: 1.15,
            capabilities: vec!["mouse_capture".into(), "mouse_injection".into(), "edge_detection".into()],
        });

        let mut discovered = Vec::new();
        discovered.push(DiscoveredDevice {
            id: "ThinkPad".into(),
            name: "ThinkPad X1".into(),
            os: "Linux Fedora".into(),
            arch: "x64".into(),
            ip_address: "192.168.1.88".into(),
            port: 48700,
            is_paired: false,
        });

        // Initialize circular topology
        let layouts = vec![
            ScreenLayout {
                device_id: "G50".into(),
                name: "Lenovo G50".into(),
                bounds: DisplayBounds::new(0, 0, 1366, 768, 1.0, true),
            },
            ScreenLayout {
                device_id: "Yoga".into(),
                name: "Lenovo Yoga".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            },
            ScreenLayout {
                device_id: "MacBook".into(),
                name: "MacBook Air".into(),
                bounds: DisplayBounds::new(0, 0, 2560, 1600, 2.0, false),
            },
        ];
        let ct = CircularTopology::from_ring(layouts);

        let mut logs = Vec::new();
        logs.push(LogEntryDto {
            timestamp: chrono_now(),
            level: "INFO".into(),
            target: "friday_core::engine".into(),
            message: "FRIDAY Core Control Plane initialized with Circular N-Device Routing".into(),
        });
        logs.push(LogEntryDto {
            timestamp: chrono_now(),
            level: "INFO".into(),
            target: "friday_core::topology".into(),
            message: "Circular ring topology established: G50 → Yoga → MacBook → G50".into(),
        });

        Self {
            engine_running: true,
            engine_paused: false,
            local_device_id: local_id.clone(),
            active_device_id: local_id,
            topology: ct,
            devices,
            discovered_devices: discovered,
            settings,
            telemetry: TelemetryDto {
                latency_ms: 0.82,
                packet_loss_pct: 0.0,
                packets_per_sec: 240,
                bytes_per_sec: 15360,
                total_transfers: 14,
                uptime_seconds: 0,
            },
            logs,
            start_time: std::time::Instant::now(),
        }
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
        assert_eq!(state.local_device_id, "G50");
        assert_eq!(state.active_device_id, "G50");
        assert_eq!(state.devices.len(), 3);
        assert_eq!(state.topology.ring.len(), 3);

        let status = state.get_engine_status();
        assert_eq!(status.state, "running");
        assert_eq!(status.active_device_id, "G50");
        assert_eq!(status.connected_count, 3);
        assert!(status.topology_summary.contains("G50 → Yoga → MacBook → G50"));
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
