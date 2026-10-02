use crate::config::ConfigManager;
use crate::diagnostics::{run_core_benchmark, run_system_diagnostics};
use crate::state::SharedAppState;
use crate::types::{
    BenchmarkReportDto, DeviceInfo, DiagnosticReportDto, DiscoveredDevice, EngineStatus,
    LogEntryDto, PlatformCapabilitiesDto, PlatformPermissionsDto, SettingsDto, TelemetryDto,
    TopologyDto, TopologyLinkDto,
};
use friday_core::{DisplayBounds, ScreenLayout};
use tauri::State;

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
    app.add_log(
        "INFO",
        "friday_core::engine",
        "Engine started by user request",
    );
    Ok(app.get_engine_status())
}

#[tauri::command]
pub fn stop_engine(state: State<'_, SharedAppState>) -> Result<EngineStatus, String> {
    let mut app = state.lock().unwrap();
    app.engine_running = false;
    app.engine_paused = false;
    app.add_log(
        "INFO",
        "friday_core::engine",
        "Engine stopped by user request",
    );
    Ok(app.get_engine_status())
}

#[tauri::command]
pub fn pause_engine(state: State<'_, SharedAppState>) -> Result<EngineStatus, String> {
    let mut app = state.lock().unwrap();
    app.engine_paused = !app.engine_paused;
    let msg = if app.engine_paused {
        "Engine paused"
    } else {
        "Engine resumed"
    };
    app.add_log("INFO", "friday_core::engine", msg);
    Ok(app.get_engine_status())
}

#[tauri::command]
pub fn get_active_device(state: State<'_, SharedAppState>) -> String {
    let app = state.lock().unwrap();
    app.active_device_id.clone()
}

#[tauri::command]
pub fn switch_active_device(
    device_id: String,
    state: State<'_, SharedAppState>,
) -> Result<(), String> {
    let mut app = state.lock().unwrap();

    let dev = app
        .devices
        .iter()
        .find(|d| d.id == device_id || d.ip_address == device_id)
        .cloned();

    let target_id = if let Some(ref d) = dev {
        if !app.topology.ring.contains(&d.id) {
            app.topology.add_device(friday_core::ScreenLayout {
                device_id: d.id.clone(),
                name: d.name.clone(),
                bounds: friday_core::DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            });
        }
        d.id.clone()
    } else {
        device_id.clone()
    };

    if !app.topology.ring.contains(&target_id) {
        return Err(format!(
            "Device {} is not in the circular ring topology",
            target_id
        ));
    }
    app.active_device_id = target_id.clone();
    app.topology.set_active_device(&target_id);
    for d in &mut app.devices {
        d.is_active = d.id == target_id;
    }
    app.add_log(
        "INFO",
        "friday_core::ownership",
        &format!("Ownership manually switched to {}", target_id),
    );
    app.persist_config();
    drop(app);
    crate::engine::notify_active_device_changed(&state, &target_id);
    Ok(())
}

#[tauri::command]
pub fn get_devices(state: State<'_, SharedAppState>) -> Vec<DeviceInfo> {
    let app = state.lock().unwrap();
    app.devices.clone()
}

#[tauri::command]
pub fn discover_devices(state: State<'_, SharedAppState>) -> Vec<DiscoveredDevice> {
    crate::discovery::scan_local_subnet(state.inner().clone())
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
    let dev_name = name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("Laptop ({})", ip));
    let dev_id = dev_name.replace(' ', "_");

    if app
        .devices
        .iter()
        .any(|d| d.ip_address == ip || d.id == dev_id)
    {
        return Err(format!(
            "Device with IP {} or ID {} already exists in known devices",
            ip, dev_id
        ));
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
        capabilities: vec![
            "mouse_capture".into(),
            "mouse_injection".into(),
            "edge_detection".into(),
        ],
    };

    app.devices.push(new_device.clone());
    app.topology.add_device(ScreenLayout {
        device_id: dev_id.clone(),
        name: dev_name.clone(),
        bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
    });

    app.add_log(
        "INFO",
        "friday_network::pairing",
        &format!(
            "Device {} added ({}) and connected to ring",
            dev_name, new_device.ip_address
        ),
    );
    app.persist_config();
    Ok(new_device)
}

