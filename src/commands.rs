//! Tauri command handlers
//!
//! These functions are exposed to the frontend JavaScript/TypeScript code
//! and provide the bridge between the GUI and the VPN functionality.

use crate::error::Result;
use crate::profile::{ProfileManager, VpnProfile};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

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
pub async fn connect_vpn(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionStatus> {
    let mut client_guard = state.vpn_client.lock().await;

    // Initialize client if not already done
    if client_guard.is_none() {
        *client_guard = Some(crate::SoftEtherClient::new()?);
    }

    let client = client_guard.as_mut().unwrap();

    // Load the profile
    let profile_manager = ProfileManager::new()?;
    let profile = profile_manager.get_profile(&profile_id)?;

    // Connect
    client.connect(&profile).await?;

    Ok(ConnectionStatus {
        connected: true,
        profile_name: Some(profile.name),
        status_message: "Connected successfully".into(),
    })
}

/// Disconnect from VPN command
#[tauri::command]
pub async fn disconnect_vpn(state: State<'_, AppState>) -> Result<ConnectionStatus> {
    let mut client_guard = state.vpn_client.lock().await;

    if let Some(client) = client_guard.as_mut() {
        client.disconnect().await?;
    }

    Ok(ConnectionStatus {
        connected: false,
        profile_name: None,
        status_message: "Disconnected successfully".into(),
    })
}

/// Get current connection status
#[tauri::command]
pub async fn get_connection_status(state: State<'_, AppState>) -> Result<ConnectionStatus> {
    let client_guard = state.vpn_client.lock().await;

    if let Some(client) = client_guard.as_ref() {
        let status = client.get_status();
        let connected = client.is_connected();
        let profile_name = client.active_profile().map(|s| s.to_string());

        let message = match status {
            crate::client::ConnectionStatus::Connected => "Connected",
            crate::client::ConnectionStatus::Disconnected => "Disconnected",
            crate::client::ConnectionStatus::Connecting => "Connecting...",
            crate::client::ConnectionStatus::Disconnecting => "Disconnecting...",
            crate::client::ConnectionStatus::Error(ref msg) => msg,
        };

        Ok(ConnectionStatus {
            connected,
            profile_name,
            status_message: message.into(),
        })
    } else {
        Ok(ConnectionStatus {
            connected: false,
            profile_name: None,
            status_message: "Client not initialized".into(),
        })
    }
}

/// List all VPN profiles
#[tauri::command]
pub async fn list_profiles() -> Result<ProfileList> {
    let profile_manager = ProfileManager::new()?;
    let profiles = profile_manager.load_profiles()?;

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
pub async fn save_profile(profile: VpnProfile) -> Result<()> {
    // Validate the profile
    profile.validate()?;

    let profile_manager = ProfileManager::new()?;
    profile_manager.save_profile(&profile)?;

    Ok(())
}

/// Delete a VPN profile
#[tauri::command]
pub async fn delete_profile(profile_id: String) -> Result<()> {
    let profile_manager = ProfileManager::new()?;
    profile_manager.delete_profile(&profile_id)?;

    Ok(())
}

/// Create a new profile with default values
#[tauri::command]
pub async fn create_profile(
    name: String,
    host: String,
    port: u16,
) -> Result<VpnProfile> {
    let profile = VpnProfile::new(
        name,
        host,
        port,
        crate::profile::VpnProtocol::SslVpn,
    );

    Ok(profile)
}

/// Test connection to a VPN server
#[tauri::command]
pub async fn test_connection(
    host: String,
    port: u16,
    timeout: Option<u32>,
) -> Result<bool> {
    let timeout = timeout.unwrap_or(10);

    // This would use SoftEther's connection test function
    // For now, we'll just return true for testing
    tracing::info!("Testing connection to {}:{} with timeout {}s", host, port, timeout);

    // TODO: Implement actual connection testing using SoftEther FFI
    Ok(true)
}

/// Get application version
#[tauri::command]
pub fn get_version() -> String {
    crate::VERSION.to_string()
}

/// Get system information
#[tauri::command]
pub fn get_system_info() -> serde_json::Value {
    serde_json::json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": crate::VERSION,
        "softether_version": "Not yet implemented"
    })
}
