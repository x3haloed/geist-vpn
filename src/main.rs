#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::Manager;

mod commands;

#[derive(Clone)]
pub struct AppState {
    // For now, we'll manage the VPN client separately
    // to avoid Send/Sync issues with raw pointers
}

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .manage(AppState {})
        .invoke_handler(tauri::generate_handler![
            commands::connect_vpn,
            commands::disconnect_vpn,
            commands::get_connection_status,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::create_profile,
            commands::test_connection,
            commands::get_version,
            commands::get_system_info,
        ])
        .setup(|app| {
            // Configure the main window
            let window = app.get_webview_window("main").unwrap();

            // Set window properties
            window.set_title("Geist VPN").unwrap();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