#[tauri::command]
pub fn pair_device(
    device_id: String,
    state: State<'_, SharedAppState>,
) -> Result<DeviceInfo, String> {
    let mut app = state.lock().unwrap();
    let idx = app
        .discovered_devices
        .iter()
        .position(|d| d.id == device_id)
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
        capabilities: vec![
            "mouse_capture".into(),
            "mouse_injection".into(),
            "edge_detection".into(),
        ],
    };

    app.devices.push(new_device.clone());
    app.topology.add_device(ScreenLayout {
        device_id: discovered.id.clone(),
        name: discovered.name.clone(),
        bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
    });

    app.add_log(
        "INFO",
        "friday_network::pairing",
        &format!(
            "Device {} successfully paired and added to circular ring",
            discovered.name
        ),
    );
    app.persist_config();
    Ok(new_device)
}

#[tauri::command]
pub fn unpair_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();

    let dev = app
        .devices
        .iter()
        .find(|d| d.id == device_id || d.ip_address == device_id)
        .cloned();

    let (target_id, target_name) = if let Some(ref d) = dev {
        if d.id == app.local_device_id || d.is_local {
            return Err("Cannot unpair the local host machine".into());
        }
        (d.id.clone(), d.name.clone())
    } else {
        if device_id == app.local_device_id {
            return Err("Cannot unpair the local host machine".into());
        }
        (device_id.clone(), device_id.clone())
    };

    app.devices.retain(|d| {
        d.id != target_id
            && d.ip_address != target_id
            && d.id != device_id
            && d.ip_address != device_id
    });
    app.topology.remove_device(&target_id);
    app.topology.remove_device(&device_id);

    if app.active_device_id == target_id || app.active_device_id == device_id {
        let local_id = app.local_device_id.clone();
        app.active_device_id = local_id.clone();
        app.topology.set_active_device(&local_id);
    }

    let target_ip = dev
        .as_ref()
        .map(|d| d.ip_address.clone())
        .unwrap_or_else(|| {
            if device_id.parse::<std::net::IpAddr>().is_ok() {
                device_id.clone()
            } else {
                String::new()
            }
        });

    if !target_ip.is_empty() {
        let dest = format!("{}:{}", target_ip, crate::pairing::PAIR_REQUEST_PORT);
        let msg = format!("FRIDAY_UNPAIR:{}", app.local_device_id);
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            for _ in 0..3 {
                let _ = socket.send_to(msg.as_bytes(), &dest);
            }
        }
    }

    app.add_log(
        "INFO",
        "friday_network::pairing",
        &format!("Device {} ({}) unpaired", target_name, target_id),
    );
    app.persist_config();
    Ok(())
}

#[tauri::command]
pub fn connect_device(device_id: String, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    if let Some(dev) = app
        .devices
        .iter_mut()
        .find(|d| d.id == device_id || d.ip_address == device_id)
    {
        dev.is_connected = true;
        let name = dev.name.clone();
        app.add_log(
            "INFO",
            "friday_network::transport",
            &format!("Connected to {}", name),
        );
        app.persist_config();
        drop(app);
        crate::engine::notify_device_connected(&state, &device_id);
        Ok(())
    } else {
        Err(format!("Device {} not found", device_id))
    }
}

