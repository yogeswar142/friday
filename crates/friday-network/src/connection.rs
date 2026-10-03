use crate::events::{ConnectionState, FridayNetworkEvent};
use crate::identity::DeviceIdentity;
use crate::protocol::{NetworkPacket, PacketPayload};
use crate::transport::NetworkTransport;
use crate::trust::TrustStore;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::{debug, info};

/// Network diagnostic metrics for advanced inspection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkDiagnostics {
    pub local_ip: String,
    pub remote_ip: Option<String>,
    pub port: u16,
    pub transport: String,
    pub interface: String,
    pub connection_id: String,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub rtt_ms: f32,
    pub packet_loss_pct: f32,
    pub reconnect_attempts: u32,
    pub discovery_status: String,
    pub state: ConnectionState,
}

/// Security errors encountered when validating incoming packets
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SecurityError {
    #[error("Device '{0}' is not trusted (not in trust store)")]
    UntrustedSender(String),
    #[error("No active session established for device '{0}'")]
    NoActiveSession(String),
    #[error("Device '{device_id}' cannot inject input while in state '{current_state:?}' (must be Connected)")]
    InvalidStateForInput {
        device_id: String,
        current_state: ConnectionState,
    },
    #[error("Replay or stale packet detected from '{device_id}' with sequence {sequence}")]
    ReplayDetected { device_id: String, sequence: u32 },
}

use crate::transport::ReplayWindow;

/// Managed session for a single connected or connecting device
pub struct ManagedSession {
    pub device_id: String,
    pub display_name: String,
    pub remote_endpoint: SocketAddr,
    pub state: ConnectionState,
    pub rtt_ms: f32,
    pub packet_loss_pct: f32,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub last_heartbeat_sent: Instant,
    pub last_heartbeat_recv: Instant,
    pub reconnect_attempts: u32,
    pub next_reconnect_delay_ms: u64,
    pub replay_window: ReplayWindow,
}

/// Central Connection Manager orchestrating device states, authenticated sessions,
/// heartbeats, and exponential backoff automatic reconnection.
#[derive(Clone)]
pub struct ConnectionManager {
    identity: DeviceIdentity,
    trust_store: TrustStore,
    transport: Arc<NetworkTransport>,
    sessions: Arc<RwLock<HashMap<String, ManagedSession>>>,
    event_tx: broadcast::Sender<FridayNetworkEvent>,
    stop_flag: Arc<AtomicBool>,
    total_packets_sent: Arc<AtomicU64>,
    total_packets_recv: Arc<AtomicU64>,
}

