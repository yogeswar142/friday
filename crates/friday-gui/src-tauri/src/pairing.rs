/// FRIDAY Pairing Protocol (v3 — ephemeral reply port, no port conflicts)
///
/// Protocol:
///   Port 48702 = remote listens for PAIR_REQUEST
///   Reply port = random ephemeral (OS-assigned), included in the request message
///
/// Full flow:
///   [Initiator]
///     1. Bind reply socket on 0.0.0.0:0 → OS assigns ephemeral port (e.g. 54321)
///     2. Send to <remote_ip>:48702:
///        FRIDAY_PAIR_REQUEST:<pin>:<from_id>:<from_name>:<reply_port>
///     3. Wait up to 30s on the ephemeral port for:
///        FRIDAY_PAIR_ACCEPT:<pin>  or  FRIDAY_PAIR_REJECT:<pin>
///
///   [Remote machine — pairing responder]
///     1. Receives FRIDAY_PAIR_REQUEST on port 48702
///     2. Parses <pin>, <from_id>, <from_name>, <reply_port>
///     3. Stores PendingPairRequest { pin, from_id, from_name, from_ip, reply_port }
///
///   [Remote GUI]
///     1. Polls get_pending_pair_requests every 2s
///     2. Shows incoming modal with PIN to user
///     3. User clicks Accept → respond_to_pair_request(pin, true)
///        → Rust sends FRIDAY_PAIR_ACCEPT:<pin> to <from_ip>:<reply_port>
///     4. User clicks Reject → sends FRIDAY_PAIR_REJECT:<pin>
///
///   [Initiator receives reply]
///     → Ok(true) if ACCEPT, Ok(false) if REJECT or timeout
///     → pair_device() called automatically on accept
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tracing::{info, warn};

use crate::state::SharedAppState;

/// Port the remote device listens on for incoming pair requests
pub const PAIR_REQUEST_PORT: u16 = 48702;

/// A pending incoming pair request on this machine
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingPairRequest {
    pub pin: String,
    pub from_id: String,
    pub from_name: String,
    pub from_ip: String,
    /// Ephemeral port the initiator is listening on for our reply
    pub reply_port: u16,
}

/// Global list of pending incoming requests (populated by responder thread)
pub static PENDING_REQUESTS: OnceLock<Mutex<Vec<PendingPairRequest>>> = OnceLock::new();

/// Event-driven notification channel: responder sends on TX, Tauri setup hook receives on RX.
/// This eliminates 2-second polling for pair requests.
static PAIR_NOTIFY_TX: OnceLock<Mutex<Sender<PendingPairRequest>>> = OnceLock::new();
static PAIR_NOTIFY_RX: OnceLock<Mutex<Receiver<PendingPairRequest>>> = OnceLock::new();

fn init_notify_channel() {
    // Initialize once; subsequent calls are no-ops due to OnceLock semantics.
    PAIR_NOTIFY_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<PendingPairRequest>();
        PAIR_NOTIFY_RX.get_or_init(|| Mutex::new(rx));
        Mutex::new(tx)
    });
    // Ensure RX side is also initialized (needed if called from the receiver side first)
    PAIR_NOTIFY_RX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<PendingPairRequest>();
        PAIR_NOTIFY_TX.get_or_init(|| Mutex::new(tx));
        Mutex::new(rx)
    });
}

fn pending() -> &'static Mutex<Vec<PendingPairRequest>> {
    PENDING_REQUESTS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Get all pending incoming pair requests (called by GUI poll as fallback)
pub fn get_pending_requests() -> Vec<PendingPairRequest> {
    pending().lock().unwrap().clone()
}

/// Remove a request by PIN (after user responds)
pub fn remove_pending(pin: &str) {
    pending().lock().unwrap().retain(|r| r.pin != pin);
}

/// Block until a new pair request arrives, then return it.
/// Called from the Tauri setup hook thread — never called from GUI/main thread.
/// Returns `None` only if the notification channel is broken (should not happen in production).
pub fn wait_for_next_pair_request() -> Option<PendingPairRequest> {
    init_notify_channel();
    if let Some(rx_mutex) = PAIR_NOTIFY_RX.get() {
        // Block with a timeout so the loop can exit gracefully on shutdown
        rx_mutex
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(30))
            .ok()
    } else {
        None
    }
}