#[tauri::command]
pub fn disconnect_device(
    device_id: String,
    state: State<'_, SharedAppState>,
) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    let found_id = app
        .devices
        .iter()
        .find(|d| d.id == device_id || d.ip_address == device_id)
        .map(|d| d.id.clone());

    if let Some(id) = found_id {
        if let Some(dev) = app.devices.iter_mut().find(|d| d.id == id) {
            dev.is_connected = false;
        }
        if app.active_device_id == id {
            let local_id = app.local_device_id.clone();
            app.active_device_id = local_id.clone();
            app.topology.set_active_device(&local_id);
        }
        app.add_log(
            "INFO",
            "friday_network::transport",
            &format!("Disconnected from {}", id),
        );
        app.persist_config();
        drop(app);
        crate::engine::notify_device_disconnected(&state, &id);
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
        is_host: app.is_host,
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

    app.add_log(
        "INFO",
        "friday_core::topology",
        &format!("Circular ring topology updated: {}", ring.join(" → ")),
    );
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
        return Err(format!(
            "Device {} does not exist in paired devices",
            device_id
        ));
    }
    if !app.topology.ring.contains(&device_id) {
        app.topology.ring.push(device_id.clone());
    }
    app.add_log(
        "INFO",
        "friday_core::topology",
        &format!("Added {} to circular ring", device_id),
    );
    app.persist_config();
    Ok(())
}

#[tauri::command]
pub fn remove_ring_device(
    device_id: String,
    state: State<'_, SharedAppState>,
) -> Result<(), String> {
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
    app.add_log(
        "INFO",
        "friday_core::topology",
        &format!("Removed {} from circular ring", device_id),
    );
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
pub fn save_settings(
    settings: SettingsDto,
    state: State<'_, SharedAppState>,
) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    ConfigManager::save(&settings)?;
    app.settings = settings;
    app.add_log(
        "INFO",
        "friday_core::config",
        "Configuration saved successfully",
    );
    Ok(())
}

