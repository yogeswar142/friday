pub mod commands;
pub mod config;
pub mod diagnostics;
pub mod state;
pub mod types;

use std::sync::{Arc, Mutex};
use state::AppState;

pub fn run() {
    let shared_state = Arc::new(Mutex::new(AppState::new()));

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running FRIDAY desktop application");
}