// ── Initiator side ─────────────────────────────────────────────────────────────

/// Quick UDP probe — sends a FRIDAY_PAIR_PROBE and expects FRIDAY_PAIR_PROBE_ACK within 3s.
/// Returns Ok(()) if reachable, Err(message) with actionable text if not.
pub fn probe_pairing_port(target_ip: &str) -> Result<(), String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Cannot bind probe socket: {}", e))?;
    socket.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);

    let dest = format!("{}:{}", target_ip, PAIR_REQUEST_PORT);
    let msg = format!("FRIDAY_PAIR_PROBE:{}", local_port);
    socket
        .send_to(msg.as_bytes(), &dest)
        .map_err(|e| format!("Cannot reach {}:{} — {}", target_ip, PAIR_REQUEST_PORT, e))?;

    let mut buf = [0u8; 64];
    match socket.recv_from(&mut buf) {
        Ok((len, _)) => {
            let reply = String::from_utf8_lossy(&buf[..len]);
            if reply.starts_with("FRIDAY_PAIR_PROBE_ACK") {
                Ok(())
            } else {
                // Got a reply but not the expected ACK — old version maybe
                // Treat as reachable anyway
                Ok(())
            }
        }
        Err(_) => Err(format!(
            "Cannot reach FRIDAY on {}:{} after 3 seconds.\n\
             Make sure:\n\
             1. FRIDAY is open on that machine (Devices tab)\n\
             2. That machine has the latest FRIDAY build (git pull + rebuild)\n\
             3. Windows Firewall: allow UDP port {} inbound for friday-gui.exe",
            target_ip, PAIR_REQUEST_PORT, PAIR_REQUEST_PORT
        )),
    }
}

