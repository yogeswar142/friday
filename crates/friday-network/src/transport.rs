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
}
