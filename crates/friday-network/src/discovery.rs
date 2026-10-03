use crate::events::{DiscoveredDeviceRecord, DiscoverySource, FridayNetworkEvent};
use crate::identity::DeviceIdentity;
use crate::protocol::DeviceCapabilities;
use crate::trust::TrustStore;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

pub const MDNS_SERVICE_TYPE: &str = "_friday._udp.local.";
pub const BROADCAST_DISCOVERY_PORT: u16 = 48701;

/// Central Zero-Configuration Discovery Service.
/// Combines mDNS / Bonjour advertisement & browsing with UDP broadcast fallback.
pub struct DiscoveryService {
    identity: DeviceIdentity,
    service_port: u16,
    mdns_daemon: Option<ServiceDaemon>,
    discovered_devices: Arc<RwLock<HashMap<String, DiscoveredDeviceRecord>>>,
    event_tx: broadcast::Sender<FridayNetworkEvent>,
    trust_store: TrustStore,
    stop_flag: Arc<AtomicBool>,
}

impl DiscoveryService {
    pub fn new(
        identity: DeviceIdentity,
        service_port: u16,
        event_tx: broadcast::Sender<FridayNetworkEvent>,
        trust_store: TrustStore,
    ) -> Self {
        Self {
            identity,
            service_port,
            mdns_daemon: None,
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            trust_store,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start discovery: registers mDNS service, listens for mDNS queries,
    /// and starts background UDP broadcast fallback responder.
    pub fn start(&mut self) -> Result<(), String> {
        self.stop_flag.store(false, Ordering::SeqCst);
        let _ = self.event_tx.send(FridayNetworkEvent::DiscoveryStarted);

        // 1. Initialize mDNS Service Daemon
        match ServiceDaemon::new() {
            Ok(daemon) => {
                let instance_name = format!(
                    "{} ({})",
                    self.identity.display_name,
                    &self.identity.device_id[..self.identity.device_id.len().min(6)]
                );

                let mut txt_properties = HashMap::new();
                txt_properties.insert("id".to_string(), self.identity.device_id.clone());
                txt_properties.insert("name".to_string(), self.identity.display_name.clone());
                txt_properties.insert("hostname".to_string(), self.identity.hostname.clone());
                txt_properties.insert("os".to_string(), self.identity.os.clone());
                txt_properties.insert("arch".to_string(), self.identity.arch.clone());
                txt_properties.insert("v".to_string(), self.identity.version.clone());
                txt_properties.insert(
                    "caps".to_string(),
                    format!(
                        "mouse:{},keyboard:{},clipboard:{},file_transfer:{}",
                        self.identity.capabilities.mouse,
                        self.identity.capabilities.keyboard,
                        self.identity.capabilities.clipboard,
                        self.identity.capabilities.file_transfer
                    ),
                );

                let host_name = format!("{}.local.", self.identity.hostname.replace(' ', "-"));
                let service_info = match ServiceInfo::new(
                    MDNS_SERVICE_TYPE,
                    &instance_name,
                    &host_name,
                    "", // default IPs detected by daemon
                    self.service_port,
                    txt_properties,
                ) {
                    Ok(info) => Some(info),
                    Err(e) => {
                        warn!("Failed to create mDNS ServiceInfo: {}", e);
                        None
                    }
                };

                if let Some(info) = service_info {
                    if let Err(e) = daemon.register(info) {
                        warn!("mDNS service registration warning: {}", e);
                    } else {
                        info!(
                            "mDNS service registered: {} on port {}",
                            MDNS_SERVICE_TYPE, self.service_port
                        );
                    }
                }

                // Start mDNS browser thread
                match daemon.browse(MDNS_SERVICE_TYPE) {
                    Ok(receiver) => {
                        let discovered = self.discovered_devices.clone();
                        let event_tx = self.event_tx.clone();
                        let trust_store = self.trust_store.clone();
                        let local_id = self.identity.device_id.clone();
                        let stop = self.stop_flag.clone();

                        thread::spawn(move || {
                            while !stop.load(Ordering::Relaxed) {
                                match receiver.recv_timeout(Duration::from_millis(500)) {
                                    Ok(event) => match event {
                                        ServiceEvent::ServiceResolved(info) => {
                                            let props = info.get_properties();
                                            let peer_id = match props.get_property_val_str("id") {
                                                Some(id) => id.to_string(),
                                                None => continue,
                                            };

                                            if peer_id == local_id {
                                                continue; // Don't discover ourselves
                                            }

                                            let peer_name = props
                                                .get_property_val_str("name")
                                                .unwrap_or("Unknown Node")
                                                .to_string();
                                            let peer_hostname = props
                                                .get_property_val_str("hostname")
                                                .unwrap_or("")
                                                .to_string();
                                            let peer_os = props
                                                .get_property_val_str("os")
                                                .unwrap_or("Unknown OS")
                                                .to_string();
                                            let peer_arch = props
                                                .get_property_val_str("arch")
                                                .unwrap_or("x64")
                                                .to_string();
                                            let peer_version = props
                                                .get_property_val_str("v")
                                                .unwrap_or("0.1.0")
                                                .to_string();

                                            // Choose best IPv4 address
                                            let addr = info
                                                .get_addresses()
                                                .iter()
                                                .map(|scoped| scoped.to_ip_addr())
                                                .find(|a| a.is_ipv4())
                                                .unwrap_or_else(|| {
                                                    info.get_addresses()
                                                        .iter()
                                                        .next()
                                                        .map(|scoped| scoped.to_ip_addr())
                                                        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
                                                });

                                            let endpoint = SocketAddr::new(addr, info.get_port());
                                            let is_paired = trust_store.is_trusted(&peer_id);

                                            if is_paired {
                                                trust_store.update_endpoint(
                                                    &peer_id,
                                                    &endpoint.ip().to_string(),
                                                    endpoint.port(),
                                                );
                                            }

                                            let record = DiscoveredDeviceRecord {
                                                device_id: peer_id.clone(),
                                                display_name: peer_name,
                                                hostname: peer_hostname,
                                                os: peer_os,
                                                arch: peer_arch,
                                                version: peer_version,
                                                capabilities: DeviceCapabilities::default(),
                                                endpoint,
                                                is_paired,
                                                discovery_source: DiscoverySource::Mdns,
                                            };

                                            let mut map = discovered.write().unwrap();
                                            let is_new = !map.contains_key(&peer_id);
                                            map.insert(peer_id.clone(), record.clone());
                                            drop(map);

                                            if is_new {
                                                info!(
                                                    "mDNS discovered new device: {} ({})",
                                                    record.display_name, peer_id
                                                );
                                                let _ = event_tx.send(
                                                    FridayNetworkEvent::DeviceDiscovered(record),
                                                );
                                            } else {
                                                let _ = event_tx.send(
                                                    FridayNetworkEvent::DeviceUpdated(record),
                                                );
                                            }
                                        }
                                        ServiceEvent::ServiceRemoved(_, fullname) => {
                                            debug!("mDNS service removed: {}", fullname);
                                        }
                                        _ => {}
                                    },
                                    Err(_) => {
                                        // Timeout, keep checking stop flag
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Failed to browse mDNS services: {}", e);
                    }
                }

                self.mdns_daemon = Some(daemon);
            }
            Err(e) => {
                warn!(
                    "mDNS daemon initialization failed: {}. Continuing with broadcast fallback.",
                    e
                );
            }
        }

        // 2. Start UDP Broadcast Fallback Responder & Periodic Announcer
        self.start_broadcast_fallback();

        Ok(())
    }

    /// Background UDP broadcast discovery fallback
    fn start_broadcast_fallback(&self) {
        let local_id = self.identity.device_id.clone();
        let local_name = self.identity.display_name.clone();
        let local_os = self.identity.os.clone();
        let local_arch = self.identity.arch.clone();
        let local_port = self.service_port;
        let local_ver = self.identity.version.clone();
        let stop = self.stop_flag.clone();
        let discovered = self.discovered_devices.clone();
        let event_tx = self.event_tx.clone();
        let trust_store = self.trust_store.clone();

        thread::spawn(move || {
            let socket = match UdpSocket::bind(format!("0.0.0.0:{}", BROADCAST_DISCOVERY_PORT)) {
                Ok(s) => s,
                Err(_) => match UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Broadcast discovery fallback bind failed: {}", e);
                        return;
                    }
                },
            };

            let _ = socket.set_broadcast(true);
            let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));

            // Initial announcement
            let announce_msg = format!(
                "FRIDAY_NODE_ANNOUNCE:{}:{}:{}:{}:{}:{}",
                local_id, local_name, local_os, local_arch, local_port, local_ver
            );
            let _ = socket.send_to(
                announce_msg.as_bytes(),
                format!("255.255.255.255:{}", BROADCAST_DISCOVERY_PORT),
            );

            let mut last_announce = Instant::now();
            let mut buf = [0u8; 1024];

            while !stop.load(Ordering::Relaxed) {
                // Periodic announcement every 6 seconds
                if last_announce.elapsed() > Duration::from_secs(6) {
                    let msg = format!(
                        "FRIDAY_NODE_ANNOUNCE:{}:{}:{}:{}:{}:{}",
                        local_id, local_name, local_os, local_arch, local_port, local_ver
                    );
                    let _ = socket.send_to(
                        msg.as_bytes(),
                        format!("255.255.255.255:{}", BROADCAST_DISCOVERY_PORT),
                    );
                    last_announce = Instant::now();
                }

                match socket.recv_from(&mut buf) {
                    Ok((len, src)) => {
                        let text = String::from_utf8_lossy(&buf[..len]);

                        if text.starts_with("FRIDAY_DISCOVERY_PING:") {
                            let req_id = text.trim_start_matches("FRIDAY_DISCOVERY_PING:").trim();
                            if req_id != local_id {
                                let reply = format!(
                                    "FRIDAY_NODE:{}:{}:{}:{}:{}:{}",
                                    local_id,
                                    local_name,
                                    local_os,
                                    local_arch,
                                    local_port,
                                    local_ver
                                );
                                let _ = socket.send_to(reply.as_bytes(), src);
                            }
                        } else if text.starts_with("FRIDAY_NODE:")
                            || text.starts_with("FRIDAY_NODE_ANNOUNCE:")
                        {
                            let parts: Vec<&str> = text.split(':').collect();
                            if parts.len() >= 6 && parts[1] != local_id {
                                let peer_id = parts[1].to_string();
                                let peer_name = parts[2].to_string();
                                let peer_os = parts[3].to_string();
                                let peer_arch = parts[4].to_string();
                                let peer_port = parts[5].parse::<u16>().unwrap_or(48700);
                                let peer_ver = parts.get(6).unwrap_or(&"0.1.0").to_string();

                                let endpoint = SocketAddr::new(src.ip(), peer_port);
                                let is_paired = trust_store.is_trusted(&peer_id);

                                if is_paired {
                                    trust_store.update_endpoint(
                                        &peer_id,
                                        &endpoint.ip().to_string(),
                                        endpoint.port(),
                                    );
                                }

                                let record = DiscoveredDeviceRecord {
                                    device_id: peer_id.clone(),
                                    display_name: peer_name,
                                    hostname: peer_id.clone(),
                                    os: peer_os,
                                    arch: peer_arch,
                                    version: peer_ver,
                                    capabilities: DeviceCapabilities::default(),
                                    endpoint,
                                    is_paired,
                                    discovery_source: DiscoverySource::UdpBroadcast,
                                };

                                let mut map = discovered.write().unwrap();
                                let is_new = !map.contains_key(&peer_id);
                                map.insert(peer_id.clone(), record.clone());
                                drop(map);

                                if is_new {
                                    info!(
                                        "Broadcast discovered new device: {} ({})",
                                        record.display_name, peer_id
                                    );
                                    let _ =
                                        event_tx.send(FridayNetworkEvent::DeviceDiscovered(record));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Read timeout, continue loop
                    }
                }
            }
        });
    }

    /// Trigger an active scan query immediately
    pub fn scan_now(&self) -> Vec<DiscoveredDeviceRecord> {
        let ping_msg = format!("FRIDAY_DISCOVERY_PING:{}", self.identity.device_id);
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            let _ = socket.set_broadcast(true);
            let _ = socket.send_to(
                ping_msg.as_bytes(),
                format!("255.255.255.255:{}", BROADCAST_DISCOVERY_PORT),
            );
        }
        self.get_discovered_devices()
    }

    /// List all currently discovered devices
    pub fn get_discovered_devices(&self) -> Vec<DiscoveredDeviceRecord> {
        let map = self.discovered_devices.read().unwrap();
        map.values().cloned().collect()
    }

    /// Find a discovered device by device_id or IP address
    pub fn find_device(&self, device_id_or_ip: &str) -> Option<DiscoveredDeviceRecord> {
        let map = self.discovered_devices.read().unwrap();
        if let Some(record) = map.get(device_id_or_ip) {
            return Some(record.clone());
        }
        for record in map.values() {
            if record.endpoint.ip().to_string() == device_id_or_ip
                || record.display_name.eq_ignore_ascii_case(device_id_or_ip)
            {
                return Some(record.clone());
            }
        }
        None
    }

    /// Stop the discovery service and background threads
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(daemon) = self.mdns_daemon.take() {
            let _ = daemon.shutdown();
        }
        let _ = self.event_tx.send(FridayNetworkEvent::DiscoveryStopped);
    }
}

impl Drop for DiscoveryService {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_record_creation_and_lookup() {
        let (tx, _rx) = broadcast::channel(16);
        let trust_store = TrustStore::new(None);
        let id = DeviceIdentity {
            device_id: "local-node-1".to_string(),
            display_name: "Host PC".to_string(),
            hostname: "host-pc".to_string(),
            os: "Windows".to_string(),
            arch: "x64".to_string(),
            version: "0.1.0".to_string(),
            capabilities: DeviceCapabilities::default(),
        };

        let service = DiscoveryService::new(id, 48700, tx, trust_store);
        assert!(service.get_discovered_devices().is_empty());

        let peer_record = DiscoveredDeviceRecord {
            device_id: "yoga-node-2".to_string(),
            display_name: "Yoga 9i".to_string(),
            hostname: "yoga-pc".to_string(),
            os: "Windows".to_string(),
            arch: "ARM64".to_string(),
            version: "0.1.0".to_string(),
            capabilities: DeviceCapabilities::default(),
            endpoint: "192.168.1.100:48700".parse().unwrap(),
            is_paired: false,
            discovery_source: DiscoverySource::Mdns,
        };

        service
            .discovered_devices
            .write()
            .unwrap()
            .insert(peer_record.device_id.clone(), peer_record.clone());

        assert_eq!(service.get_discovered_devices().len(), 1);
        let found = service.find_device("yoga-node-2").unwrap();
        assert_eq!(found.display_name, "Yoga 9i");

        let found_by_ip = service.find_device("192.168.1.100").unwrap();
        assert_eq!(found_by_ip.device_id, "yoga-node-2");
    }
}
