use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub state: String, // "running", "stopped", "paused"
    pub active_device_id: String,
    pub local_device_id: String,
    pub is_host: bool,
    pub connected_count: usize,
    pub network_state: String,
    pub latency_ms: f32,
    pub packet_loss_pct: f32,
    pub packets_transferred: u64,
    pub topology_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub os: String,
    pub arch: String,
    pub ip_address: String,
    pub port: u16,
    pub is_local: bool,
    pub is_active: bool,
    pub is_connected: bool,
    pub latency_ms: f32,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub id: String,
    pub name: String,
    pub os: String,
    pub arch: String,
    pub ip_address: String,
    pub port: u16,
    pub is_paired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyLinkDto {
    pub source: String,
    pub target: String,
    pub edge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyDto {
    pub ring: Vec<String>,
    pub active_device: String,
    pub is_host: bool,
    pub devices: Vec<DeviceInfo>,
    pub links: Vec<TopologyLinkDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDto {
    pub latency_ms: f32,
    pub packet_loss_pct: f32,
    pub packets_per_sec: u32,
    pub bytes_per_sec: u32,
    pub total_transfers: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilitiesDto {
    pub mouse_capture: bool,
    pub mouse_injection: bool,
    pub keyboard_capture: bool,
    pub keyboard_injection: bool,
    pub clipboard: bool,
    pub file_transfer: bool,
    pub screen_information: bool,
    pub multi_monitor: bool,
    pub edge_detection: bool,
    pub system_tray: bool,
    pub startup: bool,
    pub notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformPermissionsDto {
    pub platform: String,
    pub display_server: String,
    pub input_backend: String,
    pub has_input_permission: bool,
    pub permission_warning: Option<String>,
    pub permission_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheckItem {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReportDto {
    pub engine_state: String,
    pub platform: String,
    pub architecture: String,
    pub os_version: String,
    pub input_backend: String,
    pub network_transport: String,
    pub latency_ms: f32,
    pub packet_loss_pct: f32,
    pub active_device: String,
    pub ring_nodes: Vec<String>,
    pub topology_valid: bool,
    pub checks: Vec<DiagnosticCheckItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReportDto {
    pub serialization_throughput_kops: f64,
    pub coord_transform_latency_ns: f64,
    pub routing_decision_latency_ns: f64,
    pub simulated_hops_tested: usize,
    pub rating: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDto {
    pub edge_dwell_ms: u64,
    pub edge_threshold_px: i32,
    pub cursor_continuity: bool,
    pub cursor_memory: bool,
    pub peer_port: u16,
    pub discovery_enabled: bool,
    pub network_interface: String,
    pub timeout_ms: u64,
    pub start_with_system: bool,
    pub start_minimized: bool,
    pub appearance: String, // "dark", "light", "system"
    pub log_level: String,
    pub developer_mode: bool,
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self {
            edge_dwell_ms: 500,
            edge_threshold_px: 3,
            cursor_continuity: true,
            cursor_memory: true,
            peer_port: 48700,
            discovery_enabled: true,
            network_interface: "Default".into(),
            timeout_ms: 3000,
            start_with_system: false,
            start_minimized: false,
            appearance: "dark".into(),
            log_level: "info".into(),
            developer_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryDto {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}
