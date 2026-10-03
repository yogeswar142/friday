import {
  BenchmarkReport,
  DeviceInfo,
  DiagnosticReport,
  DiscoveredDevice,
  EngineStatus,
  LocalDeviceDto,
  LogEntry,
  NetworkDiagnosticsDto,
  PendingPairRequest,
  PlatformCapabilities,
  PlatformPermissions,
  SettingsDto,
  TelemetryDto,
  TopologyDto,
} from "../types";

// Detect if running inside a Tauri webview
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    // Inside real Tauri app — call the Rust backend directly.
    // Errors propagate to the caller (no silent mock fallback).
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<T>(cmd, args);
  }
  // Browser / dev preview only — use local mock
  return mockFallback<T>(cmd, args);
}

// Fallback state for dev / browser previews
const userAgent = typeof navigator !== "undefined" ? navigator.userAgent : "";
const detectedOs = userAgent.includes("Windows") ? "Windows 11" : userAgent.includes("Mac") ? "macOS" : "Linux";
const detectedHost = "Host-Machine";
let mockEngineRunning = true;
let mockActiveDevice = detectedHost;
let mockRing = [detectedHost];
let mockDevices: DeviceInfo[] = [
  {
    id: detectedHost,
    name: `${detectedHost} (This Machine)`,
    os: detectedOs,
    arch: "x64",
    ip_address: "127.0.0.1",
    port: 48700,
    is_local: true,
    is_active: true,
    is_connected: true,
    latency_ms: 0.0,
    capabilities: ["mouse_capture", "mouse_injection", "edge_detection"],
  },
];

let mockDiscovered: DiscoveredDevice[] = [];

let mockLocalDevice: LocalDeviceDto = {
  device_id: "15435278-0a86-4fae-a84a-869833b6dad1",
  display_name: detectedHost,
  hostname: detectedHost,
  os: detectedOs,
  arch: "x64",
  version: "0.2.0",
  is_host: true,
};

let mockSettings: SettingsDto = {
  edge_dwell_ms: 500,
  edge_threshold_px: 3,
  cursor_continuity: true,
  cursor_memory: true,
  peer_port: 48700,
  discovery_enabled: true,
  network_interface: "Default",
  timeout_ms: 3000,
  start_with_system: false,
  start_minimized: false,
  appearance: "dark",
  log_level: "info",
  developer_mode: false,
};

