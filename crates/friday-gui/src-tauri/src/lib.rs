pub mod commands;
pub mod config;
pub mod diagnostics;
pub mod discovery;
pub mod pairing;
pub mod state;
pub mod types;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use state::AppState;

pub fn run() {
    // On Linux, WebKit 2.52+ with DMA-BUF renderer can fail to load the tauri://
    // custom URI scheme, showing a white screen with "Connection refused".
    // Setting these env vars before GTK/WebKit initializes fixes the issue.
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    let shared_state = Arc::new(Mutex::new(AppState::new()));
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Launch background UDP discovery service
    discovery::start_discovery_service(shared_state.clone(), stop_flag.clone());

    // Launch background UDP pairing responder (listens for incoming pair requests)
    pairing::start_pairing_responder(stop_flag.clone());

    tauri::Builder::default()
        .manage(shared_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::start_engine,
            commands::stop_engine,
            commands::pause_engine,
            commands::get_active_device,
            commands::switch_active_device,
            commands::get_devices,
            commands::discover_devices,
            commands::add_manual_device,
            commands::pair_device,
            commands::unpair_device,
            commands::connect_device,
            commands::disconnect_device,
            commands::get_topology,
            commands::set_topology,
            commands::reorder_ring,
            commands::add_ring_device,
            commands::remove_ring_device,
            commands::get_telemetry,
            commands::run_diagnostics,
            commands::run_benchmark,
            commands::get_settings,
            commands::save_settings,
            commands::get_platform_capabilities,
            commands::get_platform_permissions,
            commands::open_permission_settings,
            commands::get_logs,
            commands::initiate_pairing,
            commands::get_pending_pair_requests,
            commands::respond_to_pair_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FRIDAY desktop application");
}
