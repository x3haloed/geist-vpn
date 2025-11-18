#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use geist_vpn::SoftEtherClient;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

mod commands;

#[derive(Clone)]
pub struct AppState {
    pub vpn_client: Arc<Mutex<Option<SoftEtherClient>>>,
}

fn main() {
    tracing_subscriber::init();

    tauri::Builder::default()
        .manage(AppState {
            vpn_client: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_vpn,
            commands::disconnect_vpn,
            commands::get_connection_status,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