function mockFallback<T>(cmd: string, args?: Record<string, unknown>): T {
  switch (cmd) {
    case "get_status":
      return {
        state: mockEngineRunning ? "running" : "stopped",
        active_device_id: mockActiveDevice,
        local_device_id: "G50",
        is_host: true,
        connected_count: mockDevices.filter((d) => d.is_connected).length,
        network_state: mockEngineRunning ? "Connected" : "Disconnected",
        latency_ms: 0.82,
        packet_loss_pct: 0.0,
        packets_transferred: 18432,
        topology_summary: `${mockRing.join(" → ")} → ${mockRing[0]}`,
      } as unknown as T;

    case "start_engine":
      mockEngineRunning = true;
      return mockFallback("get_status");

    case "stop_engine":
      mockEngineRunning = false;
      return mockFallback("get_status");

    case "pause_engine":
      return mockFallback("get_status");

    case "get_active_device":
      return mockActiveDevice as unknown as T;

    case "switch_active_device": {
      const dev = args?.device_id as string;
      if (dev) {
        mockActiveDevice = dev;
        mockDevices.forEach((d) => (d.is_active = d.id === dev));
      }
      return undefined as unknown as T;
    }

    case "get_devices":
      return mockDevices as unknown as T;

    case "discover_devices":
      return mockDiscovered as unknown as T;

    case "pair_device": {
      const devId = args?.device_id as string;
      const found = mockDiscovered.find((d) => d.id === devId);
      if (found) {
        mockDiscovered = mockDiscovered.filter((d) => d.id !== devId);
        const newDev: DeviceInfo = {
          ...found,
          is_local: false,
          is_active: false,
          is_connected: true,
          latency_ms: 0.95,
          capabilities: ["mouse_capture", "mouse_injection", "edge_detection"],
        };
        mockDevices.push(newDev);
        mockRing.push(devId);
        return newDev as unknown as T;
      }
      throw new Error(`Device ${devId} not found`);
    }

    case "unpair_device": {
      const devId = args?.device_id as string;
      mockDevices = mockDevices.filter((d) => d.id !== devId);
      mockRing = mockRing.filter((id) => id !== devId);
      if (mockActiveDevice === devId) {
        mockActiveDevice = "G50";
      }
      return undefined as unknown as T;
    }

    case "connect_device": {
      const devId = args?.device_id as string;
      const dev = mockDevices.find((d) => d.id === devId);
      if (dev) dev.is_connected = true;
      return undefined as unknown as T;
    }

    case "disconnect_device": {
      const devId = args?.device_id as string;
      const dev = mockDevices.find((d) => d.id === devId);
      if (dev) dev.is_connected = false;
      return undefined as unknown as T;
    }

    case "get_topology": {
      const links = [];
      for (let i = 0; i < mockRing.length; i++) {
        const next = (i + 1) % mockRing.length;
        links.push({
          source: mockRing[i],
          target: mockRing[next],
          edge: "Right",
        });
      }
      return {
        ring: mockRing,
        active_device: mockActiveDevice,
        is_host: true,
        devices: mockDevices,
        links,
      } as unknown as T;
    }

    case "set_topology":
    case "reorder_ring": {
      const newRing = args?.ring as string[];
      if (newRing && newRing.length > 0) {
        mockRing = newRing;
        if (!mockRing.includes(mockActiveDevice)) {
          mockActiveDevice = mockRing[0];
        }
      }
      return undefined as unknown as T;
    }

    case "get_telemetry":
      return {
        latency_ms: 0.82,
        packet_loss_pct: 0.0,
        packets_per_sec: 240,
        bytes_per_sec: 15360,
        total_transfers: 42,
        uptime_seconds: 1420,
      } as unknown as T;

    case "run_diagnostics":
      return {
        engine_state: mockEngineRunning ? "running" : "stopped",
        platform: "linux",
        architecture: "x64",
        os_version: "Ubuntu 24.04 LTS (Noble)",
        input_backend: "X11 XTest / XRecord",
        network_transport: "UDP with Bincode packet protocol (sub-millisecond)",
        latency_ms: 0.82,
        packet_loss_pct: 0.0,
        active_device: mockActiveDevice,
        ring_nodes: mockRing,
        topology_valid: mockRing.length >= 2,
        checks: [
          { name: "Engine Control Plane", passed: mockEngineRunning, detail: `Engine is ${mockEngineRunning ? "running" : "stopped"}` },
          { name: "Circular Topology Ring", passed: true, detail: `${mockRing.length} nodes in circular ring: ${mockRing.join(" → ")}` },
          { name: "Active Input Owner", passed: true, detail: `Exclusive ownership assigned to ${mockActiveDevice}` },
          { name: "Network Latency & Jitter", passed: true, detail: "0.82 ms roundtrip latency, 0.0% packet loss" },
          { name: "Platform Display Server", passed: true, detail: "Connected to X11" },
        ],
      } as unknown as T;

    case "run_benchmark":
      return {
        serialization_throughput_kops: 842.5,
        coord_transform_latency_ns: 24.1,
        routing_decision_latency_ns: 68.3,
        simulated_hops_tested: 50000,
        rating: "Ultra Low Latency (Production Ready)",
      } as unknown as T;

    case "get_settings":
      return mockSettings as unknown as T;

    case "save_settings": {
      mockSettings = args?.settings as SettingsDto;
      return undefined as unknown as T;
    }

    case "get_platform_capabilities":
      return {
        mouse_capture: true,
        mouse_injection: true,
        keyboard_capture: false,
        keyboard_injection: false,
        clipboard: false,
        file_transfer: false,
        screen_information: true,
        multi_monitor: true,
        edge_detection: true,
        system_tray: true,
        startup: true,
        notifications: true,
      } as unknown as T;

    case "get_platform_permissions":
      return {
        platform: "linux",
        display_server: "X11",
        input_backend: "X11 (XTest)",
        has_input_permission: true,
        permission_warning: undefined,
        permission_instructions: undefined,
      } as unknown as T;

    case "add_manual_device": {
      const ip = (args?.ip_address as string) || "192.168.1.2";
      const name = (args?.name as string) || `Laptop (${ip})`;
      const port = (args?.port as number) || 48700;
      const id = name.replace(/\s+/g, "_");
      const newDev: DeviceInfo = {
        id,
        name,
        os: "Remote Machine",
        arch: "x64",
        ip_address: ip,
        port,
        is_local: false,
        is_active: false,
        is_connected: true,
        latency_ms: 0.85,
        capabilities: ["mouse_capture", "mouse_injection", "edge_detection"],
      };
      mockDevices.push(newDev);
      if (!mockRing.includes(id)) {
        mockRing.push(id);
      }
      return newDev as unknown as T;
    }

    case "get_logs":
      return [
        { timestamp: "17:15:00", level: "INFO", target: "friday_core::engine", message: "FRIDAY Core Control Plane initialized with Circular N-Device Routing" },
        { timestamp: "17:15:05", level: "INFO", target: "friday_network::transport", message: "UDP Transport listening on 0.0.0.0:48700" },
      ] as unknown as T;

    case "clear_logs":
      return undefined as unknown as T;

    case "get_pending_pair_requests":
      return [] as unknown as T;

    case "respond_to_pair_request":
      return undefined as unknown as T;

    case "initiate_pairing":
      // In mock mode, simulate instant accept after 1.5s
      return new Promise<T>((resolve) => setTimeout(() => resolve(true as unknown as T), 1500)) as unknown as T;

    case "get_local_device":
      return mockLocalDevice as unknown as T;

    case "set_local_device_name": {
      const name = args?.name as string;
      if (name) {
        mockLocalDevice.display_name = name;
      }
      return mockLocalDevice as unknown as T;
    }

    case "get_discovered_devices":
      return mockDiscovered as unknown as T;

    case "get_paired_devices":
      return mockDevices as unknown as T;

    case "start_discovery":
      return mockDiscovered as unknown as T;

    case "stop_discovery":
      return undefined as unknown as T;

    case "approve_pairing":
      return undefined as unknown as T;

    case "reject_pairing":
      return undefined as unknown as T;

    case "forget_device": {
      const devId = args?.device_id as string;
      mockDevices = mockDevices.filter((d) => d.id !== devId);
      mockRing = mockRing.filter((id) => id !== devId);
      return undefined as unknown as T;
    }

    case "get_connection_status":
      return "Connected" as unknown as T;

    case "get_network_diagnostics":
      return {
        local_ip: "192.168.1.100",
        remote_ip: "192.168.1.105",
        port: 48700,
        transport: "UDP with Bincode",
        interface: "Wi-Fi (WLAN)",
        connection_id: mockLocalDevice.device_id,
        packets_sent: 12450,
        packets_received: 12430,
        rtt_ms: 0.85,
        packet_loss_pct: 0.0,
        reconnect_attempts: 0,
        discovery_status: "Active (mDNS _friday._udp.local. + Broadcast)",
        connection_state: "Connected",
      } as unknown as T;

    default:
      return undefined as unknown as T;
  }
}

