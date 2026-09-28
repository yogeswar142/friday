use tauri::State;
use friday_core::{DisplayBounds, ScreenLayout};
use crate::config::ConfigManager;
use crate::diagnostics::{run_core_benchmark, run_system_diagnostics};
use crate::state::SharedAppState;
use crate::types::{
    BenchmarkReportDto, DeviceInfo, DiagnosticReportDto, DiscoveredDevice, EngineStatus,
    LogEntryDto, PlatformCapabilitiesDto, PlatformPermissionsDto, SettingsDto, TelemetryDto,
    TopologyDto, TopologyLinkDto,
};

#[tauri::command]
pub fn get_status(state: State<'_, SharedAppState>) -> EngineStatus {
    let app = state.lock().unwrap();
    app.get_engine_status()
}

#[tauri::command]
pub fn start_engine(state: State<'_, SharedAppState>) -> Result<EngineStatus, String> {
    let mut app = state.lock().unwrap();
    app.engine_running = true;
    app.engine_paused = false;
    app.add_log("INFO", "friday_core::engine", "Engine started by user request");
    Ok(app.get_engine_status())
}

#[tauri::command]
pub fn stop_engine(state: State<'_, SharedAppState>) -> Result<EngineStatus, String> {
    let mut app = state.lock().unwrap();
    app.engine_running = false;
    app.engine_paused = false;
    app.add_log("INFO", "friday_core::engine", "Engine stopped by user request");
    Ok(app.get_engine_status())
}

#[tauri::command]
pub fn pause_engine(state: State<'_, SharedAppState>) -> Result<EngineStatus, String> {
    let mut app = state.lock().unwrap();
    app.engine_paused = !app.engine_paused;
    let msg = if app.engine_paused { "Engine paused" } else { "Engine resumed" };
    app.add_log("INFO", "friday_core::engine", msg);
    Ok(app.get_engine_status())
}

#[tauri::command]
pub fn get_active_device(state: State<'_, SharedAppState>) -> String {
    let app = state.lock().unwrap();
    app.active_device_id.clone()
}

#[tauri::command]
pub fn switch_active_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if !app.topology.ring.contains(&device_id) {
        return Err(format!("Device {} is not in the circular ring topology", device_id));
    }
    app.active_device_id = device_id.clone();
    app.topology.set_active_device(&device_id);
    for d in &mut app.devices {
        d.is_active = d.id == device_id;
    }
    app.add_log("INFO", "friday_core::ownership", &format!("Ownership manually switched to {}", device_id));
    Ok(())
}

#[tauri::command]
pub fn get_devices(state: State<'_, SharedAppState>) -> Vec<DeviceInfo> {
    let app = state.lock().unwrap();
    app.devices.clone()
}

#[tauri::command]
pub fn discover_devices(state: State<'_, SharedAppState>) -> Vec<DiscoveredDevice> {
    let (local_id, peer_port) = {
        let app = state.lock().unwrap();
        (app.local_device_id.clone(), app.settings.peer_port)
    };

    let mut discovered = Vec::new();
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        let _ = socket.set_broadcast(true);
        let _ = socket.set_read_timeout(Some(std::time::Duration::from_millis(300)));

        let msg = format!("FRIDAY_DISCOVERY_PING:{}", local_id);
        let broadcast_addr = format!("255.255.255.255:{}", peer_port);
        let _ = socket.send_to(msg.as_bytes(), &broadcast_addr);

        let mut buf = [0u8; 512];
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(300) {
            if let Ok((len, src)) = socket.recv_from(&mut buf) {
                let text = String::from_utf8_lossy(&buf[..len]);
                if text.starts_with("FRIDAY_NODE:") {
                    let parts: Vec<&str> = text.split(':').collect();
                    if parts.len() >= 6 && parts[1] != local_id {
                        let id = parts[1].to_string();
                        let name = parts[2].to_string();
                        let os = parts[3].to_string();
                        let arch = parts[4].to_string();
                        let port = parts[5].parse::<u16>().unwrap_or(peer_port);
                        let ip = src.ip().to_string();

                        if !discovered.iter().any(|d: &DiscoveredDevice| d.id == id) {
                            discovered.push(DiscoveredDevice {
                                id,
                                name,
                                os,
                                arch,
                                ip_address: ip,
                                port,
                                is_paired: false,
                            });
                        }
                    }
                }
            }
        }
    }

    let mut app = state.lock().unwrap();
    app.discovered_devices = discovered.clone();
    discovered
}