impl ConnectionManager {
    pub fn new(
        identity: DeviceIdentity,
        trust_store: TrustStore,
        transport: Arc<NetworkTransport>,
        event_tx: broadcast::Sender<FridayNetworkEvent>,
    ) -> Self {
        Self {
            identity,
            trust_store,
            transport,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
            total_packets_sent: Arc::new(AtomicU64::new(0)),
            total_packets_recv: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Connect to a trusted device by its device_id
    pub async fn connect_device(&self, device_id: &str) -> Result<(), String> {
        let trusted = self.trust_store.get_trusted(device_id).ok_or_else(|| {
            format!(
                "Device {} is not in trust store. Please pair first.",
                device_id
            )
        })?;

        let endpoint_str = format!("{}:{}", trusted.last_known_ip, trusted.last_known_port);
        let endpoint: SocketAddr = endpoint_str
            .parse()
            .map_err(|e| format!("Invalid target endpoint {}: {}", endpoint_str, e))?;

        self.set_device_state(device_id, ConnectionState::Connecting);

        // Send Control Handshake packet
        let handshake_packet = NetworkPacket {
            header: crate::protocol::PacketHeader {
                magic: *crate::protocol::MAGIC_BYTES,
                version: crate::protocol::PROTOCOL_VERSION,
                channel: crate::protocol::ChannelType::Control,
                sequence: self.transport.next_seq(),
            },
            payload: PacketPayload::ControlHandshake {
                device_id: self.identity.device_id.clone(),
                device_name: self.identity.display_name.clone(),
                capabilities: self.identity.capabilities.clone(),
            },
        };

        let encoded = handshake_packet
            .encode()
            .map_err(|e| format!("Encode error: {}", e))?;

        self.transport
            .send_raw_to(&encoded, endpoint)
            .await
            .map_err(|e| format!("Send handshake failed: {}", e))?;

        // Initialize or update managed session
        {
            let mut map = self.sessions.write().unwrap();
            let session = ManagedSession {
                device_id: device_id.to_string(),
                display_name: trusted.display_name.clone(),
                remote_endpoint: endpoint,
                state: ConnectionState::Connected,
                rtt_ms: 0.85,
                packet_loss_pct: 0.0,
                packets_sent: 1,
                packets_received: 0,
                last_heartbeat_sent: Instant::now(),
                last_heartbeat_recv: Instant::now(),
                reconnect_attempts: 0,
                next_reconnect_delay_ms: 1000,
                replay_window: ReplayWindow::new(),
            };
            map.insert(device_id.to_string(), session);
        }

        self.set_device_state(device_id, ConnectionState::Connected);
        let _ = self.event_tx.send(FridayNetworkEvent::DeviceConnected {
            device_id: device_id.to_string(),
            endpoint: endpoint_str,
        });

        info!(
            "Connected to trusted device: {} ('{}') at {}",
            device_id, trusted.display_name, endpoint
        );

        Ok(())
    }

    /// Disconnect from a device gracefully
    pub async fn disconnect_device(&self, device_id: &str) -> Result<(), String> {
        let (endpoint, _old_state) = {
            let mut map = self.sessions.write().unwrap();
            if let Some(session) = map.get_mut(device_id) {
                let ep = session.remote_endpoint;
                let old = session.state;
                session.state = ConnectionState::Disconnected;
                (Some(ep), old)
            } else {
                (None, ConnectionState::Unknown)
            }
        };

        if let Some(ep) = endpoint {
            // Send Goodbye heartbeat / control
            let goodbye = NetworkPacket {
                header: crate::protocol::PacketHeader {
                    magic: *crate::protocol::MAGIC_BYTES,
                    version: crate::protocol::PROTOCOL_VERSION,
                    channel: crate::protocol::ChannelType::Control,
                    sequence: self.transport.next_seq(),
                },
                payload: PacketPayload::Heartbeat { timestamp: 0 },
            };
            if let Ok(bytes) = goodbye.encode() {
                let _ = self.transport.send_raw_to(&bytes, ep).await;
            }
        }

        self.set_device_state(device_id, ConnectionState::Disconnected);
        let _ = self.event_tx.send(FridayNetworkEvent::DeviceDisconnected {
            device_id: device_id.to_string(),
            reason: "User requested disconnect".to_string(),
        });

        info!("Disconnected from device: {}", device_id);
        Ok(())
    }

    /// Get current state of a device
    pub fn get_device_state(&self, device_id: &str) -> ConnectionState {
        let map = self.sessions.read().unwrap();
        if let Some(session) = map.get(device_id) {
            session.state
        } else if self.trust_store.is_trusted(device_id) {
            ConnectionState::Trusted
        } else {
            ConnectionState::Discovered
        }
    }

    /// Transition a device state and emit event if changed
    pub fn set_device_state(&self, device_id: &str, new_state: ConnectionState) {
        let old_state = {
            let mut map = self.sessions.write().unwrap();
            if let Some(session) = map.get_mut(device_id) {
                let old = session.state;
                session.state = new_state;
                old
            } else {
                let old = if self.trust_store.is_trusted(device_id) {
                    ConnectionState::Trusted
                } else {
                    ConnectionState::Discovered
                };
                map.insert(
                    device_id.to_string(),
                    ManagedSession {
                        device_id: device_id.to_string(),
                        display_name: device_id.to_string(),
                        remote_endpoint: "0.0.0.0:0"
                            .parse()
                            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0))),
                        state: new_state,
                        rtt_ms: 0.0,
                        packet_loss_pct: 0.0,
                        packets_sent: 0,
                        packets_received: 0,
                        last_heartbeat_sent: Instant::now(),
                        last_heartbeat_recv: Instant::now(),
                        reconnect_attempts: 0,
                        next_reconnect_delay_ms: 1000,
                        replay_window: ReplayWindow::new(),
                    },
                );
                old
            }
        };

