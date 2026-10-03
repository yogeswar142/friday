use crate::connection::{ConnectionManager, NetworkDiagnostics};
use crate::discovery::DiscoveryService;
use crate::events::{
    ConnectionState, DiscoveredDeviceRecord, FridayNetworkEvent, PairingRequestEvent,
};
use crate::identity::DeviceIdentity;
use crate::pairing::{generate_pairing_pin, PairingManager};
use crate::transport::NetworkTransport;
use crate::trust::{TrustStore, TrustedDevice};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::info;

/// Central FRIDAY Device & Connection Manager.
/// Provides a unified, type-safe API for both GUI and CLI frontends.
#[derive(Clone)]
pub struct DeviceManager {
    identity: Arc<RwLock<DeviceIdentity>>,
    trust_store: TrustStore,
    discovery: Arc<RwLock<DiscoveryService>>,
    pairing: PairingManager,
    connection: ConnectionManager,
    transport: Arc<NetworkTransport>,
    event_tx: broadcast::Sender<FridayNetworkEvent>,
    config_dir: Option<std::path::PathBuf>,
}

impl DeviceManager {
    /// Initialize a new DeviceManager with optional custom config directory (for tests)
    pub async fn new(
        bind_addr: &str,
        peer_port: u16,
        config_dir: Option<&Path>,
    ) -> Result<Self, String> {
        let (event_tx, _event_rx) = broadcast::channel(64);

        // 1. Identity & Trust
        let identity = DeviceIdentity::load_or_create(config_dir);
        let trust_store = TrustStore::new(config_dir);

        // 2. Real-time Transport
        let transport = Arc::new(
            NetworkTransport::bind(bind_addr)
                .await
                .map_err(|e| format!("Failed to bind transport on {}: {}", bind_addr, e))?,
        );

        // 3. Discovery Service
        let discovery_service = DiscoveryService::new(
            identity.clone(),
            peer_port,
            event_tx.clone(),
            trust_store.clone(),
        );

        // 4. Pairing Manager
        let pairing = PairingManager::new(identity.clone(), trust_store.clone(), event_tx.clone());
        pairing.start_responder();

        // 5. Connection Manager
        let connection = ConnectionManager::new(
            identity.clone(),
            trust_store.clone(),
            transport.clone(),
            event_tx.clone(),
        );

        let manager = Self {
            identity: Arc::new(RwLock::new(identity)),
            trust_store,
            discovery: Arc::new(RwLock::new(discovery_service)),
            pairing,
            connection,
            transport,
            event_tx,
            config_dir: config_dir.map(|p| p.to_path_buf()),
        };

        // Start discovery automatically
        if let Ok(mut disc) = manager.discovery.write() {
            let _ = disc.start();
        }

        // Start background auto-reconnect loop
        let cm_clone = manager.connection.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(1000));
            loop {
                interval.tick().await;
                cm_clone.tick_reconnection().await;
            }
        });

        info!("FRIDAY DeviceManager initialized successfully");
        Ok(manager)
    }

    /// Retrieve local device identity
    pub fn local_device(&self) -> DeviceIdentity {
        self.identity.read().unwrap().clone()
    }

    /// Change local human-readable device display name (e.g. "Yoga", "G50")
    pub fn set_local_device_name(&self, name: &str) -> Result<(), String> {
        let mut id = self.identity.write().unwrap();
        id.set_display_name(name, self.config_dir.as_deref())
            .map_err(|e| e.to_string())
    }

    /// List all discovered nearby devices on local LAN
    pub fn get_discovered_devices(&self) -> Vec<DiscoveredDeviceRecord> {
        let disc = self.discovery.read().unwrap();
        disc.get_discovered_devices()
    }

    /// List all paired & trusted devices
    pub fn get_paired_devices(&self) -> Vec<TrustedDevice> {
        self.trust_store.list_trusted()
    }

    /// Active discovery refresh scan
    pub fn scan_now(&self) -> Vec<DiscoveredDeviceRecord> {
        let disc = self.discovery.read().unwrap();
        disc.scan_now()
    }

    /// Start discovery
    pub fn start_discovery(&self) -> Result<(), String> {
        let mut disc = self.discovery.write().unwrap();
        disc.start()
    }

    /// Stop discovery
    pub fn stop_discovery(&self) {
        let mut disc = self.discovery.write().unwrap();
        disc.stop();
    }

    /// Initiate pairing with a target device by ID or IP address
    pub async fn pair_device(
        &self,
        device_id_or_ip: &str,
        custom_pin: Option<&str>,
    ) -> Result<TrustedDevice, String> {
        // Find target device's IP
        let target_ip = {
            let disc = self.discovery.read().unwrap();
            disc.find_device(device_id_or_ip)
                .map(|d| d.endpoint.ip().to_string())
                .unwrap_or_else(|| device_id_or_ip.to_string())
        };

        let pin = custom_pin
            .map(|p| p.to_string())
            .unwrap_or_else(generate_pairing_pin);

        self.pairing.initiate_pairing(&target_ip, &pin).await
    }

    /// Approve incoming pairing request with PIN
    pub fn approve_pairing(&self, pin: &str) -> Result<(), String> {
        self.pairing.respond_to_request(pin, true)
    }

    /// Reject incoming pairing request with PIN
    pub fn reject_pairing(&self, pin: &str) -> Result<(), String> {
        self.pairing.respond_to_request(pin, false)
    }

    /// Get all pending incoming pairing requests
    pub fn get_pending_pairing_requests(&self) -> Vec<PairingRequestEvent> {
        PairingManager::get_pending_requests()
    }

    /// Forget / unpair a device
    pub fn forget_device(&self, device_id: &str) -> Result<bool, String> {
        self.pairing.unpair_device(device_id)
    }

    /// Connect to a trusted device
    pub async fn connect_device(&self, device_id: &str) -> Result<(), String> {
        self.connection.connect_device(device_id).await
    }

    /// Disconnect from a device
    pub async fn disconnect_device(&self, device_id: &str) -> Result<(), String> {
        self.connection.disconnect_device(device_id).await
    }

    /// Get current connection status of a device
    pub fn get_connection_status(&self, device_id: &str) -> ConnectionState {
        self.connection.get_device_state(device_id)
    }

    /// Get detailed network diagnostics for troubleshooting
    pub fn get_network_diagnostics(&self, target_id: Option<&str>) -> NetworkDiagnostics {
        self.connection.get_network_diagnostics(target_id)
    }

    /// Subscribe to typed FRIDAY network events
    pub fn subscribe_events(&self) -> broadcast::Receiver<FridayNetworkEvent> {
        self.event_tx.subscribe()
    }

    /// Underlying transport reference for low-latency mouse routing data plane
    pub fn transport(&self) -> Arc<NetworkTransport> {
        self.transport.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_device_manager_initialization() {
        let unique_dir = std::env::temp_dir().join(format!("friday_dm_test_{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&unique_dir);

        let dm = DeviceManager::new("127.0.0.1:0", 48700, Some(&unique_dir))
            .await
            .unwrap();

        let local = dm.local_device();
        assert!(!local.device_id.is_empty());
        assert!(!local.display_name.is_empty());
        assert_eq!(dm.get_paired_devices().len(), 0);

        dm.set_local_device_name("Custom Laptop").unwrap();
        assert_eq!(dm.local_device().display_name, "Custom Laptop");

        let _ = std::fs::remove_dir_all(&unique_dir);
    }
}