#[tauri::command]
pub fn get_platform_capabilities() -> PlatformCapabilitiesDto {
    PlatformCapabilitiesDto {
        mouse_capture: true,
        mouse_injection: true,
        keyboard_capture: false,   // In development
        keyboard_injection: false, // In development
        clipboard: false,          // Coming soon
        file_transfer: false,      // Coming soon
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
        let display_server = if is_wayland {
            "Wayland".to_string()
        } else {
            "X11".to_string()
        };
        let input_backend = if is_wayland {
            "Wayland (Portal / evdev)".to_string()
        } else {
            "X11 (XTest)".to_string()
        };

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

// ── Pairing handshake commands ─────────────────────────────────────────────────

/// Initiate a pairing request to a discovered remote device.
/// Sends the PIN over UDP to the remote machine's pairing port (48702),
/// then BLOCKS (up to 30 seconds) waiting for the remote user to Accept/Reject.
/// Returns Ok(true) if accepted, Ok(false) if rejected/timeout.
///
/// NOTE: This command MUST be called from the frontend with a long-enough timeout.
/// The frontend generates the PIN, shows it to the local user, then calls this.
/// The remote machine's GUI shows a corresponding incoming-request dialog.
#[tauri::command]
pub fn initiate_pairing(
    device_id: String,
    pin: String,
    target_ip: Option<String>,
    state: State<'_, SharedAppState>,
) -> Result<bool, String> {
    app_log(
        &state,
        "INFO",
        "friday_network::pairing",
        &format!(
            "Initiating pairing: target='{}', hint_ip={:?}, PIN={}",
            device_id, target_ip, pin
        ),
    );

    let (local_id, local_name, resolved_target_ip, device_name) = {
        let app = state.lock().unwrap();

        // 1. Try to find the device in discovered list by id or by IP
        let dev = app
            .discovered_devices
            .iter()
            .find(|d| d.id == device_id || d.ip_address == device_id)
            .cloned()
            .or_else(|| {
                // 2. Also search in paired devices list
                app.devices
                    .iter()
                    .find(|d| d.id == device_id || d.ip_address == device_id)
                    .map(|d| crate::types::DiscoveredDevice {
                        id: d.id.clone(),
                        name: d.name.clone(),
                        os: d.os.clone(),
                        arch: d.arch.clone(),
                        ip_address: d.ip_address.clone(),
                        port: d.port,
                        is_paired: true,
                    })
            });

        let resolved_ip = if let Some(ref d) = dev {
            d.ip_address.clone()
        } else if let Some(ref ip) = target_ip {
            ip.clone()
        } else if device_id.parse::<std::net::IpAddr>().is_ok() {
            device_id.clone()
        } else {
            let err = format!(
                "Device '{}' not found in discovered list. Please click 'Scan Subnet' or specify IP address.",
                device_id
            );
            app_log(&state, "WARN", "friday_network::pairing", &err);
            return Err(err);
        };

        let resolved_name = dev
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("Remote ({})", resolved_ip));

        let local_id = app.local_device_id.clone();
        let local_name = app
            .devices
            .first()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| local_id.clone());

        (local_id, local_name, resolved_ip, resolved_name)
    };

    let target_ip = resolved_target_ip;

    // Pre-flight: quick 3s probe to confirm FRIDAY is reachable on pairing port.
    // Returns a clear error immediately instead of waiting 30s on timeout.
    crate::pairing::probe_pairing_port(&target_ip).inspect_err(|e| {
        app_log(&state, "WARN", "friday_network::pairing", e);
    })?;

    app_log(
        &state,
        "INFO",
        "friday_network::pairing",
        &format!(
            "🔍 Probe OK — {} is reachable on port {}. Sending PAIR_REQUEST now...",
            target_ip,
            crate::pairing::PAIR_REQUEST_PORT
        ),
    );

    // This blocks the Tauri command thread for up to 30 seconds.
    // Tauri async commands run on a thread pool so the GUI stays responsive.
    let accepted =
        crate::pairing::send_pairing_request(&target_ip, &local_id, &local_name, &pin, &state)?;

    if accepted {
        // Promote discovered → paired device automatically
        let mut app = state.lock().unwrap();
        // Initiator of pairing is the HOST controller
        app.is_host = true;
        let discovered = app
            .discovered_devices
            .iter()
            .position(|d| d.id == device_id || d.ip_address == target_ip)
            .map(|idx| app.discovered_devices.remove(idx));

        if let Some(d) = discovered {
            let new_device = crate::types::DeviceInfo {
                id: d.id.clone(),
                name: d.name.clone(),
                os: d.os.clone(),
                arch: d.arch.clone(),
                ip_address: d.ip_address.clone(),
                port: d.port,
                is_local: false,
                is_active: false,
                is_connected: true,
                latency_ms: 0.0,
                capabilities: vec![
                    "mouse_capture".into(),
                    "mouse_injection".into(),
                    "edge_detection".into(),
                ],
            };
            app.devices.push(new_device.clone());
            app.topology.add_device(friday_core::ScreenLayout {
                device_id: d.id.clone(),
                name: d.name.clone(),
                bounds: friday_core::DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            });
            app.add_log(
                "INFO",
                "friday_network::pairing",
                &format!("Device {} paired successfully (PIN confirmed)", d.name),
            );
            app.persist_config();
        } else if !app
            .devices
            .iter()
            .any(|d| d.ip_address == target_ip || d.id == device_id)
        {
            let new_device = crate::types::DeviceInfo {
                id: device_id.clone(),
                name: device_name.clone(),
                os: "Linux".into(),
                arch: "x86_64".into(),
                ip_address: target_ip.clone(),
                port: 48700,
                is_local: false,
                is_active: false,
                is_connected: true,
                latency_ms: 0.0,
                capabilities: vec![
                    "mouse_capture".into(),
                    "mouse_injection".into(),
                    "edge_detection".into(),
                ],
            };
            app.devices.push(new_device.clone());
            app.topology.add_device(friday_core::ScreenLayout {
                device_id: device_id.clone(),
                name: device_name.clone(),
                bounds: friday_core::DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            });
            app.add_log(
                "INFO",
                "friday_network::pairing",
                &format!("Device {} paired successfully (PIN confirmed)", device_name),
            );
            app.persist_config();
        } else {
            if let Some(dev) = app
                .devices
                .iter_mut()
                .find(|d| d.ip_address == target_ip || d.id == device_id)
            {
                dev.is_connected = true;
            }
            app.add_log(
                "INFO",
                "friday_network::pairing",
                &format!("Device {} paired and connected", device_name),
            );
            app.persist_config();
        }
    } else {
        app_log(
            &state,
            "WARN",
            "friday_network::pairing",
            &format!("Pairing with {} was rejected or timed out", device_id),
        );
    }

    Ok(accepted)
}

