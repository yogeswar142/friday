pub mod connection;
pub mod discovery;
pub mod events;
pub mod identity;
pub mod manager;
pub mod noise_session;
pub mod pairing;
pub mod protocol;
pub mod transport;
pub mod trust;

pub use connection::{ConnectionManager, ManagedSession, NetworkDiagnostics, SecurityError};
pub use discovery::DiscoveryService;
pub use events::{
    ConnectionState, DiscoveredDeviceRecord, DiscoverySource, FridayNetworkEvent,
    PairingRequestEvent,
};
pub use identity::DeviceIdentity;
pub use manager::DeviceManager;
pub use noise_session::{
    handshake_initiator, handshake_responder, is_noise_packet, NoiseError, NoiseSession,
    NoiseStaticKeyPair, NOISE_TAG_BYTES,
};
pub use pairing::{generate_pairing_pin, PairingManager};
pub use protocol::{ChannelType, DeviceCapabilities, NetworkPacket, PacketHeader, PacketPayload};
pub use transport::{NetworkTransport, ReplayWindow};
pub use trust::{TrustStore, TrustedDevice};
