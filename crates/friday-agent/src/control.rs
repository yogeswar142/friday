use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTrigger {
    pub edge: ScreenEdge,
    pub norm_x: f32,
    pub norm_y: f32,
}

/// Advertised screen geometry sent during handshake
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub device_name: String,
}

/// Control message for session handshake and session management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    /// Sent by incoming connection to announce itself
    Hello {
        device_name: String,
        screen: ScreenInfo,
    },
    /// Sent back acknowledging the connection
    Welcome {
        device_name: String,
        screen: ScreenInfo,
    },
    /// Cursor handoff: this device is passing control to the peer
    /// normalized cursor position on handoff edge
    HandoffControl {
        entry_x_norm: f32,
        entry_y_norm: f32,
    },
    /// Request to return control back to the sender
    ReturnControl {
        entry_x_norm: f32,
        entry_y_norm: f32,
    },
    /// Safety: release all held inputs immediately
    ReleaseAll,
    /// Heartbeat for connection liveness
    Ping {
        seq: u32,
    },
    Pong {
        seq: u32,
    },
    /// Graceful disconnect
    Goodbye,
}

impl ControlMessage {
    pub fn encode(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}