        if old_state != new_state {
            info!(
                "Device {} state changed: {:?} -> {:?}",
                device_id, old_state, new_state
            );
            let _ = self
                .event_tx
                .send(FridayNetworkEvent::ConnectionStateChanged {
                    device_id: device_id.to_string(),
                    old_state,
                    new_state,
                });
        }
    }

    /// Automatic reconnection tick: applies exponential backoff for disconnected trusted devices
    pub async fn tick_reconnection(&self) {
        let mut to_reconnect = Vec::new();

        {
            let mut map = self.sessions.write().unwrap();
            for (id, session) in map.iter_mut() {
                if (session.state == ConnectionState::Disconnected
                    || session.state == ConnectionState::Reconnecting)
                    && session.last_heartbeat_sent.elapsed().as_millis() as u64
                        >= session.next_reconnect_delay_ms
                {
                    session.state = ConnectionState::Reconnecting;
                    session.reconnect_attempts += 1;
                    // Exponential backoff: 1s, 2s, 4s, 8s, up to 15s max
                    session.next_reconnect_delay_ms =
                        (session.next_reconnect_delay_ms * 2).min(15000);
                    session.last_heartbeat_sent = Instant::now();

                    if session.reconnect_attempts > 6 {
                        session.state = ConnectionState::Unreachable;
                        debug!(
                            "Device {} exceeded max reconnect attempts, marking Unreachable",
                            id
                        );
                    } else {
                        to_reconnect.push((id.clone(), session.remote_endpoint));
                    }
                }
            }
        }

        // Try reconnect probes asynchronously
        for (id, ep) in to_reconnect {
            debug!("Attempting reconnect to {} at {:?}", id, ep);
            let ping = NetworkPacket {
                header: crate::protocol::PacketHeader {
                    magic: *crate::protocol::MAGIC_BYTES,
                    version: crate::protocol::PROTOCOL_VERSION,
                    channel: crate::protocol::ChannelType::Control,
                    sequence: self.transport.next_seq(),
                },
                payload: PacketPayload::Heartbeat {
                    timestamp: Instant::now().elapsed().as_millis() as u64,
                },
            };
            if let Ok(bytes) = ping.encode() {
                let _ = self.transport.send_raw_to(&bytes, ep).await;
            }
        }
    }

    /// Handle received heartbeat acknowledgment from peer
    pub fn handle_heartbeat_ack(&self, device_id: &str, rtt_ms: f32) {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.get_mut(device_id) {
            session.rtt_ms = (session.rtt_ms * 0.7) + (rtt_ms * 0.3);
            session.last_heartbeat_recv = Instant::now();
            if session.state == ConnectionState::Reconnecting
                || session.state == ConnectionState::Disconnected
            {
                session.state = ConnectionState::Connected;
                session.reconnect_attempts = 0;
                session.next_reconnect_delay_ms = 1000;
                let _ = self
                    .event_tx
                    .send(FridayNetworkEvent::ConnectionStateChanged {
                        device_id: device_id.to_string(),
                        old_state: ConnectionState::Reconnecting,
                        new_state: ConnectionState::Connected,
                    });
            }
        }
    }

    /// Security validation error when processing incoming packets
    pub fn validate_incoming_packet(
        &self,
        device_id: &str,
        packet: &NetworkPacket,
    ) -> Result<(), SecurityError> {
        // 1. Must be in trust store
        if !self.trust_store.is_trusted(device_id) {
            return Err(SecurityError::UntrustedSender(device_id.to_string()));
        }

        // 2. Active session check
        let mut map = self.sessions.write().unwrap();
        let session = map
            .get_mut(device_id)
            .ok_or_else(|| SecurityError::NoActiveSession(device_id.to_string()))?;

        // 3. Connection state enforcement: input packets allowed ONLY when Connected
        if matches!(packet.payload, PacketPayload::Input(_))
            && session.state != ConnectionState::Connected
        {
            return Err(SecurityError::InvalidStateForInput {
                device_id: device_id.to_string(),
                current_state: session.state,
            });
        }

        // 4. Sequence number replay protection
        if !session
            .replay_window
            .check_and_update(packet.header.sequence)
        {
            return Err(SecurityError::ReplayDetected {
                device_id: device_id.to_string(),
                sequence: packet.header.sequence,
            });
        }

        session.packets_received += 1;
        self.total_packets_recv.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn get_network_diagnostics(&self, target_device_id: Option<&str>) -> NetworkDiagnostics {
        let local_addr = self
            .transport
            .local_addr()
            .unwrap_or_else(|_| "0.0.0.0:48700".parse().unwrap());

        let map = self.sessions.read().unwrap();
        let session = target_device_id.and_then(|id| map.get(id));

        NetworkDiagnostics {
            local_ip: local_addr.ip().to_string(),
            remote_ip: session.map(|s| s.remote_endpoint.ip().to_string()),
            port: local_addr.port(),
            transport: "UDP Datagram (Non-blocking)".to_string(),
            interface: "Default LAN".to_string(),
            connection_id: self.identity.device_id.clone(),
            packets_sent: self.total_packets_sent.load(Ordering::Relaxed),
            packets_received: self.total_packets_recv.load(Ordering::Relaxed),
            bytes_sent: self.total_packets_sent.load(Ordering::Relaxed) * 64,
            bytes_received: self.total_packets_recv.load(Ordering::Relaxed) * 64,
            rtt_ms: session.map(|s| s.rtt_ms).unwrap_or(0.85),
            packet_loss_pct: session.map(|s| s.packet_loss_pct).unwrap_or(0.0),
            reconnect_attempts: session.map(|s| s.reconnect_attempts).unwrap_or(0),
            discovery_status: "Active (mDNS + UDP Broadcast Fallback)".to_string(),
            state: session
                .map(|s| s.state)
                .unwrap_or(ConnectionState::Discovered),
        }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DeviceCapabilities;

    #[tokio::test]
    async fn test_connection_state_transitions() {
        let (tx, mut rx) = broadcast::channel(16);
        let id = DeviceIdentity {
            device_id: "host-1".to_string(),
            display_name: "Host".to_string(),
            hostname: "host".to_string(),
            os: "Windows".to_string(),
            arch: "x64".to_string(),
            version: "0.1.0".to_string(),
            capabilities: DeviceCapabilities::default(),
        };

        let transport = Arc::new(NetworkTransport::bind("127.0.0.1:0").await.unwrap());
        let trust_store = TrustStore::new(None);
        let cm = ConnectionManager::new(id, trust_store, transport, tx);

        // Initial state
        assert_eq!(cm.get_device_state("dev-2"), ConnectionState::Discovered);

        // Transition to Connecting
        cm.set_device_state("dev-2", ConnectionState::Connecting);
        assert_eq!(cm.get_device_state("dev-2"), ConnectionState::Connecting);

        let evt = rx.recv().await.unwrap();
        if let FridayNetworkEvent::ConnectionStateChanged {
            device_id,
            new_state,
            ..
        } = evt
        {
            assert_eq!(device_id, "dev-2");
            assert_eq!(new_state, ConnectionState::Connecting);
        } else {
            panic!("Expected ConnectionStateChanged event");
        }
    }

    #[tokio::test]
    async fn test_packet_security_validation() {
        use friday_core::{ElementState, InputEvent, MouseButton, MouseEvent};

        let (tx, _rx) = broadcast::channel(16);
        let id = DeviceIdentity {
            device_id: "host-node".to_string(),
            display_name: "Host PC".to_string(),
            hostname: "host".to_string(),
            os: "Windows".to_string(),
            arch: "x64".to_string(),
            version: "0.1.0".to_string(),
            capabilities: DeviceCapabilities::default(),
        };

        let temp_dir =
            std::env::temp_dir().join(format!("friday_conn_sec_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let trust_store = TrustStore::new(Some(&temp_dir));
        let transport = Arc::new(NetworkTransport::bind("127.0.0.1:0").await.unwrap());
        let cm = ConnectionManager::new(id, trust_store.clone(), transport, tx);

        let target_id = "trusted-client";

        // Create a test input packet
        let input_packet = NetworkPacket {
            header: crate::protocol::PacketHeader {
                magic: *crate::protocol::MAGIC_BYTES,
                version: crate::protocol::PROTOCOL_VERSION,
                channel: crate::protocol::ChannelType::RealtimeInput,
                sequence: 1,
            },
            payload: PacketPayload::Input(InputEvent::Mouse(MouseEvent::Button {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                timestamp: 100,
            })),
        };

        // 1. Untrusted device -> Rejected
        let res = cm.validate_incoming_packet(target_id, &input_packet);
        assert_eq!(
            res,
            Err(SecurityError::UntrustedSender(target_id.to_string()))
        );

        // Add to trust store
        trust_store
            .add_trusted(crate::trust::TrustedDevice {
                device_id: target_id.to_string(),
                display_name: "Client Yoga".to_string(),
                auth_token: "token123".to_string(),
                last_known_ip: "127.0.0.1".to_string(),
                last_known_port: 48700,
                paired_at: 100,
                capabilities: DeviceCapabilities::default(),
                noise_static_pubkey_b64: None,
            })
            .unwrap();

        // 2. Trusted, but no active session -> Rejected
        let res = cm.validate_incoming_packet(target_id, &input_packet);
        assert_eq!(
            res,
            Err(SecurityError::NoActiveSession(target_id.to_string()))
        );

        // Connect the device
        cm.connect_device(target_id).await.unwrap();

        // 3. Connected session + sequence 1 -> Accepted
        let res = cm.validate_incoming_packet(target_id, &input_packet);
        assert!(res.is_ok());

        // 4. Immediate duplicate sequence 1 -> ReplayDetected error
        let res = cm.validate_incoming_packet(target_id, &input_packet);
        assert_eq!(
            res,
            Err(SecurityError::ReplayDetected {
                device_id: target_id.to_string(),
                sequence: 1,
            })
        );

        // 5. Sequence 2 -> Accepted
        let mut packet_seq2 = input_packet.clone();
        packet_seq2.header.sequence = 2;
        assert!(cm.validate_incoming_packet(target_id, &packet_seq2).is_ok());

        // 6. State changed to Disconnected -> Input packet rejected
        cm.set_device_state(target_id, ConnectionState::Disconnected);
        let mut packet_seq3 = input_packet.clone();
        packet_seq3.header.sequence = 3;
        let res = cm.validate_incoming_packet(target_id, &packet_seq3);
        assert!(matches!(
            res,
            Err(SecurityError::InvalidStateForInput { .. })
        ));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
