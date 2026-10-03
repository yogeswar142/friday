use crate::protocol::NetworkPacket;
use friday_core::{CoreError, InputEvent, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;

pub struct NetworkTransport {
    socket: Arc<UdpSocket>,
    sequence: AtomicU32,
}

impl NetworkTransport {
    pub async fn bind(addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind(addr)
            .await
            .map_err(|e| CoreError::RouterError(format!("Socket bind failed: {}", e)))?;
        Ok(Self {
            socket: Arc::new(socket),
            sequence: AtomicU32::new(0),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket
            .local_addr()
            .map_err(|e| CoreError::RouterError(e.to_string()))
    }

    pub async fn send_input_to(&self, event: InputEvent, target_addr: SocketAddr) -> Result<()> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        let packet = NetworkPacket::new_input(event, seq);
        let bytes = packet.encode()?;
        self.socket
            .send_to(&bytes, target_addr)
            .await
            .map_err(|e| CoreError::RouterError(format!("UDP send failed: {}", e)))?;
        Ok(())
    }

    pub async fn recv_packet(&self) -> Result<(NetworkPacket, SocketAddr)> {
        let mut buf = [0u8; 65536];
        let (len, src_addr) = self
            .socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| CoreError::RouterError(format!("UDP recv failed: {}", e)))?;

        let packet = NetworkPacket::decode(&buf[..len])?;
        Ok((packet, src_addr))
    }

    /// Send raw bytes directly to a target address (for pre-encoded packets)
    pub async fn send_raw_to(&self, bytes: &[u8], target_addr: SocketAddr) -> Result<()> {
        self.socket
            .send_to(bytes, target_addr)
            .await
            .map_err(|e| CoreError::RouterError(format!("UDP raw send failed: {}", e)))?;
        Ok(())
    }

    pub fn next_seq(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }
}

/// Zero-allocation, RFC 6479 sliding-window replay protection filter.
///
/// Tracks the highest sequence number observed and maintains a 64-bit bitmap
/// of recently received sequence numbers.
///
/// - In-order packets advance the window.
/// - Out-of-order packets within 64 positions of the head are accepted if not previously seen.
/// - Stale packets (> 64 positions behind) or duplicate packets are rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayWindow {
    last_seq: u32,
    bitmap: u64,
    initialized: bool,
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayWindow {
    pub fn new() -> Self {
        Self {
            last_seq: 0,
            bitmap: 0,
            initialized: false,
        }
    }

    /// Check if `seq` is valid (not replayed or stale) and update the window state.
    /// Returns `true` if packet is accepted, `false` if replayed or too old.
    pub fn check_and_update(&mut self, seq: u32) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.last_seq = seq;
            self.bitmap = 1;
            return true;
        }

        if seq > self.last_seq {
            let diff = seq - self.last_seq;
            if diff < 64 {
                self.bitmap = (self.bitmap << diff) | 1;
            } else {
                self.bitmap = 1;
            }
            self.last_seq = seq;
            true
        } else {
            let diff = self.last_seq - seq;
            if diff >= 64 {
                // Stale: older than the 64-packet window
                false
            } else {
                let bit = 1u64 << diff;
                if (self.bitmap & bit) != 0 {
                    // Replay: already received this sequence number
                    false
                } else {
                    // Out-of-order within window: mark received
                    self.bitmap |= bit;
                    true
                }
            }
        }
    }

    pub fn last_sequence(&self) -> u32 {
        self.last_seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PacketPayload;
    use friday_core::{ElementState, MouseButton, MouseEvent};

    #[tokio::test]
    async fn test_network_transport_loopback() {
        let sender = NetworkTransport::bind("127.0.0.1:0").await.unwrap();
        let receiver = NetworkTransport::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let evt = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Right,
            state: ElementState::Pressed,
            timestamp: 123,
        });

        sender.send_input_to(evt.clone(), recv_addr).await.unwrap();

        let (packet, _src) = receiver.recv_packet().await.unwrap();
        if let PacketPayload::Input(received_evt) = packet.payload {
            assert_eq!(received_evt, evt);
        } else {
            panic!("Expected Input payload");
        }
    }

    #[test]
    fn test_replay_window_in_order_and_duplicates() {
        let mut window = ReplayWindow::new();

        // Initial packet
        assert!(window.check_and_update(100));
        assert_eq!(window.last_sequence(), 100);

        // Immediate duplicate rejected
        assert!(!window.check_and_update(100));

        // In-order forward progression
        assert!(window.check_and_update(101));
        assert!(window.check_and_update(102));
        assert!(window.check_and_update(103));

        // Duplicate of 101 rejected
        assert!(!window.check_and_update(101));
        assert!(!window.check_and_update(103));
    }

    #[test]
    fn test_replay_window_out_of_order_within_window() {
        let mut window = ReplayWindow::new();

        assert!(window.check_and_update(10));
        assert!(window.check_and_update(12)); // skipped 11
        assert_eq!(window.last_sequence(), 12);

        // 11 arrives out-of-order within the window -> accepted
        assert!(window.check_and_update(11));

        // Second time 11 arrives -> rejected as duplicate
        assert!(!window.check_and_update(11));
    }

    #[test]
    fn test_replay_window_stale_packet_rejection() {
        let mut window = ReplayWindow::new();

        assert!(window.check_and_update(10));
        // Advance sequence by 70 packets (> 64)
        assert!(window.check_and_update(80));

        // Packet 10 is now 70 positions behind -> must be rejected as stale
        assert!(!window.check_and_update(10));

        // Packet 16 is 64 positions behind -> stale
        assert!(!window.check_and_update(16));

        // Packet 17 is 63 positions behind (diff = 63 < 64) -> accepted
        assert!(window.check_and_update(17));
        // But duplicate 17 is rejected
        assert!(!window.check_and_update(17));
    }
}
