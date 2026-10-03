use crate::protocol::DeviceCapabilities;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Connection lifecycle states for a FRIDAY device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Device state has not yet been determined
    Unknown,
    /// Device is discovered on the local network but not paired
    Discovered,
    /// Actively undergoing user pairing code verification
    Pairing,
    /// Successfully paired and trusted, but no active network session
    Trusted,
    /// Establishing authenticated handshake session
    Connecting,
    /// Fully connected and transmitting/receiving input or heartbeats
    Connected,
    /// Disconnected session (was previously connected)
    Disconnected,
    /// Unreachable due to packet loss or heartbeat timeout
    Unreachable,
    /// Actively attempting exponential backoff reconnection
    Reconnecting,
}

impl ConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Discovered => "Discovered",
            Self::Pairing => "Pairing",
            Self::Trusted => "Trusted",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
            Self::Disconnected => "Disconnected",
            Self::Unreachable => "Unreachable",
            Self::Reconnecting => "Reconnecting",
        }
    }
}

/// Information about a device discovered on the LAN via mDNS or broadcast
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredDeviceRecord {
    pub device_id: String,
    pub display_name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub capabilities: DeviceCapabilities,
    pub endpoint: SocketAddr,
    pub is_paired: bool,
    pub discovery_source: DiscoverySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoverySource {
    Mdns,
    UdpBroadcast,
    Manual,
}

/// Incoming pair request notification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingRequestEvent {
    pub pin: String,
    pub from_id: String,
    pub from_name: String,
    pub from_ip: String,
    pub reply_port: u16,
}

/// Typed events emitted by the FRIDAY network engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FridayNetworkEvent {
    /// A new FRIDAY device appeared on the local network
    DeviceDiscovered(DiscoveredDeviceRecord),
    /// An already discovered device updated its endpoint or details
    DeviceUpdated(DiscoveredDeviceRecord),
    /// A device disappeared from mDNS or timed out
    DeviceLost(String),
    /// Incoming pairing request from a remote peer
    PairingRequested(PairingRequestEvent),
    /// Pairing completed (accepted and saved to trust store)
    PairingCompleted {
        device_id: String,
        device_name: String,
        success: bool,
        message: String,
    },
    /// Pairing failed (rejected or timed out)
    PairingFailed { device_id: String, reason: String },
    /// Connected to remote device
    DeviceConnected { device_id: String, endpoint: String },
    /// Disconnected from remote device
    DeviceDisconnected { device_id: String, reason: String },
    /// Connection state transition
    ConnectionStateChanged {
        device_id: String,
        old_state: ConnectionState,
        new_state: ConnectionState,
    },
    /// Discovery service started
    DiscoveryStarted,
    /// Discovery service stopped
    DiscoveryStopped,
}
