pub mod commands;
pub mod config;
pub mod diagnostics;
pub mod discovery;
pub mod engine;
pub mod network_report;
pub mod pairing;
pub mod state;
pub mod types;

use state::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

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
    // Also wires Tauri event emission when a pair request arrives (event-driven, not polling)
    pairing::start_pairing_responder(shared_state.clone(), stop_flag.clone());

    // Launch background Mouse Topology Engine service
    engine::start_engine_service(shared_state.clone(), stop_flag.clone());

    tauri::Builder::default()
        .manage(shared_state)
        // Wire event-driven pair request notifications to the frontend:
        // The pairing responder thread pushes pair requests to PENDING_REQUESTS and signals
        // a channel. Here we register a setup hook to subscribe and emit Tauri events.
        .setup(|app| {
            // Spawn a background thread that polls the Tauri-bound pair notification channel
            // and forwards events to the WebView via emit_all().
            // We use the notification channel set up in pairing::start_pairing_responder().
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                // Block on the pair-notification receiver for event-driven delivery.
                // This replaces the 2-second frontend polling.
                loop {
                    if let Some(req) = pairing::wait_for_next_pair_request() {
                        // Emit directly to all WebView windows — no polling delay.
                        let _ = app_handle.emit("friday:pair_request", &req);
                    }
                }
            });
            Ok(())
        })
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
            commands::clear_logs,
            commands::initiate_pairing,
            commands::get_pending_pair_requests,
            commands::respond_to_pair_request,
            commands::set_device_role,
            commands::get_local_device,
            commands::set_local_device_name,
            commands::get_discovered_devices,
            commands::get_paired_devices,
            commands::start_discovery,
            commands::stop_discovery,
            commands::approve_pairing,
            commands::reject_pairing,
            commands::forget_device,
            commands::set_device_input_preferences,
            commands::set_host_input_preferences,
            commands::get_connection_status,
            commands::get_network_diagnostics,
            commands::get_network_working_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FRIDAY desktop application");
}
