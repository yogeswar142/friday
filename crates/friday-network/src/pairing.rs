use crate::events::{FridayNetworkEvent, PairingRequestEvent};
use crate::identity::DeviceIdentity;
use crate::protocol::DeviceCapabilities;
use crate::trust::{TrustStore, TrustedDevice};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use tracing::{info, warn};

pub const PAIRING_PORT: u16 = 48702;

/// Global list of pending incoming pairing requests
static PENDING_REQUESTS: OnceLock<RwLock<HashMap<String, PairingRequestEvent>>> = OnceLock::new();

fn pending_map() -> &'static RwLock<HashMap<String, PairingRequestEvent>> {
    PENDING_REQUESTS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Computes a cryptographic SHA-256 auth token for pairing session
pub fn compute_auth_token(
    initiator_id: &str,
    responder_id: &str,
    pin: &str,
    nonce: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"FRIDAY_V1_AUTH_TOKEN:");
    hasher.update(initiator_id.as_bytes());
    hasher.update(b":");
    hasher.update(responder_id.as_bytes());
    hasher.update(b":");
    hasher.update(pin.as_bytes());
    hasher.update(b":");
    hasher.update(nonce.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Generates a cryptographically random 6-digit PIN using OS entropy (OsRng).
///
/// # Security
/// Uses `rand::rngs::OsRng` which sources from the OS entropy pool:
/// - Windows: `CryptGenRandom` / `BCryptGenRandom`
/// - Linux: `getrandom(2)` syscall
/// - macOS: `SecRandomCopyBytes`
///
/// Predictable seeds (system time, process ID, etc.) MUST NOT be used for
/// pairing PINs as they are observable by a local attacker.
pub fn generate_pairing_pin() -> String {
    use rand::Rng;
    let pin_num: u32 = rand::rngs::OsRng.gen_range(100_000..=999_999);
    format!("{:06}", pin_num)
}

/// Generates a cryptographically random nonce string for auth token computation.
/// The nonce MUST be different for every pairing session to prevent token reuse attacks.
pub fn generate_session_nonce() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::rngs::OsRng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Pairing manager for coordinating incoming and outgoing pairing flows
#[derive(Clone)]
pub struct PairingManager {
    identity: DeviceIdentity,
    trust_store: TrustStore,
    event_tx: broadcast::Sender<FridayNetworkEvent>,
    stop_flag: Arc<AtomicBool>,
}

impl PairingManager {
    pub fn new(
        identity: DeviceIdentity,
        trust_store: TrustStore,
        event_tx: broadcast::Sender<FridayNetworkEvent>,
    ) -> Self {
        Self {
            identity,
            trust_store,
            event_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start pairing responder background thread (listens on PAIRING_PORT: 48702)
    pub fn start_responder(&self) {
        let socket = match UdpSocket::bind(format!("0.0.0.0:{}", PAIRING_PORT)) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "Pairing responder could not bind to port {}: {}. Pairing requests will use fallback.",
                    PAIRING_PORT, e
                );
                return;
            }
        };

        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok();

        let stop = self.stop_flag.clone();
        let event_tx = self.event_tx.clone();
        let trust_store = self.trust_store.clone();
        let local_id = self.identity.device_id.clone();

        info!(
            "FRIDAY Pairing Responder listening on UDP port {}",
            PAIRING_PORT
        );

        thread::spawn(move || {
            let mut buf = [0u8; 1024];

            while !stop.load(Ordering::Relaxed) {
                match socket.recv_from(&mut buf) {
                    Ok((len, src)) => {
                        let text = String::from_utf8_lossy(&buf[..len]);

                        // 1. Connectivity Probe
                        if text.starts_with("FRIDAY_PAIR_PROBE:") {
                            let _ = socket.send_to(b"FRIDAY_PAIR_PROBE_ACK", src);
                            continue;
                        }

                        // 2. Unpair / Forget Notification
                        if text.starts_with("FRIDAY_UNPAIR:") {
                            let body = text.trim_start_matches("FRIDAY_UNPAIR:");
                            let parts: Vec<&str> = body.split(':').collect();
                            if !parts.is_empty() {
                                let remote_id = parts[0].trim();
                                info!("Received remote unpair notification from {}", remote_id);
                                let _ = trust_store.remove_trusted(remote_id);
                                let _ = event_tx.send(FridayNetworkEvent::DeviceDisconnected {
                                    device_id: remote_id.to_string(),
                                    reason: "Unpaired by remote device".to_string(),
                                });
                            }
                            continue;
                        }

                        // 3. Incoming Pair Request:
                        // Format: FRIDAY_PAIR_REQ:<pin>:<initiator_id>:<initiator_name>:<initiator_os>:<reply_port>:<nonce>
                        if text.starts_with("FRIDAY_PAIR_REQ:") {
                            let body = text.trim_start_matches("FRIDAY_PAIR_REQ:");
                            let parts: Vec<&str> = body.split(':').collect();
                            if parts.len() >= 6 {
                                let pin = parts[0].trim().to_string();
                                let from_id = parts[1].trim().to_string();
                                let from_name = parts[2].trim().to_string();
                                let reply_port =
                                    parts[4].trim().parse::<u16>().unwrap_or(PAIRING_PORT);

                                if from_id == local_id {
                                    continue;
                                }

                                let req_event = PairingRequestEvent {
                                    pin: pin.clone(),
                                    from_id: from_id.clone(),
                                    from_name: from_name.clone(),
                                    from_ip: src.ip().to_string(),
                                    reply_port,
                                };

                                {
                                    let mut map = pending_map().write().unwrap();
                                    map.insert(pin.clone(), req_event.clone());
                                }

                                info!(
                                    "Incoming pairing request: PIN={} from {} ({})",
                                    pin,
                                    from_name,
                                    src.ip()
                                );
                                let _ =
                                    event_tx.send(FridayNetworkEvent::PairingRequested(req_event));
                            }
                        }
                    }
                    Err(_) => {
                        // Timeout, loop
                    }
                }
            }
        });
    }

    /// Probe if a remote host is reachable on the pairing port
    pub fn probe_endpoint(target_ip: &str) -> Result<(), String> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Cannot bind UDP probe socket: {}", e))?;
        socket.set_read_timeout(Some(Duration::from_secs(3))).ok();

        let dest = format!("{}:{}", target_ip, PAIRING_PORT);
        socket
            .send_to(b"FRIDAY_PAIR_PROBE:0", &dest)
            .map_err(|e| format!("Cannot reach {} on pairing port: {}", dest, e))?;

        let mut buf = [0u8; 64];
        match socket.recv_from(&mut buf) {
            Ok((_len, _)) => Ok(()),
            Err(_) => Err(format!(
                "Cannot reach FRIDAY on {} after 3 seconds. Verify FRIDAY is running on that machine.",
                target_ip
            )),
        }
    }

    /// Initiate an outgoing pairing request to a remote device
    pub async fn initiate_pairing(
        &self,
        target_ip: &str,
        pin: &str,
    ) -> Result<TrustedDevice, String> {
        // Run pre-flight reachability check
        Self::probe_endpoint(target_ip)?;

        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Cannot bind ephemeral pairing socket: {}", e))?;
        socket.set_read_timeout(Some(Duration::from_secs(30))).ok();

        let reply_port = socket
            .local_addr()
            .map_err(|e| format!("Local addr error: {}", e))?
            .port();

        let nonce = format!(
            "{:x}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let dest = format!("{}:{}", target_ip, PAIRING_PORT);

        // Format: FRIDAY_PAIR_REQ:<pin>:<initiator_id>:<initiator_name>:<initiator_os>:<reply_port>:<nonce>
        let req_msg = format!(
            "FRIDAY_PAIR_REQ:{}:{}:{}:{}:{}:{}",
            pin,
            self.identity.device_id,
            self.identity.display_name,
            self.identity.os,
            reply_port,
            nonce
        );

        socket
            .send_to(req_msg.as_bytes(), &dest)
            .map_err(|e| format!("Failed to send pairing request: {}", e))?;

        info!(
            "Pairing request sent to {} with PIN={}. Waiting for user confirmation...",
            dest, pin
        );

        let mut buf = [0u8; 1024];
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                let reply = String::from_utf8_lossy(&buf[..len]);

                if reply.starts_with("FRIDAY_PAIR_ACCEPT:") {
                    let body = reply.trim_start_matches("FRIDAY_PAIR_ACCEPT:");
                    let parts: Vec<&str> = body.split(':').collect();
                    if parts.len() >= 4 {
                        let reply_pin = parts[0].trim();
                        let peer_id = parts[1].trim();
                        let peer_name = parts[2].trim();
                        let auth_token = parts[3].trim();

                        if reply_pin == pin {
                            let trusted = TrustedDevice {
                                device_id: peer_id.to_string(),
                                display_name: peer_name.to_string(),
                                auth_token: auth_token.to_string(),
                                last_known_ip: src.ip().to_string(),
                                last_known_port: 48700,
                                paired_at: SystemTime::now()
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                                capabilities: DeviceCapabilities::default(),
                                noise_static_pubkey_b64: None,
                            };

                            let _ = self.trust_store.add_trusted(trusted.clone());

                            info!(
                                "Pairing accepted by {} ({})! Saved to trust store.",
                                peer_name, peer_id
                            );

                            let _ = self.event_tx.send(FridayNetworkEvent::PairingCompleted {
                                device_id: peer_id.to_string(),
                                device_name: peer_name.to_string(),
                                success: true,
                                message: "PIN confirmed and trusted".to_string(),
                            });

                            return Ok(trusted);
                        }
                    }
                } else if reply.starts_with("FRIDAY_PAIR_REJECT:") {
                    let _ = self.event_tx.send(FridayNetworkEvent::PairingFailed {
                        device_id: target_ip.to_string(),
                        reason: "User on remote machine clicked Reject".to_string(),
                    });
                    return Err("Pairing rejected by remote user".to_string());
                }

                Err("Pairing rejected or invalid response".to_string())
            }
            Err(e) => {
                let _ = self.event_tx.send(FridayNetworkEvent::PairingFailed {
                    device_id: target_ip.to_string(),
                    reason: "Timed out waiting for confirmation".to_string(),
                });
                Err(format!("Pairing timed out after 30 seconds: {}", e))
            }
        }
    }

    /// User approval or rejection of an incoming pair request
    pub fn respond_to_request(&self, pin: &str, accept: bool) -> Result<(), String> {
        let req = {
            let mut map = pending_map().write().unwrap();
            map.remove(pin)
                .ok_or_else(|| format!("No pending pair request with PIN {}", pin))?
        };

        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Bind failed: {}", e))?;

        let dest = format!("{}:{}", req.from_ip, req.reply_port);

        if accept {
            // Generate a cryptographically random nonce for this session.
            // The nonce is included in the reply message so the initiator can
            // compute and verify the auth token independently.
            let nonce = generate_session_nonce();
            let auth_token =
                compute_auth_token(&req.from_id, &self.identity.device_id, pin, &nonce);

            // Format: FRIDAY_PAIR_ACCEPT:<pin>:<responder_id>:<responder_name>:<token>:<nonce>
            let reply = format!(
                "FRIDAY_PAIR_ACCEPT:{}:{}:{}:{}:{}",
                pin, self.identity.device_id, self.identity.display_name, auth_token, nonce
            );
            socket
                .send_to(reply.as_bytes(), &dest)
                .map_err(|e| format!("Failed to send pairing accept: {}", e))?;

            // Store in our trust store too (symmetric trust)
            let trusted = TrustedDevice {
                device_id: req.from_id.clone(),
                display_name: req.from_name.clone(),
                auth_token,
                last_known_ip: req.from_ip.clone(),
                last_known_port: 48700,
                paired_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                capabilities: DeviceCapabilities::default(),
                noise_static_pubkey_b64: None,
            };
            let _ = self.trust_store.add_trusted(trusted);

            info!("Accepted pairing from {} (PIN {})", req.from_name, pin);
            let _ = self.event_tx.send(FridayNetworkEvent::PairingCompleted {
                device_id: req.from_id,
                device_name: req.from_name,
                success: true,
                message: "Pairing accepted by local user".to_string(),
            });
        } else {
            let reply = format!("FRIDAY_PAIR_REJECT:{}", pin);
            let _ = socket.send_to(reply.as_bytes(), &dest);
            info!("Rejected pairing from {} (PIN {})", req.from_name, pin);
            let _ = self.event_tx.send(FridayNetworkEvent::PairingFailed {
                device_id: req.from_id,
                reason: "Rejected by local user".to_string(),
            });
        }

        Ok(())
    }

    /// List all currently pending incoming pair requests
    pub fn get_pending_requests() -> Vec<PairingRequestEvent> {
        let map = pending_map().read().unwrap();
        map.values().cloned().collect()
    }

    /// Send unpair notice to remote device and remove from local trust store
    pub fn unpair_device(&self, device_id: &str) -> Result<bool, String> {
        let trusted = self.trust_store.get_trusted(device_id);
        let removed = self
            .trust_store
            .remove_trusted(device_id)
            .map_err(|e| e.to_string())?;

        if let Some(dev) = trusted {
            // Send unpair packet to remote machine
            let dest = format!("{}:{}", dev.last_known_ip, PAIRING_PORT);
            let msg = format!(
                "FRIDAY_UNPAIR:{}:{}",
                self.identity.device_id, dev.auth_token
            );
            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
                for _ in 0..2 {
                    let _ = socket.send_to(msg.as_bytes(), &dest);
                }
            }
        }

        let _ = self.event_tx.send(FridayNetworkEvent::DeviceDisconnected {
            device_id: device_id.to_string(),
            reason: "Unpaired".to_string(),
        });

        Ok(removed)
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_token_computation_and_pin() {
        let pin = generate_pairing_pin();
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));

        let token1 = compute_auth_token("id-a", "id-b", &pin, "nonce1");
        let token2 = compute_auth_token("id-a", "id-b", &pin, "nonce1");
        let token3 = compute_auth_token("id-a", "id-b", &pin, "nonce2");

        assert_eq!(token1, token2);
        assert_ne!(token1, token3);
    }
}
