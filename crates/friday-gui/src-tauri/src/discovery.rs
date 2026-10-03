use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::state::{detect_local_ip, detect_os_info, SharedAppState};
use crate::types::DiscoveredDevice;

pub const DISCOVERY_PORT: u16 = 48701;

/// Background responder that listens for discovery pings and announces this node
pub fn start_discovery_service(state: SharedAppState, stop_flag: Arc<AtomicBool>) {
    thread::spawn(move || {
        let (local_id, os, arch, port, local_ip) = {
            let app = state.lock().unwrap();
            let (os, arch) = detect_os_info();
            (
                app.local_device_id.clone(),
                os,
                arch,
                app.settings.peer_port,
                app.devices
                    .first()
                    .map(|d| d.ip_address.clone())
                    .unwrap_or_else(detect_local_ip),
            )
        };

        // Bind discovery listener socket
        let socket = match UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)) {
            Ok(s) => s,
            Err(e) => {
                debug!("Discovery responder port {} in use or unavailable: {}. Attempting ephemeral bind.", DISCOVERY_PORT, e);
                match UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => s,
                    Err(e2) => {
                        warn!("Failed to bind any UDP socket for discovery: {}", e2);
                        return;
                    }
                }
            }
        };

        let _ = socket.set_broadcast(true);
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));

        let initial_name = {
            let app = state.lock().unwrap();
            app.local_display_name.clone()
        };

        // Broadcast initial presence announcement to local subnet
        let announce_msg = format!(
            "FRIDAY_NODE_ANNOUNCE:{}:{}:{}:{}:{}",
            local_id, initial_name, os, arch, port
        );
        broadcast_discovery_packet(&socket, announce_msg.as_bytes(), &local_ip, DISCOVERY_PORT);

        info!(
            "FRIDAY Discovery Service active on UDP port {}",
            DISCOVERY_PORT
        );

        let mut buf = [0u8; 512];
        let mut last_heartbeat = Instant::now();

        while !stop_flag.load(Ordering::Relaxed) {
            // Periodic broadcast announce every 8 seconds
            if last_heartbeat.elapsed() > Duration::from_secs(8) {
                let current_name = {
                    let app = state.lock().unwrap();
                    app.local_display_name.clone()
                };
                let msg = format!(
                    "FRIDAY_NODE_ANNOUNCE:{}:{}:{}:{}:{}",
                    local_id, current_name, os, arch, port
                );
                broadcast_discovery_packet(&socket, msg.as_bytes(), &local_ip, DISCOVERY_PORT);
                last_heartbeat = Instant::now();
            }

            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    let msg = String::from_utf8_lossy(&buf[..len]);

                    if msg.starts_with("FRIDAY_DISCOVERY_PING:") {
                        let requester_id = msg.trim_start_matches("FRIDAY_DISCOVERY_PING:").trim();
                        if requester_id != local_id {
                            let current_name = {
                                let app = state.lock().unwrap();
                                app.local_display_name.clone()
                            };
                            let reply = format!(
                                "FRIDAY_NODE:{}:{}:{}:{}:{}",
                                local_id, current_name, os, arch, port
                            );
                            let _ = socket.send_to(reply.as_bytes(), src);
                        }
                    } else if msg.starts_with("FRIDAY_NODE_ANNOUNCE:") {
                        let parts: Vec<&str> = msg.split(':').collect();
                        if parts.len() >= 6 && parts[1] != local_id {
                            let peer_id = parts[1].to_string();
                            let peer_name = parts[2].to_string();
                            let peer_os = parts[3].to_string();
                            let peer_arch = parts[4].to_string();
                            let peer_port = parts[5].parse::<u16>().unwrap_or(48700);
                            let peer_ip = src.ip().to_string();

                            let mut app = state.lock().unwrap();
                            // If device is already paired, update its display name from latest announcement
                            if let Some(dev) = app
                                .devices
                                .iter_mut()
                                .find(|d| d.id == peer_id || d.ip_address == peer_ip)
                            {
                                if !peer_name.is_empty() && !dev.is_local {
                                    dev.name = peer_name.clone();
                                }
                            }
                            let already_paired = app
                                .devices
                                .iter()
                                .any(|d| d.id == peer_id || d.ip_address == peer_ip);
                            let already_discovered =
                                app.discovered_devices.iter().any(|d| d.id == peer_id);

                            if !already_paired && !already_discovered {
                                app.discovered_devices.push(DiscoveredDevice::new(
                                    peer_id, peer_name, peer_os, peer_arch, peer_ip, peer_port,
                                    false,
                                ));
                            } else if let Some(disc) =
                                app.discovered_devices.iter_mut().find(|d| d.id == peer_id)
                            {
                                if !peer_name.is_empty() {
                                    disc.name = peer_name;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Timeout hit, continue loop
                }
            }
        }
    });
}

