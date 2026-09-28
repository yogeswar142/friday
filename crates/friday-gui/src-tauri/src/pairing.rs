/// FRIDAY Pairing Protocol (v2 — correct port binding)
///
/// Ports:
///   48702 = remote listens for PAIR_REQUEST
///   48703 = initiator listens for PAIR_ACCEPT / PAIR_REJECT reply
///
/// Full flow:
///   [Initiator GUI]
///     1. generate 6-digit PIN
///     2. call `initiate_pairing(device_id, pin)` → Tauri command
///     3. Rust binds UDP 0.0.0.0:48703, sends FRIDAY_PAIR_REQUEST:<pin>:<id>:<name>
///        to <remote_ip>:48702
///     4. Waits up to 30s on port 48703 for FRIDAY_PAIR_ACCEPT:<pin>
///
///   [Remote machine — GUI or headless agent]
///     1. Pairing responder on port 48702 receives the request
///     2. Stores PendingPairRequest in PENDING_REQUESTS
///     3. GUI polls get_pending_pair_requests every 2s → shows incoming modal with PIN
///     4. User clicks Accept → respond_to_pair_request(pin, true)
///        → Rust sends FRIDAY_PAIR_ACCEPT:<pin> to <initiator_ip>:48703
///     5. User clicks Reject → sends FRIDAY_PAIR_REJECT:<pin>
///
///   [Initiator receives ACCEPT]
///     → pair_device() is called locally → device added to ring

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tracing::{info, warn};

/// Port the remote device listens on for incoming pair requests
pub const PAIR_REQUEST_PORT: u16 = 48702;
/// Port the initiator listens on for Accept/Reject responses
pub const PAIR_REPLY_PORT: u16 = 48703;

/// A pending incoming pair request on this machine
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingPairRequest {
    pub pin: String,
    pub from_id: String,
    pub from_name: String,
    pub from_ip: String,
}

/// Global list of pending incoming requests (populated by responder thread)
pub static PENDING_REQUESTS: OnceLock<Mutex<Vec<PendingPairRequest>>> = OnceLock::new();

fn pending() -> &'static Mutex<Vec<PendingPairRequest>> {
    PENDING_REQUESTS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Get all pending incoming pair requests (called by GUI poll)
pub fn get_pending_requests() -> Vec<PendingPairRequest> {
    pending().lock().unwrap().clone()
}

/// Remove a request by PIN (after user responds)
pub fn remove_pending(pin: &str) {
    pending().lock().unwrap().retain(|r| r.pin != pin);
}

// ── Initiator side ─────────────────────────────────────────────────────────────

/// Initiate pairing with a remote FRIDAY device.
/// Sends the PIN over UDP and waits (blocking, up to 30s) for the remote user to accept.
/// Returns Ok(true) = accepted, Ok(false) = rejected or timeout.
pub fn send_pairing_request(
    target_ip: &str,
    local_id: &str,
    local_name: &str,
    pin: &str,
) -> Result<bool, String> {
    // Bind reply listener FIRST so we're ready before remote replies
    let reply_socket =
        UdpSocket::bind(format!("0.0.0.0:{}", PAIR_REPLY_PORT))
            .map_err(|e| format!("Cannot bind reply socket on port {}: {}", PAIR_REPLY_PORT, e))?;
    reply_socket
        .set_read_timeout(Some(Duration::from_secs(30)))
        .ok();

    // Send request
    let send_socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Send socket bind failed: {}", e))?;
    let dest = format!("{}:{}", target_ip, PAIR_REQUEST_PORT);
    let msg = format!("FRIDAY_PAIR_REQUEST:{}:{}:{}", pin, local_id, local_name);

    send_socket
        .send_to(msg.as_bytes(), &dest)
        .map_err(|e| format!("Failed to send pairing request to {}: {}", dest, e))?;

    info!("Pairing request sent to {} with PIN {}", dest, pin);

    // Wait for reply on port 48703
    let mut buf = [0u8; 256];
    match reply_socket.recv_from(&mut buf) {
        Ok((len, src)) => {
            let reply = String::from_utf8_lossy(&buf[..len]);
            info!("Pairing reply from {}: {}", src, reply);
            if reply.starts_with("FRIDAY_PAIR_ACCEPT:") {
                let reply_pin = reply.trim_start_matches("FRIDAY_PAIR_ACCEPT:").trim();
                if reply_pin == pin {
                    return Ok(true);
                }
            }
            // REJECT or wrong PIN
            Ok(false)
        }
        Err(e) => {
            warn!("Pairing timed out or error after 30s: {}", e);
            Ok(false)
        }
    }
}

// ── Remote responder side ──────────────────────────────────────────────────────

/// Start the pairing request responder in a background thread.
/// Listens on PAIR_REQUEST_PORT for incoming FRIDAY_PAIR_REQUEST messages
/// and stores them in PENDING_REQUESTS for the GUI to display.
pub fn start_pairing_responder(stop_flag: Arc<AtomicBool>) {
    // Initialise the static
    let _ = pending();

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
                    // Format: FRIDAY_PAIR_REQUEST:<pin>:<from_id>:<from_name>
                    if msg.starts_with("FRIDAY_PAIR_REQUEST:") {
                        let body = msg.trim_start_matches("FRIDAY_PAIR_REQUEST:");
                        let parts: Vec<&str> = body.splitn(3, ':').collect();
                        if parts.len() == 3 {
                            let pin = parts[0].trim().to_string();
                            let from_id = parts[1].trim().to_string();
                            let from_name = parts[2].trim().to_string();
                            let from_ip = src.ip().to_string();

                            info!(
                                "Incoming pair request from {} ({}) PIN={}",
                                from_name, from_ip, pin
                            );

                            let mut pending_list = pending().lock().unwrap();
                            if !pending_list.iter().any(|r| r.pin == pin && r.from_id == from_id) {
                                pending_list.push(PendingPairRequest {
                                    pin,
                                    from_id,
                                    from_name,
                                    from_ip,
                                });
                            }
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

/// Send Accept or Reject reply to the initiator's reply port (48703)
pub fn respond_to_request(request: &PendingPairRequest, accept: bool) -> Result<(), String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Bind failed: {}", e))?;

    let dest = format!("{}:{}", request.from_ip, PAIR_REPLY_PORT);
    let msg = if accept {
        format!("FRIDAY_PAIR_ACCEPT:{}", request.pin)
    } else {
        format!("FRIDAY_PAIR_REJECT:{}", request.pin)
    };

    socket
        .send_to(msg.as_bytes(), &dest)
        .map_err(|e| format!("Failed to send pairing {} to {}: {}", if accept { "ACCEPT" } else { "REJECT" }, dest, e))?;

    info!(
        "Sent pairing {} to {}",
        if accept { "ACCEPT" } else { "REJECT" },
        dest
    );

    // Remove from pending list
    remove_pending(&request.pin);
    Ok(())
}
