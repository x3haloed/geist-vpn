//! Tauri command handlers
//!
//! These functions are exposed to the frontend JavaScript/TypeScript code
//! and provide the bridge between the GUI and the VPN functionality.

use geist_vpn::error::Result;
use geist_vpn::profile::{ProfileManager, VpnProfile};
use serde::{Deserialize, Serialize};

/// Connection status response
#[derive(Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub profile_name: Option<String>,
    pub status_message: String,
}

/// Profile list response
#[derive(Serialize, Deserialize)]
pub struct ProfileList {
    pub profiles: Vec<ProfileSummary>,
}

/// Profile summary for listing
#[derive(Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub host: String,
    pub protocol: String,
}

/// Connect to VPN command
#[tauri::command]
pub fn connect_vpn(profile_id: String) -> std::result::Result<ConnectionStatus, String> {
    // For now, create a new client each time
    // TODO: Use a global client instance
    let mut client = geist_vpn::SoftEtherClient::new().map_err(|e| e.to_string())?;

    // Load the profile
    let profile_manager = geist_vpn::profile::ProfileManager::new().map_err(|e| e.to_string())?;
    let profile = profile_manager.get_profile(&profile_id).map_err(|e| e.to_string())?;

    // For now, just return success without actually connecting
    // TODO: Implement actual connection
    Ok(ConnectionStatus {
        connected: true,
        profile_name: Some(profile.name),
        status_message: "Connected successfully".into(),
    })
}

/// Disconnect from VPN command
#[tauri::command]
pub fn disconnect_vpn() -> std::result::Result<ConnectionStatus, String> {
    // For now, just return success without actually disconnecting
    // TODO: Implement actual disconnection
    Ok(ConnectionStatus {
        connected: false,
        profile_name: None,
        status_message: "Disconnected successfully".into(),
    })
}

/// Get current connection status
#[tauri::command]
pub fn get_connection_status() -> std::result::Result<ConnectionStatus, String> {
    // For now, just return disconnected status
    // TODO: Implement actual status checking
    Ok(ConnectionStatus {
        connected: false,
        profile_name: None,
        status_message: "Disconnected".into(),
    })
}

/// List all VPN profiles
#[tauri::command]
pub fn list_profiles() -> std::result::Result<ProfileList, String> {
    let profile_manager = geist_vpn::profile::ProfileManager::new().map_err(|e| e.to_string())?;
    let profiles = profile_manager.load_profiles().map_err(|e| e.to_string())?;

    let profiles = profiles
        .into_iter()
        .map(|p| ProfileSummary {
            id: p.id,
            name: p.name,
            host: p.host,
            protocol: format!("{:?}", p.protocol),
        })
        .collect();

    Ok(ProfileList { profiles })
}

/// Save a VPN profile
#[tauri::command]
pub fn save_profile(profile: VpnProfile) -> std::result::Result<(), String> {
    // Validate the profile
    profile.validate().map_err(|e| e.to_string())?;

    let profile_manager = geist_vpn::profile::ProfileManager::new().map_err(|e| e.to_string())?;
    profile_manager.save_profile(&profile).map_err(|e| e.to_string())?;

    Ok(())
}

/// Delete a VPN profile
#[tauri::command]
pub fn delete_profile(profile_id: String) -> std::result::Result<(), String> {
    let profile_manager = geist_vpn::profile::ProfileManager::new().map_err(|e| e.to_string())?;
    profile_manager.delete_profile(&profile_id).map_err(|e| e.to_string())?;

    Ok(())
}

/// Create a new profile with default values
#[tauri::command]
pub fn create_profile(
    name: String,
    host: String,
    port: u16,
) -> std::result::Result<VpnProfile, String> {
    let profile = geist_vpn::profile::VpnProfile::new(
        name,
        host,
        port,
        geist_vpn::profile::VpnProtocol::SslVpn,
    );

    Ok(profile)
}

/// Test connection to a VPN server
#[tauri::command]
pub fn test_connection(
    host: String,
    port: u16,
    timeout: Option<u32>,
) -> std::result::Result<bool, String> {
    let timeout = timeout.unwrap_or(10);

    // This would use SoftEther's connection test function
    // For now, we'll just return true for testing
    println!("Testing connection to {}:{} with timeout {}s", host, port, timeout);

    // TODO: Implement actual connection testing using SoftEther FFI
    Ok(true)
}

/// Get application version
#[tauri::command]
pub fn get_version() -> String {
    geist_vpn::VERSION.to_string()
}

/// Get system information
#[tauri::command]
pub fn get_system_info() -> serde_json::Value {
    serde_json::json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": geist_vpn::VERSION,
        "softether_version": "Not yet implemented"
    })
}