/// Broadcast a packet to 255.255.255.255 and the local subnet broadcast address
pub fn broadcast_discovery_packet(
    socket: &UdpSocket,
    data: &[u8],
    local_ip: &str,
    target_port: u16,
) {
    let global_bcast = format!("255.255.255.255:{}", target_port);
    let _ = socket.send_to(data, &global_bcast);

    if let Some(subnet_bcast) = compute_subnet_broadcast(local_ip, target_port) {
        let _ = socket.send_to(data, &subnet_bcast);
    }
}

/// Compute subnet broadcast address for IPv4 (e.g. 192.168.1.11:48701 -> 192.168.1.255:48701)
pub fn compute_subnet_broadcast(ip_str: &str, port: u16) -> Option<String> {
    let parts: Vec<&str> = ip_str.split('.').collect();
    if parts.len() == 4 {
        Some(format!(
            "{}.{}.{}.255:{}",
            parts[0], parts[1], parts[2], port
        ))
    } else {
        None
    }
}

/// Active scan executed when user clicks "Scan Subnet"
pub fn scan_local_subnet(state: SharedAppState) -> Vec<DiscoveredDevice> {
    let (local_id, local_ip) = {
        let app = state.lock().unwrap();
        (
            app.local_device_id.clone(),
            app.devices
                .first()
                .map(|d| d.ip_address.clone())
                .unwrap_or_else(detect_local_ip),
        )
    };

    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to bind client socket for subnet scan: {}", e);
            return Vec::new();
        }
    };

    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(600)));

    let ping = format!("FRIDAY_DISCOVERY_PING:{}", local_id);
    broadcast_discovery_packet(&socket, ping.as_bytes(), &local_ip, DISCOVERY_PORT);

    let mut found = Vec::new();
    let mut buf = [0u8; 512];
    let start = Instant::now();

    while start.elapsed() < Duration::from_millis(600) {
        if let Ok((len, src)) = socket.recv_from(&mut buf) {
            let text = String::from_utf8_lossy(&buf[..len]);
            if text.starts_with("FRIDAY_NODE:") || text.starts_with("FRIDAY_NODE_ANNOUNCE:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() >= 6 && parts[1] != local_id {
                    let peer_id = parts[1].to_string();
                    let peer_name = parts[2].to_string();
                    let peer_os = parts[3].to_string();
                    let peer_arch = parts[4].to_string();
                    let peer_port = parts[5].parse::<u16>().unwrap_or(48700);
                    let peer_ip = src.ip().to_string();

                    if !found.iter().any(|d: &DiscoveredDevice| d.id == peer_id) {
                        found.push(DiscoveredDevice::new(
                            peer_id, peer_name, peer_os, peer_arch, peer_ip, peer_port, false,
                        ));
                    }
                }
            }
        }
    }

    let mut app = state.lock().unwrap();
    // Filter out already paired devices
    found.retain(|f| {
        !app.devices
            .iter()
            .any(|d| d.id == f.id || d.ip_address == f.ip_address)
    });
    app.discovered_devices = found.clone();
    found
}