/// Initiate pairing with a remote FRIDAY device.
/// Uses an OS-assigned ephemeral port for the reply so there are no port conflicts
/// on retries. Blocks the calling thread for up to 30 seconds.
///
/// Returns Ok(true) = accepted, Ok(false) = rejected or timed out.
pub fn send_pairing_request(
    target_ip: &str,
    local_id: &str,
    local_name: &str,
    pin: &str,
    state: &SharedAppState,
) -> Result<bool, String> {
    // Bind single socket on an ephemeral OS-assigned port for both send and receive
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Cannot bind pairing socket: {}", e))?;
    socket.set_read_timeout(Some(Duration::from_secs(30))).ok();

    // Get the actual port the OS assigned
    let reply_port = socket
        .local_addr()
        .map_err(|e| format!("Cannot get local addr: {}", e))?
        .port();

    // Send request — include reply_port so remote knows where to send the ACK
    let dest = format!("{}:{}", target_ip, PAIR_REQUEST_PORT);
    // Format: FRIDAY_PAIR_REQUEST:<pin>:<from_id>:<from_name>:<reply_port>
    let msg = format!(
        "FRIDAY_PAIR_REQUEST:{}:{}:{}:{}",
        pin, local_id, local_name, reply_port
    );

    socket
        .send_to(msg.as_bytes(), &dest)
        .map_err(|e| format!("Failed to send pairing request to {}: {}", dest, e))?;

    info!(
        "Pairing request sent to {} — PIN={} reply_port={}",
        dest, pin, reply_port
    );
    if let Ok(mut app) = state.lock() {
        app.add_log(
            "INFO",
            "friday_network::pairing",
            &format!(
                "▶ Pairing request sent to {}:{} | PIN={} | local_reply_port={}",
                target_ip, PAIR_REQUEST_PORT, pin, reply_port
            ),
        );
        app.add_log(
            "INFO",
            "friday_network::pairing",
            &format!(
                "⏳ Waiting up to 30s for {} to accept/reject PIN={}...",
                target_ip, pin
            ),
        );
    }

    // Wait for FRIDAY_PAIR_ACCEPT:<pin> or FRIDAY_PAIR_REJECT:<pin>
    let mut buf = [0u8; 256];
    match socket.recv_from(&mut buf) {
        Ok((len, src)) => {
            let reply = String::from_utf8_lossy(&buf[..len]);
            info!("Pairing reply from {}: {}", src, reply);

            if reply.starts_with("FRIDAY_PAIR_ACCEPT:") {
                let reply_pin = reply.trim_start_matches("FRIDAY_PAIR_ACCEPT:").trim();
                if reply_pin == pin {
                    if let Ok(mut app) = state.lock() {
                        app.add_log(
                            "INFO",
                            "friday_network::pairing",
                            &format!(
                                "✅ ACCEPTED by {} ({}): PIN={} confirmed — pairing successful!",
                                target_ip, src, pin
                            ),
                        );
                    }
                    return Ok(true);
                } else {
                    // PIN mismatch — treat as reject
                    if let Ok(mut app) = state.lock() {
                        app.add_log(
                            "WARN",
                            "friday_network::pairing",
                            &format!(
                                "⚠ PIN mismatch from {}: expected={} got={} — treating as reject",
                                src, pin, reply_pin
                            ),
                        );
                    }
                }
            } else if reply.starts_with("FRIDAY_PAIR_REJECT:") {
                if let Ok(mut app) = state.lock() {
                    app.add_log(
                        "WARN",
                        "friday_network::pairing",
                        &format!(
                            "❌ REJECTED by {} ({}): The remote user clicked Reject for PIN={}",
                            target_ip, src, pin
                        ),
                    );
                }
            } else {
                if let Ok(mut app) = state.lock() {
                    app.add_log(
                        "WARN",
                        "friday_network::pairing",
                        &format!(
                            "⚠ Unexpected reply from {} ({}): {:?}",
                            target_ip, src, reply
                        ),
                    );
                }
            }
            // REJECT or wrong PIN
            Ok(false)
        }
        Err(e) => {
            warn!("Pairing timed out or error (30s): {}", e);
            if let Ok(mut app) = state.lock() {
                app.add_log(
                    "WARN",
                    "friday_network::pairing",
                    &format!(
                        "⏱ No response from {} within 30s for PIN={} — timed out ({})",
                        target_ip, pin, e
                    ),
                );
                app.add_log(
                    "INFO",
                    "friday_network::pairing",
                    "ℹ  Possible causes: (1) The remote user did not click Accept in time  \
                     (2) The Accept notification wasn't shown on remote — check remote logs  \
                     (3) Remote Friday app closed or crashed after the probe",
                );
            }
            Ok(false)
        }
    }
}

// ── Remote responder side ──────────────────────────────────────────────────────