#[tauri::command]
pub fn add_manual_device(
    ip_address: String,
    port: Option<u16>,
    name: Option<String>,
    state: State<'_, SharedAppState>,
) -> Result<DeviceInfo, String> {
    let port = port.unwrap_or(48700);
    let ip = ip_address.trim().to_string();
    if ip.is_empty() {
        return Err("IP address cannot be empty".into());
    }

    let mut app = state.lock().unwrap();
    let dev_name = name.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| format!("Laptop ({})", ip));
    let dev_id = dev_name.replace(' ', "_");

    if app.devices.iter().any(|d| d.ip_address == ip || d.id == dev_id) {
        return Err(format!("Device with IP {} or ID {} already exists in known devices", ip, dev_id));
    }

    let new_device = DeviceInfo {
        id: dev_id.clone(),
        name: dev_name.clone(),
        os: "Remote Machine".into(),
        arch: "x64".into(),
        ip_address: ip,
        port,
        is_local: false,
        is_active: false,
        is_connected: true,
        latency_ms: 0.85,
        capabilities: vec!["mouse_capture".into(), "mouse_injection".into(), "edge_detection".into()],
    };

    app.devices.push(new_device.clone());
    app.topology.add_device(ScreenLayout {
        device_id: dev_id.clone(),
        name: dev_name.clone(),
        bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
    });

    app.add_log("INFO", "friday_network::pairing", &format!("Device {} added ({}) and connected to ring", dev_name, new_device.ip_address));
    app.persist_config();
    Ok(new_device)
}

#[tauri::command]
pub fn pair_device(device_id: String, state: State<'_, SharedAppState>) -> Result<DeviceInfo, String> {
    let mut app = state.lock().unwrap();
    let idx = app.discovered_devices.iter().position(|d| d.id == device_id)
        .ok_or_else(|| format!("Device {} not found in discovered list", device_id))?;

    let discovered = app.discovered_devices.remove(idx);
    let new_device = DeviceInfo {
        id: discovered.id.clone(),
        name: discovered.name.clone(),
        os: discovered.os.clone(),
        arch: discovered.arch.clone(),
        ip_address: discovered.ip_address.clone(),
        port: discovered.port,
        is_local: false,
        is_active: false,
        is_connected: true,
        latency_ms: 0.95,
        capabilities: vec!["mouse_capture".into(), "mouse_injection".into(), "edge_detection".into()],
    };

    app.devices.push(new_device.clone());
    app.topology.add_device(ScreenLayout {
        device_id: discovered.id.clone(),
        name: discovered.name.clone(),
        bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
    });

    app.add_log("INFO", "friday_network::pairing", &format!("Device {} successfully paired and added to circular ring", discovered.name));
    app.persist_config();
    Ok(new_device)
}

#[tauri::command]
pub fn unpair_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if device_id == app.local_device_id {
        return Err("Cannot unpair the local host machine".into());
    }

    app.devices.retain(|d| d.id != device_id);
    app.topology.remove_device(&device_id);

    if app.active_device_id == device_id {
        let local_id = app.local_device_id.clone();
        app.active_device_id = local_id.clone();
        app.topology.set_active_device(&local_id);
    }

    app.add_log("INFO", "friday_network::pairing", &format!("Device {} unpaired", device_id));
    app.persist_config();
    Ok(())
}


#[tauri::command]
pub fn connect_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if let Some(dev) = app.devices.iter_mut().find(|d| d.id == device_id) {
        dev.is_connected = true;
        app.add_log("INFO", "friday_network::transport", &format!("Connected to {}", device_id));
        Ok(())
    } else {
        Err(format!("Device {} not found", device_id))
    }
}

#[tauri::command]
pub fn disconnect_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if let Some(dev) = app.devices.iter_mut().find(|d| d.id == device_id) {
        dev.is_connected = false;
        if app.active_device_id == device_id {
            let local_id = app.local_device_id.clone();
            app.active_device_id = local_id.clone();
            app.topology.set_active_device(&local_id);
        }
        app.add_log("INFO", "friday_network::transport", &format!("Disconnected from {}", device_id));
        Ok(())
    } else {
        Err(format!("Device {} not found", device_id))
    }
}

#[tauri::command]
pub fn get_topology(state: State<'_, SharedAppState>) -> TopologyDto {
    let app = state.lock().unwrap();
    let ring = app.topology.ring.clone();
    let mut links = Vec::new();

    if ring.len() >= 2 {
        for i in 0..ring.len() {
            let next_idx = (i + 1) % ring.len();
            links.push(TopologyLinkDto {
                source: ring[i].clone(),
                target: ring[next_idx].clone(),
                edge: "Right".into(),
            });
        }
    }

    TopologyDto {
        ring: ring.clone(),
        active_device: app.active_device_id.clone(),
        devices: app.devices.clone(),
        links,
    }
}

#[tauri::command]
pub fn set_topology(ring: Vec<String>, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if ring.is_empty() {
        return Err("Topology ring cannot be empty".into());
    }

    // Verify all devices exist
    for id in &ring {
        if !app.devices.iter().any(|d| &d.id == id) {
            return Err(format!("Device ID {} does not exist in known devices", id));
        }
    }

    app.topology.set_ring(ring.clone());
    if !ring.contains(&app.active_device_id) {
        app.active_device_id = ring[0].clone();
        app.topology.set_active_device(&ring[0]);
    }

    app.add_log("INFO", "friday_core::topology", &format!("Circular ring topology updated: {}", ring.join(" → ")));
    app.persist_config();
    Ok(())
}

