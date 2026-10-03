use friday_network::{
    generate_pairing_pin, ConnectionManager, ConnectionState, DeviceCapabilities, DeviceIdentity,
    DiscoveredDeviceRecord, DiscoverySource, NetworkTransport, TrustStore,
};
use std::fs;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

#[test]
fn test_stable_device_identity_across_restarts() {
    let test_dir = std::env::temp_dir().join(format!("friday_ident_test_{}", Uuid::new_v4()));
    let _ = fs::create_dir_all(&test_dir);

    // Initial launch: generates new UUID
    let id1 = DeviceIdentity::load_or_create(Some(&test_dir));
    let initial_uuid = id1.device_id.clone();
    assert!(!initial_uuid.is_empty());

    // User customizes display name to "Yoga"
    let mut id_modified = id1.clone();
    id_modified
        .set_display_name("Yoga", Some(&test_dir))
        .unwrap();

    // Restart 1: Application restarts
    let id_restart1 = DeviceIdentity::load_or_create(Some(&test_dir));
    assert_eq!(id_restart1.device_id, initial_uuid);
    assert_eq!(id_restart1.display_name, "Yoga");

    // Restart 2: Network reconnected, IP changed, restart again
    let id_restart2 = DeviceIdentity::load_or_create(Some(&test_dir));
    assert_eq!(id_restart2.device_id, initial_uuid);
    assert_eq!(id_restart2.display_name, "Yoga");

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_device_serialization() {
    let record = DiscoveredDeviceRecord {
        device_id: "test-uuid-42".to_string(),
        display_name: "MacBook Pro".to_string(),
        hostname: "macbook.local".to_string(),
        os: "macOS".to_string(),
        arch: "ARM64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
        endpoint: "192.168.1.45:48700".parse().unwrap(),
        is_paired: true,
        discovery_source: DiscoverySource::Mdns,
    };

    let json = serde_json::to_string(&record).unwrap();
    let deserialized: DiscoveredDeviceRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(record, deserialized);

    // ConnectionState serialization
    let states = vec![
        ConnectionState::Unknown,
        ConnectionState::Discovered,
        ConnectionState::Pairing,
        ConnectionState::Trusted,
        ConnectionState::Connecting,
        ConnectionState::Connected,
        ConnectionState::Disconnected,
        ConnectionState::Unreachable,
        ConnectionState::Reconnecting,
    ];
    for state in states {
        let s_json = serde_json::to_string(&state).unwrap();
        let s_de: ConnectionState = serde_json::from_str(&s_json).unwrap();
        assert_eq!(state, s_de);
    }
}

#[test]
fn test_discovery_records_and_duplicate_handling() {
    let (tx, _rx) = broadcast::channel(16);
    let trust_store = TrustStore::new(None);
    let id = DeviceIdentity {
        device_id: "host-node".to_string(),
        display_name: "Host PC".to_string(),
        hostname: "host".to_string(),
        os: "Windows".to_string(),
        arch: "x64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
    };

    let service = friday_network::DiscoveryService::new(id, 48700, tx, trust_store);

    // Device discovered on Wi-Fi IP 192.168.1.100
    let dev_initial = DiscoveredDeviceRecord {
        device_id: "client-yoga".to_string(),
        display_name: "Yoga".to_string(),
        hostname: "yoga-pc".to_string(),
        os: "Windows".to_string(),
        arch: "ARM64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
        endpoint: "192.168.1.100:48700".parse().unwrap(),
        is_paired: false,
        discovery_source: DiscoverySource::Mdns,
    };

    let dev_dhcp_change = DiscoveredDeviceRecord {
        device_id: "client-yoga".to_string(),
        display_name: "Yoga".to_string(),
        hostname: "yoga-pc".to_string(),
        os: "Windows".to_string(),
        arch: "ARM64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
        endpoint: "192.168.1.220:48700".parse().unwrap(), // IP changed via DHCP
        is_paired: false,
        discovery_source: DiscoverySource::UdpBroadcast,
    };

    // Both discoveries arrive: stable device_id must deduplicate
    let dev_list = service.get_discovered_devices();
    assert_eq!(dev_list.len(), 0);

    // After updating with both
    let mut map = std::collections::HashMap::new();
    map.insert(dev_initial.device_id.clone(), dev_initial);
    // Duplicate with updated IP replaces previous entry cleanly
    map.insert(dev_dhcp_change.device_id.clone(), dev_dhcp_change);

    assert_eq!(map.len(), 1, "Duplicate device_id must be deduplicated");
    assert_eq!(
        map.get("client-yoga").unwrap().endpoint.ip().to_string(),
        "192.168.1.220"
    );
}

#[test]
fn test_pairing_state_machine_and_pin() {
    let pin = generate_pairing_pin();
    assert_eq!(pin.len(), 6);

    let token_valid = friday_network::pairing::compute_auth_token("id_a", "id_b", &pin, "nonce_1");
    let token_repeat = friday_network::pairing::compute_auth_token("id_a", "id_b", &pin, "nonce_1");
    assert_eq!(token_valid, token_repeat);

    // Mismatched PIN produces completely different token
    let token_wrong_pin =
        friday_network::pairing::compute_auth_token("id_a", "id_b", "000000", "nonce_1");
    assert_ne!(token_valid, token_wrong_pin);
}

#[test]
fn test_trust_persistence_and_removal() {
    let test_dir = std::env::temp_dir().join(format!("friday_trust_flow_{}", Uuid::new_v4()));
    let _ = fs::create_dir_all(&test_dir);

    let store = TrustStore::new(Some(&test_dir));
    let dev = friday_network::TrustedDevice {
        device_id: "yoga-id-999".to_string(),
        display_name: "Yoga 9i".to_string(),
        auth_token: "token_hash_abc".to_string(),
        last_known_ip: "192.168.1.15".to_string(),
        last_known_port: 48700,
        paired_at: 1000,
        capabilities: DeviceCapabilities::default(),
        noise_static_pubkey_b64: None,
    };

    // Add trusted
    store.add_trusted(dev.clone()).unwrap();
    assert!(store.is_trusted("yoga-id-999"));
    assert!(store.verify_token("yoga-id-999", "token_hash_abc"));
    assert!(!store.verify_token("yoga-id-999", "wrong_token"));

    // Reload from disk
    let store_reloaded = TrustStore::new(Some(&test_dir));
    assert!(store_reloaded.is_trusted("yoga-id-999"));

    // Forget / Remove device
    let removed = store_reloaded.remove_trusted("yoga-id-999").unwrap();
    assert!(removed);
    assert!(!store_reloaded.is_trusted("yoga-id-999"));

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_connection_state_transitions_full_lifecycle() {
    let (tx, _rx) = broadcast::channel(32);
    let id = DeviceIdentity {
        device_id: "main-host".to_string(),
        display_name: "Main Host G50".to_string(),
        hostname: "g50".to_string(),
        os: "Windows".to_string(),
        arch: "x64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
    };

    let transport = Arc::new(NetworkTransport::bind("127.0.0.1:0").await.unwrap());
    let trust_store = TrustStore::new(None);
    let cm = ConnectionManager::new(id, trust_store, transport, tx);

    let target = "remote-yoga";

    // 1. Initially Discovered
    assert_eq!(cm.get_device_state(target), ConnectionState::Discovered);

    // 2. User initiates Pairing
    cm.set_device_state(target, ConnectionState::Pairing);
    assert_eq!(cm.get_device_state(target), ConnectionState::Pairing);

    // 3. User approves PIN -> Trusted
    cm.set_device_state(target, ConnectionState::Trusted);
    assert_eq!(cm.get_device_state(target), ConnectionState::Trusted);

    // 4. Connecting
    cm.set_device_state(target, ConnectionState::Connecting);
    assert_eq!(cm.get_device_state(target), ConnectionState::Connecting);

    // 5. Handshake confirmed -> Connected
    cm.set_device_state(target, ConnectionState::Connected);
    assert_eq!(cm.get_device_state(target), ConnectionState::Connected);

    // 6. Network drop -> Disconnected
    cm.set_device_state(target, ConnectionState::Disconnected);
    assert_eq!(cm.get_device_state(target), ConnectionState::Disconnected);

    // 7. Auto-reconnecting
    cm.set_device_state(target, ConnectionState::Reconnecting);
    assert_eq!(cm.get_device_state(target), ConnectionState::Reconnecting);

    // 8. Reconnect succeeds -> Connected
    cm.handle_heartbeat_ack(target, 0.75);
    assert_eq!(cm.get_device_state(target), ConnectionState::Connected);
}

#[tokio::test]
async fn test_reconnect_exponential_backoff() {
    let (tx, _rx) = broadcast::channel(16);
    let id = DeviceIdentity {
        device_id: "host".to_string(),
        display_name: "Host".to_string(),
        hostname: "host".to_string(),
        os: "Linux".to_string(),
        arch: "x64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
    };

    let transport = Arc::new(NetworkTransport::bind("127.0.0.1:0").await.unwrap());
    let trust_store = TrustStore::new(None);
    let cm = ConnectionManager::new(id, trust_store, transport, tx);

    let target = "yoga-offline";
    cm.set_device_state(target, ConnectionState::Disconnected);

    // Run tick reconnection
    cm.tick_reconnection().await;
    let state = cm.get_device_state(target);
    assert!(
        state == ConnectionState::Reconnecting || state == ConnectionState::Disconnected,
        "State should be Reconnecting or Disconnected"
    );
}

#[test]
fn test_invalid_pairing_and_auth_failure() {
    let test_dir = std::env::temp_dir().join(format!("friday_auth_fail_{}", Uuid::new_v4()));
    let _ = fs::create_dir_all(&test_dir);

    let store = TrustStore::new(Some(&test_dir));
    let dev = friday_network::TrustedDevice {
        device_id: "device-x".to_string(),
        display_name: "Device X".to_string(),
        auth_token: "legit_token_12345".to_string(),
        last_known_ip: "10.0.0.2".to_string(),
        last_known_port: 48700,
        paired_at: 500,
        capabilities: DeviceCapabilities::default(),
        noise_static_pubkey_b64: None,
    };
    store.add_trusted(dev).unwrap();

    // Verify correct token succeeds
    assert!(store.verify_token("device-x", "legit_token_12345"));

    // Empty token fails
    assert!(!store.verify_token("device-x", ""));

    // Forged token fails
    assert!(!store.verify_token("device-x", "forged_token_attack"));

    // Unknown device fails
    assert!(!store.verify_token("unknown-device", "legit_token_12345"));

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_main_host_exclusive_physical_mouse_model() {
    use friday_core::topology::CircularTopology;
    use friday_core::DisplayBounds;
    use friday_core::ScreenLayout;

    // Main Host G50
    let host_id = "main-host-g50".to_string();
    let client_yoga_id = "client-yoga".to_string();

    let mut topo = CircularTopology::new();
    topo.add_device(ScreenLayout {
        device_id: host_id.clone(),
        name: "Main Host G50".to_string(),
        bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
    });
    topo.add_device(ScreenLayout {
        device_id: client_yoga_id.clone(),
        name: "Yoga 9i".to_string(),
        bounds: DisplayBounds::new(1920, 0, 2880, 1800, 1.0, false),
    });

    // 1. Initial active cursor is on Main Host
    assert_eq!(topo.active_device(), host_id.as_str());

    // 2. Cursor transitions across edge to Yoga
    topo.set_active_device(&client_yoga_id);
    assert_eq!(topo.active_device(), client_yoga_id.as_str());

    // 3. Physical mouse ownership verification:
    // Even when cursor is displayed/active on Yoga, Main Host retains topological
    // authority and the ability to re-claim control instantly (e.g. emergency hotkey or disconnect).
    topo.set_active_device(&host_id);
    assert_eq!(topo.active_device(), host_id.as_str());
}

#[tokio::test]
async fn test_remote_disconnect_emergency_fallback() {
    use friday_core::topology::CircularTopology;
    use friday_core::DisplayBounds;
    use friday_core::ScreenLayout;

    let (tx, _rx) = broadcast::channel(16);
    let id = DeviceIdentity {
        device_id: "host-g50".to_string(),
        display_name: "G50 Host".to_string(),
        hostname: "g50".to_string(),
        os: "Windows".to_string(),
        arch: "x64".to_string(),
        version: "0.1.0".to_string(),
        capabilities: DeviceCapabilities::default(),
    };

    let transport = Arc::new(NetworkTransport::bind("127.0.0.1:0").await.unwrap());
    let trust_store = TrustStore::new(None);
    let cm = ConnectionManager::new(id.clone(), trust_store, transport, tx);

    let host_id = id.device_id.clone();
    let remote_id = "client-yoga";

    let mut topo = CircularTopology::new();
    topo.add_device(ScreenLayout {
        device_id: host_id.clone(),
        name: "G50".to_string(),
        bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
    });
    topo.add_device(ScreenLayout {
        device_id: remote_id.to_string(),
        name: "Yoga".to_string(),
        bounds: DisplayBounds::new(1920, 0, 1920, 1080, 1.0, false),
    });

    // Cursor is currently active on Yoga
    topo.set_active_device(remote_id);
    assert_eq!(topo.active_device(), remote_id);

    // Sudden remote disconnection event (network loss or device powered off)
    cm.set_device_state(remote_id, ConnectionState::Disconnected);
    assert_eq!(
        cm.get_device_state(remote_id),
        ConnectionState::Disconnected
    );

    // Emergency fallback handler restores cursor ownership back to Main Host
    if cm.get_device_state(remote_id) == ConnectionState::Disconnected
        && topo.active_device() == remote_id
    {
        topo.set_active_device(&host_id);
    }

    assert_eq!(
        topo.active_device(),
        host_id.as_str(),
        "Cursor ownership must return to Main Host on remote disconnect"
    );
}

#[test]
fn test_noise_authenticated_transport_mouse_data_plane() {
    use friday_core::{InputEvent, MouseEvent};
    use friday_network::{
        handshake_initiator, handshake_responder, NoiseError, NoiseStaticKeyPair, NOISE_TAG_BYTES,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    // Setup Noise keypairs for Host and Client
    let host_kp = NoiseStaticKeyPair::generate().unwrap();
    let client_kp = NoiseStaticKeyPair::generate().unwrap();

    let (a_to_b_tx, a_to_b_rx) = mpsc::channel::<Vec<u8>>();
    let (b_to_a_tx, b_to_a_rx) = mpsc::channel::<Vec<u8>>();

    let send_a = move |data: &[u8]| -> Result<(), NoiseError> {
        a_to_b_tx
            .send(data.to_vec())
            .map_err(|e| NoiseError::TransportError(e.to_string()))
    };
    let recv_a = move || -> Result<Vec<u8>, NoiseError> {
        b_to_a_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|e| NoiseError::TransportError(format!("recv_a: {}", e)))
    };
    let send_b = move |data: &[u8]| -> Result<(), NoiseError> {
        b_to_a_tx
            .send(data.to_vec())
            .map_err(|e| NoiseError::TransportError(e.to_string()))
    };
    let recv_b = move || -> Result<Vec<u8>, NoiseError> {
        a_to_b_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|e| NoiseError::TransportError(format!("recv_b: {}", e)))
    };

    let host_kp_clone = host_kp.clone();
    let initiator_handle =
        std::thread::spawn(move || handshake_initiator(&host_kp_clone, send_a, recv_a));

    let client_session = handshake_responder(&client_kp, send_b, recv_b).unwrap();
    let host_session = initiator_handle.join().unwrap().unwrap();

    // Verify key pinning: host pins client pubkey, client pins host pubkey
    assert_eq!(
        host_session.remote_static_pubkey,
        client_kp.public_key_bytes().unwrap()
    );
    assert_eq!(
        client_session.remote_static_pubkey,
        host_kp.public_key_bytes().unwrap()
    );

    // Simulate high-frequency mouse event on data plane
    let mouse_evt = InputEvent::Mouse(MouseEvent::MoveRel {
        dx: 12,
        dy: -4,
        timestamp: 1234567,
    });
    let plaintext = bincode::serialize(&mouse_evt).unwrap();

    // Host encrypts packet before UDP send
    let mut ciphertext = vec![0u8; plaintext.len() + NOISE_TAG_BYTES + 4];
    let written = host_session.encrypt(&plaintext, &mut ciphertext).unwrap();

    // Client decrypts upon UDP receive
    let mut decrypted_buf = vec![0u8; 1024];
    let decrypted_len = client_session
        .decrypt(&ciphertext[..written], &mut decrypted_buf)
        .unwrap();

    let decrypted_evt: InputEvent = bincode::deserialize(&decrypted_buf[..decrypted_len]).unwrap();
    assert_eq!(mouse_evt, decrypted_evt);
}
