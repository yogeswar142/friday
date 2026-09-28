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
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tracing::{info, warn};

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
/// Uses an OS-assigned ephemeral port for the reply so there are no port conflicts
/// on retries. Blocks the calling thread for up to 30 seconds.
///
/// Returns Ok(true) = accepted, Ok(false) = rejected or timed out.
pub fn send_pairing_request(
    target_ip: &str,
    local_id: &str,
    local_name: &str,
    pin: &str,
) -> Result<bool, String> {
    // Bind reply listener on an ephemeral OS-assigned port
    let reply_socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Cannot bind reply socket: {}", e))?;
    reply_socket
        .set_read_timeout(Some(Duration::from_secs(30)))
        .ok();

    // Get the actual port the OS assigned
    let reply_port = reply_socket
        .local_addr()
        .map_err(|e| format!("Cannot get local addr: {}", e))?
        .port();

    // Send request — include reply_port so remote knows where to send the ACK
    let send_socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Send socket bind failed: {}", e))?;
    let dest = format!("{}:{}", target_ip, PAIR_REQUEST_PORT);
    // Format: FRIDAY_PAIR_REQUEST:<pin>:<from_id>:<from_name>:<reply_port>
    let msg = format!(
        "FRIDAY_PAIR_REQUEST:{}:{}:{}:{}",
        pin, local_id, local_name, reply_port
    );

    send_socket
        .send_to(msg.as_bytes(), &dest)
        .map_err(|e| format!("Failed to send pairing request to {}: {}", dest, e))?;

    info!(
        "Pairing request sent to {} — PIN={} reply_port={}",
        dest, pin, reply_port
    );

    // Wait for FRIDAY_PAIR_ACCEPT:<pin> or FRIDAY_PAIR_REJECT:<pin>
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
            warn!("Pairing timed out or error (30s): {}", e);
            Ok(false)
        }
    }
}

// ── Remote responder side ──────────────────────────────────────────────────────

/// Start the pairing request responder in a background thread.
/// Listens on PAIR_REQUEST_PORT for incoming FRIDAY_PAIR_REQUEST messages,
/// stores them in PENDING_REQUESTS for the GUI to display.
pub fn start_pairing_responder(stop_flag: Arc<AtomicBool>) {
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
                                pending_list.push(PendingPairRequest {
                                    pin,
                                    from_id,
                                    from_name,
                                    from_ip,
                                    reply_port,
                                });
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
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Bind failed: {}", e))?;

    // Reply directly to the port the initiator is listening on
    let dest = format!("{}:{}", request.from_ip, request.reply_port);
    let msg = if accept {
        format!("FRIDAY_PAIR_ACCEPT:{}", request.pin)
    } else {
        format!("FRIDAY_PAIR_REJECT:{}", request.pin)
    };

    socket
        .send_to(msg.as_bytes(), &dest)
        .map_err(|e| {
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