#[tauri::command]
pub fn reorder_ring(ring: Vec<String>, state: State<'_, SharedAppState>) -> Result<(), String> {
    set_topology(ring, state)
}

#[tauri::command]
pub fn add_ring_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if !app.devices.iter().any(|d| d.id == device_id) {
        return Err(format!("Device {} does not exist in paired devices", device_id));
    }
    if !app.topology.ring.contains(&device_id) {
        app.topology.ring.push(device_id.clone());
    }
    app.add_log("INFO", "friday_core::topology", &format!("Added {} to circular ring", device_id));
    app.persist_config();
    Ok(())
}

#[tauri::command]
pub fn remove_ring_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if app.topology.ring.len() <= 1 {
        return Err("Cannot remove last device from circular ring".into());
    }
    app.topology.remove_device(&device_id);
    if app.active_device_id == device_id {
        let local_id = app.local_device_id.clone();
        app.active_device_id = local_id.clone();
        app.topology.set_active_device(&local_id);
    }
    app.add_log("INFO", "friday_core::topology", &format!("Removed {} from circular ring", device_id));
    app.persist_config();
    Ok(())
}

#[tauri::command]
pub fn get_telemetry(state: State<'_, SharedAppState>) -> TelemetryDto {
    let app = state.lock().unwrap();
    let uptime = app.start_time.elapsed().as_secs();
    let mut t = app.telemetry.clone();
    t.uptime_seconds = uptime;
    t
}

#[tauri::command]
pub fn run_diagnostics(state: State<'_, SharedAppState>) -> DiagnosticReportDto {
    let app = state.lock().unwrap();
    let status = app.get_engine_status();
    run_system_diagnostics(
        &status.state,
        &app.active_device_id,
        &app.topology.ring,
        app.telemetry.latency_ms,
        app.telemetry.packet_loss_pct,
    )
}

#[tauri::command]
pub fn run_benchmark() -> BenchmarkReportDto {
    run_core_benchmark()
}

#[tauri::command]
pub fn get_settings(state: State<'_, SharedAppState>) -> SettingsDto {
    let app = state.lock().unwrap();
    app.settings.clone()
}

#[tauri::command]
pub fn save_settings(settings: SettingsDto, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    ConfigManager::save(&settings)?;
    app.settings = settings;
    app.add_log("INFO", "friday_core::config", "Configuration saved successfully");
    Ok(())
}

#[tauri::command]
pub fn get_platform_capabilities() -> PlatformCapabilitiesDto {
    PlatformCapabilitiesDto {
        mouse_capture: true,
        mouse_injection: true,
        keyboard_capture: false, // In development
        keyboard_injection: false, // In development
        clipboard: false, // Coming soon
        file_transfer: false, // Coming soon
        screen_information: true,
        multi_monitor: true,
        edge_detection: true,
        system_tray: true,
        startup: true,
        notifications: true,
    }
}

#[tauri::command]
pub fn get_platform_permissions() -> PlatformPermissionsDto {
    let platform = std::env::consts::OS.to_string();

    if cfg!(target_os = "linux") {
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
        let display_server = if is_wayland { "Wayland".to_string() } else { "X11".to_string() };
        let input_backend = if is_wayland { "Wayland (Portal / evdev)".to_string() } else { "X11 (XTest)".to_string() };

        let (warn, instr) = if is_wayland {
            (
                Some("Wayland security policies restrict global mouse grab by default.".into()),
                Some("Use X11 session or configure RemoteDesktop portal permission for uninhibited edge handoff.".into())
            )
        } else {
            (None, None)
        };

        PlatformPermissionsDto {
            platform,
            display_server,
            input_backend,
            has_input_permission: true,
            permission_warning: warn,
            permission_instructions: instr,
        }
    } else if cfg!(target_os = "macos") {
        PlatformPermissionsDto {
            platform,
            display_server: "Quartz Compositor".into(),
            input_backend: "CoreGraphics EventTap".into(),
            has_input_permission: true,
            permission_warning: None,
            permission_instructions: Some("Grant Accessibility and Input Monitoring permissions in System Settings > Privacy & Security.".into()),
        }
    } else {
        PlatformPermissionsDto {
            platform,
            display_server: "Desktop Window Manager".into(),
            input_backend: "Win32 SendInput / Hook".into(),
            has_input_permission: true,
            permission_warning: None,
            permission_instructions: None,
        }
    }
}

#[tauri::command]
pub fn open_permission_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
    Ok(())
}

#[tauri::command]
pub fn get_logs(state: State<'_, SharedAppState>) -> Vec<LogEntryDto> {
    let app = state.lock().unwrap();
    app.logs.clone()
}
