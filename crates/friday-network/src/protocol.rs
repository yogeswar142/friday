use serde::{Deserialize, Serialize};
use friday_core::{CoreError, InputEvent, Result};

pub const MAGIC_BYTES: &[u8; 4] = b"FRDY";
pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChannelType {
    RealtimeInput = 1,
    Control = 2,
    BulkData = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub channel: ChannelType,
    pub sequence: u32,
}

impl Default for PacketHeader {
    fn default() -> Self {
        Self {
            magic: *MAGIC_BYTES,
            version: PROTOCOL_VERSION,
            channel: ChannelType::RealtimeInput,
            sequence: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PacketPayload {
    Input(InputEvent),
    /// Raw binary control message (ControlMessage from friday-agent)
    Control(Vec<u8>),
    ControlHandshake {
        device_id: String,
        device_name: String,
        capabilities: DeviceCapabilities,
    },
    Heartbeat {
        timestamp: u64,
    },
    BulkChunk {
        file_id: String,
        chunk_index: u64,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceCapabilities {
    pub mouse: bool,
    pub keyboard: bool,
    pub clipboard: bool,
    pub file_transfer: bool,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            mouse: true,
            keyboard: true,
            clipboard: true,
            file_transfer: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPacket {
    pub header: PacketHeader,
    pub payload: PacketPayload,
}

impl NetworkPacket {
    pub fn new_input(event: InputEvent, sequence: u32) -> Self {
        Self {
            header: PacketHeader {
                magic: *MAGIC_BYTES,
                version: PROTOCOL_VERSION,
                channel: ChannelType::RealtimeInput,
                sequence,
            },
            payload: PacketPayload::Input(event),
        }
    }

    pub fn new_control(payload_bytes: Vec<u8>) -> Self {
        Self {
            header: PacketHeader {
                magic: *MAGIC_BYTES,
                version: PROTOCOL_VERSION,
                channel: ChannelType::Control,
                sequence: 0,
            },
            payload: PacketPayload::Control(payload_bytes),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| CoreError::SerializationError(e.to_string()))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let packet: Self = bincode::deserialize(bytes)
            .map_err(|e| CoreError::DeserializationError(e.to_string()))?;
        if &packet.header.magic != MAGIC_BYTES {
            return Err(CoreError::InvalidPacket(0));
        }
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use friday_core::{ElementState, MouseButton, MouseEvent};

    #[test]
    fn test_network_packet_roundtrip() {
        let input = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Pressed,
            timestamp: 50,
        });
        let packet = NetworkPacket::new_input(input, 1);
        let encoded = packet.encode().unwrap();
        let decoded = NetworkPacket::decode(&encoded).unwrap();

        assert_eq!(decoded.header.sequence, 1);
        if let PacketPayload::Input(InputEvent::Mouse(MouseEvent::Button { button, .. })) = decoded.payload {
            assert_eq!(button, MouseButton::Left);
        } else {
            panic!("Unexpected payload type");
        }
    }
}
