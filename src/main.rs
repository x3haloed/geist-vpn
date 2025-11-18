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
            commands::get_profile,
            commands::toggle_profile_favorite,
            commands::get_favorite_profiles,
            commands::get_recent_profiles,
            commands::create_profile,
            commands::test_connection,
            commands::get_version,
            commands::get_system_info,
        ])
        .setup(|app| {
            // Create the main window programmatically for Tauri v2
            let window = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                .title("Geist VPN")
                .inner_size(1200.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .center()
                .build()
                .map_err(|e| {
                    eprintln!("Failed to create main window: {}", e);
                    e
                })?;

            // Additional steps to ensure visibility on macOS
            window.show().unwrap_or_else(|e| eprintln!("Warning: Could not show window: {}", e));
            window.set_focus().unwrap_or_else(|e| eprintln!("Warning: Could not focus window: {}", e));

            println!("Geist VPN window created and should be visible");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