/// Start the pairing request responder in a background thread.
/// Listens on PAIR_REQUEST_PORT for incoming FRIDAY_PAIR_REQUEST messages,
/// stores them in PENDING_REQUESTS for the GUI to display.
pub fn start_pairing_responder(
    shared_state: Arc<Mutex<crate::state::AppState>>,
    stop_flag: Arc<AtomicBool>,
) {
    let _ = pending(); // initialise OnceLock

    std::thread::spawn(move || {
        let socket = match UdpSocket::bind(format!("0.0.0.0:{}", PAIR_REQUEST_PORT)) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "Cannot bind pairing responder on port {}: {}",
                    PAIR_REQUEST_PORT, e
                );
                return;
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok();

        info!(
            "FRIDAY Pairing Responder listening on UDP port {}",
            PAIR_REQUEST_PORT
        );

        let mut buf = [0u8; 512];
        while !stop_flag.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    let msg = String::from_utf8_lossy(&buf[..len]);

                    // ── Connectivity probe (pre-flight check from initiator) ──
                    if msg.starts_with("FRIDAY_PAIR_PROBE:") {
                        let _ = socket.send_to(b"FRIDAY_PAIR_PROBE_ACK", src);
                        continue;
                    }

                    // ── Remote Unpair notification ──
                    // Format: FRIDAY_UNPAIR:<from_id>
                    if msg.starts_with("FRIDAY_UNPAIR:") {
                        let from_id = msg.trim_start_matches("FRIDAY_UNPAIR:").trim();
                        let from_ip = src.ip().to_string();
                        info!("Received FRIDAY_UNPAIR from {} ({})", from_id, from_ip);
                        if let Ok(mut app) = shared_state.lock() {
                            let removed = app
                                .devices
                                .iter()
                                .find(|d| {
                                    d.id == from_id || d.ip_address == from_ip || d.id == from_ip
                                })
                                .cloned();
                            if let Some(dev) = removed {
                                let target_id = dev.id.clone();
                                let target_name = dev.name.clone();
                                app.devices.retain(|d| {
                                    d.id != target_id && d.ip_address != from_ip && d.id != from_id
                                });
                                app.topology.remove_device(&target_id);
                                app.topology.remove_device(from_id);
                                if app.active_device_id == target_id
                                    || app.active_device_id == from_id
                                {
                                    let local_id = app.local_device_id.clone();
                                    app.active_device_id = local_id.clone();
                                    app.topology.set_active_device(&local_id);
                                }
                                app.add_log(
                                    "INFO",
                                    "friday_network::pairing",
                                    &format!(
                                        "Device {} ({}) unpaired by remote peer",
                                        target_name, target_id
                                    ),
                                );
                                app.persist_config();
                            }
                        }
                        continue;
                    }

                    // ── Real pair request ──
                    // Format: FRIDAY_PAIR_REQUEST:<pin>:<from_id>:<from_name>:<reply_port>
                    if msg.starts_with("FRIDAY_PAIR_REQUEST:") {
                        let body = msg.trim_start_matches("FRIDAY_PAIR_REQUEST:");
                        // Split into exactly 4 parts: pin, from_id, from_name, reply_port
                        let parts: Vec<&str> = body.splitn(4, ':').collect();
                        if parts.len() == 4 {
                            let pin = parts[0].trim().to_string();
                            let from_id = parts[1].trim().to_string();
                            let from_name = parts[2].trim().to_string();
                            let reply_port = parts[3].trim().parse::<u16>().unwrap_or(48703);
                            let from_ip = src.ip().to_string();

                            info!(
                                "Incoming pair request from {} ({}) PIN={} reply_port={}",
                                from_name, from_ip, pin, reply_port
                            );

                            let mut pending_list = pending().lock().unwrap();
                            if !pending_list
                                .iter()
                                .any(|r| r.pin == pin && r.from_id == from_id)
                            {
                                let new_req = PendingPairRequest {
                                    pin,
                                    from_id,
                                    from_name,
                                    from_ip,
                                    reply_port,
                                };
                                pending_list.push(new_req.clone());
                                drop(pending_list); // release lock before notifying

                                // Event-driven: notify the Tauri setup hook thread immediately.
                                // This triggers an emit_all("friday:pair_request", ...) with
                                // zero polling delay instead of waiting for the 2-second poll.
                                init_notify_channel();
                                if let Some(tx) = PAIR_NOTIFY_TX.get() {
                                    let _ = tx.lock().unwrap().send(new_req);
                                }
                            }
                        } else {
                            warn!(
                                "Malformed pair request from {}: expected 4 fields, got {}",
                                src,
                                parts.len()
                            );
                        }
                    }
                }
                Err(_) => {
                    // Read timeout — keep looping
                }
            }
        }
    });
}

/// Send Accept or Reject reply directly to the initiator's ephemeral reply port
pub fn respond_to_request(request: &PendingPairRequest, accept: bool) -> Result<(), String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Bind failed: {}", e))?;

    // Reply directly to the port the initiator is listening on
    let dest = format!("{}:{}", request.from_ip, request.reply_port);
    let msg = if accept {
        format!("FRIDAY_PAIR_ACCEPT:{}", request.pin)
    } else {
        format!("FRIDAY_PAIR_REJECT:{}", request.pin)
    };

    socket.send_to(msg.as_bytes(), &dest).map_err(|e| {
        format!(
            "Failed to send pairing {} to {}: {}",
            if accept { "ACCEPT" } else { "REJECT" },
            dest,
            e
        )
    })?;

    info!(
        "Sent pairing {} to {}",
        if accept { "ACCEPT" } else { "REJECT" },
        dest
    );

    remove_pending(&request.pin);
    Ok(())
}
