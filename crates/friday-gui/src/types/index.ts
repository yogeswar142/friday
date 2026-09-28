export interface EngineStatus {
  state: "running" | "stopped" | "paused";
  active_device_id: string;
  local_device_id: string;
  connected_count: number;
  network_state: string;
  latency_ms: number;
  packet_loss_pct: number;
  packets_transferred: number;
  topology_summary: string;
}

export interface DeviceInfo {
  id: string;
  name: string;
  os: string;
  arch: string;
  ip_address: string;
  port: number;
  is_local: boolean;
  is_active: boolean;
  is_connected: boolean;
  latency_ms: number;
  capabilities: string[];
}

export interface DiscoveredDevice {
  id: string;
  name: string;
  os: string;
  arch: string;
  ip_address: string;
  port: number;
  is_paired: boolean;
}

export interface TopologyLink {
  source: string;
  target: string;
  edge: string;
}

export interface TopologyDto {
  ring: string[];
  active_device: string;
  devices: DeviceInfo[];
  links: TopologyLink[];
}

export interface TelemetryDto {
  latency_ms: number;
  packet_loss_pct: number;
  packets_per_sec: number;
  bytes_per_sec: number;
  total_transfers: number;
  uptime_seconds: number;
}

export interface PlatformCapabilities {
  mouse_capture: boolean;
  mouse_injection: boolean;
  keyboard_capture: boolean;
  keyboard_injection: boolean;
  clipboard: boolean;
  file_transfer: boolean;
  screen_information: boolean;
  multi_monitor: boolean;
  edge_detection: boolean;
  system_tray: boolean;
  startup: boolean;
  notifications: boolean;
}

export interface PlatformPermissions {
  platform: string;
  display_server: string;
  input_backend: string;
  has_input_permission: boolean;
  permission_warning?: string;
  permission_instructions?: string;
}

export interface DiagnosticCheckItem {
  name: string;
  passed: boolean;
  detail: string;
}

export interface DiagnosticReport {
  engine_state: string;
  platform: string;
  architecture: string;
  os_version: string;
  input_backend: string;
  network_transport: string;
  latency_ms: number;
  packet_loss_pct: number;
  active_device: string;
  ring_nodes: string[];
  topology_valid: boolean;
  checks: DiagnosticCheckItem[];
}

export interface BenchmarkReport {
  serialization_throughput_kops: number;
  coord_transform_latency_ns: number;
  routing_decision_latency_ns: number;
  simulated_hops_tested: number;
  rating: string;
}

export interface SettingsDto {
  edge_dwell_ms: number;
  edge_threshold_px: number;
  cursor_continuity: boolean;
  cursor_memory: boolean;
  peer_port: number;
  discovery_enabled: boolean;
  network_interface: string;
  timeout_ms: number;
  start_with_system: boolean;
  start_minimized: boolean;
  appearance: "dark" | "light" | "system";
  log_level: string;
  developer_mode: boolean;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

export type TabType = "overview" | "devices" | "topology" | "diagnostics" | "settings";