/// Poll for incoming pair requests on this machine.
/// The remote machine's GUI calls this every 2 seconds.
/// Returns a list of PendingPairRequest objects.
#[tauri::command]
pub fn get_pending_pair_requests() -> Vec<crate::pairing::PendingPairRequest> {
    crate::pairing::get_pending_requests()
}

/// Accept or reject an incoming pair request.
/// Called by the remote machine's user when they see the incoming pairing modal.
/// If accepted, the device is also added to this machine's paired list.
#[tauri::command]
pub fn respond_to_pair_request(
    pin: String,
    accept: bool,
    state: State<'_, SharedAppState>,
) -> Result<(), String> {
    let request = {
        let requests = crate::pairing::get_pending_requests();
        requests
            .into_iter()
            .find(|r| r.pin == pin)
            .ok_or_else(|| format!("No pending pair request with PIN {}", pin))?
    };

    // Send network reply
    crate::pairing::respond_to_request(&request, accept)?;

    if accept {
        // Add the initiator device to OUR paired list too (symmetric pairing)
        let mut app = state.lock().unwrap();
        // Since this machine accepted a pairing request from a host, this machine is a CLIENT screen!
        app.is_host = false;
        let already_known = app.devices.iter().any(|d| d.ip_address == request.from_ip);
        if !already_known {
            let new_device = crate::types::DeviceInfo {
                id: request.from_id.clone(),
                name: request.from_name.clone(),
                os: "Remote Machine".into(),
                arch: "x64".into(),
                ip_address: request.from_ip.clone(),
                port: 48700,
                is_local: false,
                is_active: false,
                is_connected: true,
                latency_ms: 0.0,
                capabilities: vec![
                    "mouse_capture".into(),
                    "mouse_injection".into(),
                    "edge_detection".into(),
                ],
            };
            app.devices.push(new_device);
            app.topology.add_device(friday_core::ScreenLayout {
                device_id: request.from_id.clone(),
                name: request.from_name.clone(),
                bounds: friday_core::DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            });
            app.add_log(
                "INFO",
                "friday_network::pairing",
                &format!(
                    "Accepted pairing from {} — device added to ring (Client Mode)",
                    request.from_name
                ),
            );
            app.persist_config();
        } else {
            app.persist_config();
        }
    } else {
        let app = state.lock().unwrap();
        drop(app); // release lock before log
        app_log(
            &state,
            "INFO",
            "friday_network::pairing",
            &format!(
                "Rejected pairing request from {} (PIN {})",
                request.from_name, pin
            ),
        );
    }

    Ok(())
}

#[tauri::command]
pub fn set_device_role(is_host: bool, state: State<'_, SharedAppState>) -> Result<(), String> {
    let mut app = state.lock().unwrap();
    app.is_host = is_host;
    app.add_log(
        "INFO",
        "friday_core::engine",
        &format!(
            "Device role switched to: {}",
            if is_host { "Host (Controller)" } else { "Client (Receiver)" }
        ),
    );
    app.persist_config();
    Ok(())
}

// Helper: add a log entry without holding the lock across an await
fn app_log(state: &State<'_, SharedAppState>, level: &str, target: &str, msg: &str) {
    if let Ok(mut app) = state.lock() {
        app.add_log(level, target, msg);
    }
}
