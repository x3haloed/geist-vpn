#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use geist_vpn::SoftEtherClient;
use std::sync::Arc;
use tauri::{AppHandle, Manager, TrayIconBuilder, Menu, MenuItem, Submenu};
use tokio::sync::Mutex;

mod commands;

fn setup_system_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create tray menu
    let menu = Menu::new(app)?
        .add_item(MenuItem::new(app, "Show Geist VPN", true, None)?)?
        .add_native_item(MenuItem::Separator)?
        .add_item(MenuItem::new(app, "Connect", true, None)?)?
        .add_item(MenuItem::new(app, "Disconnect", true, None)?)?
        .add_native_item(MenuItem::Separator)?
        .add_item(MenuItem::new(app, "Quit", true, None)?)?;

    // Create tray icon
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Geist VPN")
        .build(app)?;

    Ok(())
}

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
            commands::create_profile,
            commands::test_connection,
            commands::get_version,
            commands::get_system_info,
        ])
        .setup(|app| {
            // Configure the main window
            let window = app.get_window("main").unwrap();

            // Set window properties
            window.set_title("Geist VPN").unwrap();

            // On macOS, set up proper window behavior
            #[cfg(target_os = "macos")]
            {
                use tauri::TitleBarStyle;
                window.set_title_bar_style(TitleBarStyle::Transparent).unwrap();
            }

            // Set up system tray
            setup_system_tray(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
