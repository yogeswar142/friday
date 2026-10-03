export interface EngineStatus {
  state: "running" | "stopped" | "paused";
  active_device_id: string;
  local_device_id: string;
  is_host: boolean;
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
  connection_state?: string;
  trust_state?: string;
  share_mouse?: boolean;
  share_keyboard?: boolean;
  share_clipboard?: boolean;
}

export interface DiscoveredDevice {
  id: string;
  name: string;
  os: string;
  arch: string;
  ip_address: string;
  port: number;
  is_paired: boolean;
  connection_state?: string;
  version?: string;
  capabilities?: string[];
}

export interface LocalDeviceDto {
  device_id: string;
  display_name: string;
  hostname: string;
  os: string;
  arch: string;
  version: string;
  is_host: boolean;
}

export interface NetworkDiagnosticsDto {
  local_ip: string;
  remote_ip?: string;
  port: number;
  transport: string;
  interface: string;
  connection_id: string;
  packets_sent: number;
  packets_received: number;
  rtt_ms: number;
  packet_loss_pct: number;
  reconnect_attempts: number;
  discovery_status: string;
  connection_state: string;
}

/** An incoming pairing request from a remote FRIDAY node */
export interface PendingPairRequest {
  pin: string;
  from_id: string;
  from_name: string;
  from_ip: string;
  reply_port: number;
}

export interface TopologyLink {
  source: string;
  target: string;
  edge: string;
}

export interface TopologyDto {
  ring: string[];
  active_device: string;
  is_host: boolean;
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

export interface NetworkPacketBreakdown {
  mouse_moves: number;
  mouse_buttons: number;
  keyboard_events: number;
  control_packets: number;
  clipboard_packets: number;
}

export interface TimelineEventDto {
  timestamp: string;
  level: string;
  category: string;
  message: string;
}

export interface NetworkWorkingReportDto {
  session_duration: string;
  session_start_time: string;
  local_role: string;
  local_device_name: string;
  local_device_id: string;
  local_ip: string;
  local_port: number;
  active_device_name: string;
  active_device_id: string;
  is_controlling_remote: boolean;
  connected_peers_count: number;
  peers_summary: string[];
  total_tx_packets: number;
  total_tx_bytes: number;
  total_rx_packets: number;
  total_rx_bytes: number;
  current_tx_pps: number;
  current_rx_pps: number;
  current_tx_kbps: number;
  current_rx_kbps: number;
  tx_breakdown: NetworkPacketBreakdown;
  rx_breakdown: NetworkPacketBreakdown;
  latency_ms: number;
  min_latency_ms: number;
  max_latency_ms: number;
  avg_latency_ms: number;
  jitter_ms: number;
  max_jitter_ms: number;
  stall_count: number;
  last_stall_ms: number;
  max_stall_ms: number;
  network_health: string;
  timeline: TimelineEventDto[];
  formatted_report: string;
}