// Exported typed Control API
export const api = {
  getStatus: () => invokeTauri<EngineStatus>("get_status"),
  startEngine: () => invokeTauri<EngineStatus>("start_engine"),
  stopEngine: () => invokeTauri<EngineStatus>("stop_engine"),
  pauseEngine: () => invokeTauri<EngineStatus>("pause_engine"),
  getActiveDevice: () => invokeTauri<string>("get_active_device"),
  switchActiveDevice: (deviceId: string) =>
    invokeTauri<void>("switch_active_device", { deviceId, device_id: deviceId }),
  getDevices: () => invokeTauri<DeviceInfo[]>("get_devices"),
  discoverDevices: () => invokeTauri<DiscoveredDevice[]>("discover_devices"),
  addManualDevice: (ipAddress: string, port?: number, name?: string) =>
    invokeTauri<DeviceInfo>("add_manual_device", { ipAddress, ip_address: ipAddress, port, name }),
  pairDevice: (deviceId: string) =>
    invokeTauri<DeviceInfo>("pair_device", { deviceId, device_id: deviceId }),
  unpairDevice: (deviceId: string) =>
    invokeTauri<void>("unpair_device", { deviceId, device_id: deviceId }),
  connectDevice: (deviceId: string) =>
    invokeTauri<void>("connect_device", { deviceId, device_id: deviceId }),
  disconnectDevice: (deviceId: string) =>
    invokeTauri<void>("disconnect_device", { deviceId, device_id: deviceId }),
  getTopology: () => invokeTauri<TopologyDto>("get_topology"),
  setTopology: (ring: string[]) => invokeTauri<void>("set_topology", { ring }),
  reorderRing: (ring: string[]) => invokeTauri<void>("reorder_ring", { ring }),
  addRingDevice: (deviceId: string) =>
    invokeTauri<void>("add_ring_device", { deviceId, device_id: deviceId }),
  removeRingDevice: (deviceId: string) =>
    invokeTauri<void>("remove_ring_device", { deviceId, device_id: deviceId }),
  getTelemetry: () => invokeTauri<TelemetryDto>("get_telemetry"),
  runDiagnostics: () => invokeTauri<DiagnosticReport>("run_diagnostics"),
  runBenchmark: () => invokeTauri<BenchmarkReport>("run_benchmark"),
  getSettings: () => invokeTauri<SettingsDto>("get_settings"),
  saveSettings: (settings: SettingsDto) => invokeTauri<void>("save_settings", { settings }),
  getPlatformCapabilities: () => invokeTauri<PlatformCapabilities>("get_platform_capabilities"),
  getPlatformPermissions: () => invokeTauri<PlatformPermissions>("get_platform_permissions"),
  openPermissionSettings: () => invokeTauri<void>("open_permission_settings"),
  getLogs: () => invokeTauri<LogEntry[]>("get_logs"),
  clearLogs: () => invokeTauri<void>("clear_logs"),
  /** Send pairing request to remote device — blocks up to 30s waiting for user accept */
  initiatePairing: (deviceId: string, pin: string, targetIp?: string) =>
    invokeTauri<boolean>("initiate_pairing", {
      deviceId,
      device_id: deviceId,
      pin,
      targetIp,
      target_ip: targetIp,
    }),
  /** Poll for incoming pair requests on this machine (call every 2s) */
  getPendingPairRequests: () =>
    invokeTauri<PendingPairRequest[]>("get_pending_pair_requests"),
  /** Accept or reject an incoming pair request */
  respondToPairRequest: (pin: string, accept: boolean) =>
    invokeTauri<void>("respond_to_pair_request", { pin, accept }),
  /** Set device role: true for Host (Controller), false for Client (Receiver) */
  setDeviceRole: (isHost: boolean) =>
    invokeTauri<void>("set_device_role", { isHost, is_host: isHost }),
  /** Get stable identity and name of this local machine */
  getLocalDevice: () => invokeTauri<LocalDeviceDto>("get_local_device"),
  /** Rename this machine's display name */
  setLocalDeviceName: (name: string) =>
    invokeTauri<LocalDeviceDto>("set_local_device_name", { name }),
  /** Get list of nearby discovered devices */
  getDiscoveredDevices: () =>
    invokeTauri<DiscoveredDevice[]>("get_discovered_devices"),
  /** Get list of paired & trusted devices */
  getPairedDevices: () => invokeTauri<DeviceInfo[]>("get_paired_devices"),
  /** Start active discovery refresh */
  startDiscovery: () => invokeTauri<DiscoveredDevice[]>("start_discovery"),
  /** Stop discovery */
  stopDiscovery: () => invokeTauri<void>("stop_discovery"),
  /** Approve pending pairing request with PIN */
  approvePairing: (pin: string) => invokeTauri<void>("approve_pairing", { pin }),
  /** Reject pending pairing request with PIN */
  rejectPairing: (pin: string) => invokeTauri<void>("reject_pairing", { pin }),
  /** Forget / unpair device permanently */
  forgetDevice: (deviceId: string) =>
    invokeTauri<void>("forget_device", { deviceId, device_id: deviceId }),
  /** Query live connection status */
  getConnectionStatus: (deviceId: string) =>
    invokeTauri<string>("get_connection_status", { deviceId, device_id: deviceId }),
  /** Query low-level network diagnostics */
  getNetworkDiagnostics: (targetId?: string) =>
    invokeTauri<NetworkDiagnosticsDto>("get_network_diagnostics", {
      targetId,
      target_id: targetId,
    }),
};
