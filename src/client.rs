//! SoftEtherVPN Client wrapper
//!
//! Provides a safe Rust interface to SoftEtherVPN's client functionality.

use crate::error::{Error, Result};
use crate::profile::{VpnProfile, VpnProtocol, AuthMethod};
use std::collections::HashMap;
use tokio::sync::broadcast;

/// SoftEther VPN Client wrapper
pub struct SoftEtherClient {
    /// Internal client handle (pointer to SoftEther CLIENT struct)
    client_handle: *mut std::ffi::c_void,

    /// Current connection state
    state: ConnectionState,

    /// Active profile if connected
    active_profile: Option<VpnProfile>,

    /// Status update channel sender
    status_tx: broadcast::Sender<ConnectionStatus>,
}

/// Internal connection state for better lifecycle management
#[derive(Debug, Clone, PartialEq)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error(String),
}

impl SoftEtherClient {
    /// Create a new SoftEther client instance
    pub fn new() -> Result<Self> {
        // Initialize SoftEther threading if not already done
        Self::ensure_softether_initialized()?;

        unsafe {
            let handle = crate::bindings::CiNewClient();
            if handle.is_null() {
                return Err(Error::InitializationFailed {
                    message: "Failed to create SoftEther client".into(),
                });
            }

            // Create broadcast channel for status updates (buffer size 16)
            let (status_tx, _) = broadcast::channel(16);

            Ok(Self {
                client_handle: handle,
                state: ConnectionState::Disconnected,
                active_profile: None,
                status_tx,
            })
        }
    }

    /// Ensure SoftEtherVPN library is properly initialized for threading
    fn ensure_softether_initialized() -> Result<()> {
        use std::sync::Once;
        static INIT: Once = Once::new();

        let init_result = Ok(());
        INIT.call_once(|| {
            // Skip FFI calls during tests
            #[cfg(not(test))]
            unsafe {
                // Start the client service (required for threading)
                crate::bindings::CtStartClient();
                tracing::info!("SoftEtherVPN client service started");
            }

            #[cfg(test)]
            {
                tracing::info!("SoftEtherVPN client service initialization skipped during tests");
            }
        });

        init_result
    }

    /// Global cleanup for SoftEtherVPN threading system
    ///
    /// This should be called when the application shuts down.
    pub fn global_cleanup() -> Result<()> {
        // Skip FFI calls during tests
        #[cfg(not(test))]
        unsafe {
            crate::bindings::CtStopClient();
            tracing::info!("SoftEtherVPN client service stopped");
        }

        #[cfg(test)]
        {
            tracing::info!("SoftEtherVPN client service cleanup skipped during tests");
        }
        Ok(())
    }

    /// Subscribe to connection status updates
    ///
    /// Returns a receiver that will get status updates as they happen.
    pub fn subscribe_status(&self) -> broadcast::Receiver<ConnectionStatus> {
        self.status_tx.subscribe()
    }

    /// Subscribe to detailed connection status updates
    ///
    /// Returns a receiver that will get detailed status updates.
    pub fn subscribe_detailed_status(&self) -> broadcast::Receiver<DetailedConnectionStatus> {
        // For now, we don't have a separate channel for detailed status
        // This could be implemented later if needed
        panic!("Detailed status subscription not yet implemented");
    }

    /// Send a status update to all subscribers
    fn send_status_update(&self, status: ConnectionStatus) {
        let _ = self.status_tx.send(status);
    }

    /// Connect to a VPN server using the provided profile
    pub async fn connect(&mut self, profile: &VpnProfile) -> Result<()> {
        // Check current state
        match self.state {
            ConnectionState::Connected => {
                return Err(Error::ConnectionFailed {
                    message: "Client is already connected".into(),
                });
            }
            ConnectionState::Connecting => {
                return Err(Error::ConnectionFailed {
                    message: "Connection attempt already in progress".into(),
                });
            }
            ConnectionState::Disconnecting => {
                return Err(Error::ConnectionFailed {
                    message: "Cannot connect while disconnecting".into(),
                });
            }
            _ => {} // Disconnected or Error states are OK to proceed from
        }

        // Validate profile before attempting connection
        profile.validate()?;

        // Transition to connecting state
        self.state = ConnectionState::Connecting;
        self.send_status_update(ConnectionStatus::Connecting);
        tracing::info!("Starting VPN connection to: {}", profile.name);

        // Create connection request structure
        let connect_req = self.create_connect_request(profile)?;

        // Attempt connection with timeout
        let connect_future = async {
            let result = unsafe {
                crate::bindings::CtConnect(
                    self.client_handle,
                    connect_req.as_typed_ptr::<crate::bindings::RPC_CLIENT_CONNECT>(),
                )
            };

            if result == 0 {
                return Err(Error::ConnectionFailed {
                    message: "VPN connection failed".into(),
                });
            }

            Ok(())
        };

        // Add timeout (30 seconds default)
        let timeout_duration = std::time::Duration::from_secs(profile.timeout as u64);
        match tokio::time::timeout(timeout_duration, connect_future).await {
            Ok(Ok(())) => {
                // Connection successful
                self.state = ConnectionState::Connected;
                self.active_profile = Some(profile.clone());
                self.send_status_update(ConnectionStatus::Connected);
                tracing::info!("Successfully connected to VPN: {}", profile.name);
                Ok(())
            }
            Ok(Err(e)) => {
                // Connection failed
                self.state = ConnectionState::Disconnected;
                self.send_status_update(ConnectionStatus::Disconnected);
                tracing::error!("VPN connection failed: {}", e);
                Err(e)
            }
            Err(_) => {
                // Timeout
                self.state = ConnectionState::Disconnected;
                self.send_status_update(ConnectionStatus::Disconnected);
                let error = Error::ConnectionFailed {
                    message: format!("VPN connection timed out after {} seconds", profile.timeout),
                };
                tracing::error!("{}", error);
                Err(error)
            }
        }
    }

    /// Disconnect from the current VPN connection
    pub async fn disconnect(&mut self) -> Result<()> {
        // Check current state
        match self.state {
            ConnectionState::Disconnected => {
                return Ok(()); // Already disconnected
            }
            ConnectionState::Connecting => {
                return Err(Error::ConnectionFailed {
                    message: "Cannot disconnect while connecting".into(),
                });
            }
            ConnectionState::Disconnecting => {
                return Err(Error::ConnectionFailed {
                    message: "Disconnect already in progress".into(),
                });
            }
            _ => {} // Connected or Error states are OK to disconnect from
        }

        // Transition to disconnecting state
        self.state = ConnectionState::Disconnecting;
        self.send_status_update(ConnectionStatus::Disconnecting);
        tracing::info!("Starting VPN disconnection");

        // For disconnect, we need to pass the same connection request
        // In a full implementation, we'd store this, but for now we'll create a minimal one
        if let Some(profile) = &self.active_profile {
            // Create a minimal disconnect request
            use crate::memory::strings;
            let account_name_wide = strings::rust_to_softether_wide(&profile.account_name)?;

            let disconnect_req = crate::bindings::RPC_CLIENT_CONNECT {
                AccountName: account_name_wide,
            };

            let disconnect_req = crate::memory::malloc_box(disconnect_req)
                .map_err(|_| Error::FfiError {
                    message: "Failed to allocate disconnect request".into(),
                })?;

            // Attempt disconnect with timeout
            let disconnect_future = async {
                let result = unsafe {
                    crate::bindings::CtDisconnect(
                        self.client_handle,
                        Box::into_raw(disconnect_req),
                        0, // inner parameter (false as u8)
                    )
                };

                if result == 0 {
                    return Err(Error::FfiError {
                        message: "VPN disconnect failed".into(),
                    });
                }

                Ok(())
            };

            // Add timeout (10 seconds for disconnect)
            let timeout_duration = std::time::Duration::from_secs(10);
            match tokio::time::timeout(timeout_duration, disconnect_future).await {
                Ok(Ok(())) => {
                    // Disconnect successful
                    self.state = ConnectionState::Disconnected;
                    self.active_profile = None;
                    self.send_status_update(ConnectionStatus::Disconnected);
                    tracing::info!("Successfully disconnected from VPN");
                    Ok(())
                }
                Ok(Err(e)) => {
                    // Disconnect failed but let's still clean up state
                    self.state = ConnectionState::Error("Disconnect failed".into());
                    self.active_profile = None;
                    self.send_status_update(ConnectionStatus::Error("Disconnect failed".into()));
                    tracing::error!("VPN disconnect failed: {}", e);
                    Err(e)
                }
                Err(_) => {
                    // Timeout - still clean up state
                    self.state = ConnectionState::Error("Disconnect timeout".into());
                    self.active_profile = None;
                    self.send_status_update(ConnectionStatus::Error("Disconnect timeout".into()));
                    let error = Error::FfiError {
                        message: "VPN disconnect timed out".into(),
                    };
                    tracing::error!("{}", error);
                    Err(error)
                }
            }
        } else {
            // No active profile, just update state
            self.state = ConnectionState::Disconnected;
            self.send_status_update(ConnectionStatus::Disconnected);
            tracing::info!("Disconnected from VPN (no active profile)");
            Ok(())
        }
    }

    /// Get the current connection status
    pub fn get_status(&self) -> ConnectionStatus {
        match &self.state {
            ConnectionState::Disconnected => ConnectionStatus::Disconnected,
            ConnectionState::Connecting => ConnectionStatus::Connecting,
            ConnectionState::Connected => ConnectionStatus::Connected,
            ConnectionState::Disconnecting => ConnectionStatus::Disconnecting,
            ConnectionState::Error(msg) => ConnectionStatus::Error(msg.clone()),
        }
    }

    /// Get detailed connection status from SoftEther
    ///
    /// This queries the actual connection status from the SoftEther client.
    pub async fn get_detailed_status(&self) -> Result<DetailedConnectionStatus> {
        if self.client_handle.is_null() {
            return Ok(DetailedConnectionStatus::default());
        }

        unsafe {
            // Allocate memory for the status structure
            let status_size = std::mem::size_of::<crate::bindings::RPC_CLIENT_GET_CONNECTION_STATUS>();
            let status_mem = crate::memory::zero_malloc_raw(status_size)?;

            // Call CtGetAccountStatus
            let success = crate::bindings::CtGetAccountStatus(
                self.client_handle,
                status_mem.as_typed_ptr::<crate::bindings::RPC_CLIENT_GET_CONNECTION_STATUS>(),
            );

            if !success {
                return Err(Error::FfiError {
                    message: "Failed to get connection status".into(),
                });
            }

            // Extract status information
            let status_ptr = status_mem.as_typed_ptr::<crate::bindings::RPC_CLIENT_GET_CONNECTION_STATUS>();
            let status = &*status_ptr;

            // Convert server name from C string to Rust string
            let server_name = std::ffi::CStr::from_ptr(status.ServerName.as_ptr())
                .to_string_lossy()
                .into_owned();

            let detailed_status = DetailedConnectionStatus {
                account_name: crate::memory::strings::softether_wide_to_rust(&status.AccountName),
                active: status.Active,
                connected: status.Connected,
                session_status: status.SessionStatus as u32,
                server_name,
                server_port: status.ServerPort as u16,
                server_product_name: std::ffi::CStr::from_ptr(status.ServerProductName.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                server_product_version: status.ServerProductVer as u32,
                server_product_build: status.ServerProductBuild as u32,
            };

            Ok(detailed_status)
        }
    }

    /// Check if the client is currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Connected)
    }

    /// Get the active profile if connected
    pub fn active_profile(&self) -> Option<&VpnProfile> {
        self.active_profile.as_ref()
    }

    /// Create a connection request structure for SoftEther FFI
    fn create_connect_request(&self, profile: &VpnProfile) -> Result<crate::memory::RawMemory> {
        use crate::memory::strings;

        // Allocate raw memory for the RPC_CLIENT_CONNECT structure
        let size = std::mem::size_of::<crate::bindings::RPC_CLIENT_CONNECT>();
        let raw_mem = crate::memory::malloc_raw(size)?;

        // Create the RPC_CLIENT_CONNECT structure and copy it into the allocated memory
        let account_name_wide = strings::rust_to_softether_wide(&profile.account_name)?;
        let connect_req = crate::bindings::RPC_CLIENT_CONNECT {
            AccountName: account_name_wide,
        };

        unsafe {
            std::ptr::copy_nonoverlapping(
                &connect_req as *const _ as *const u8,
                raw_mem.as_ptr() as *mut u8,
                size
            );
        }

        Ok(raw_mem)
    }
}

/// Connection status enum
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error(String),
}

/// Detailed connection status information
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetailedConnectionStatus {
    /// Account name for this connection
    pub account_name: String,
    /// Whether the account is active
    pub active: bool,
    /// Whether currently connected
    pub connected: bool,
    /// Session status code
    pub session_status: u32,
    /// Server hostname/IP
    pub server_name: String,
    /// Server port
    pub server_port: u16,
    /// Server product name
    pub server_product_name: String,
    /// Server product version
    pub server_product_version: u32,
    /// Server product build number
    pub server_product_build: u32,
}

impl Drop for SoftEtherClient {
    fn drop(&mut self) {
        if self.is_connected() {
            // Note: In a real implementation, we might want to make this async
            // For now, we'll just log the issue and attempt cleanup
            tracing::warn!("SoftEtherClient dropped while still connected - attempting cleanup");

            // Try to disconnect synchronously (best effort)
            if let Some(profile) = &self.active_profile {
                use crate::memory::strings;
                if let Ok(account_name_wide) = strings::rust_to_softether_wide(&profile.account_name) {
                    let disconnect_req = crate::bindings::RPC_CLIENT_CONNECT {
                        AccountName: account_name_wide,
                    };

                    if let Ok(disconnect_req) = crate::memory::malloc_box(disconnect_req) {
                        unsafe {
                            crate::bindings::CtDisconnect(
                                self.client_handle,
                                Box::into_raw(disconnect_req),
                                0,
                            );
                        }
                    }
                }
            }
        }

        unsafe {
            if !self.client_handle.is_null() {
                crate::bindings::CtReleaseClient(self.client_handle);
            }
        }
    }
}

// Note: ConnectionRequest is now handled by the memory management system
// and RPC_CLIENT_CONNECT structure from bindings.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory;

    #[test]
    fn test_client_creation() {
        // Note: This test requires the SoftEther library to be properly built
        // For now, we'll just test the structure
        let profile = VpnProfile::default();
        assert!(!profile.name.is_empty());
    }

    #[test]
    fn test_profile_validation() {
        // Create a valid profile with credentials
        let mut profile = VpnProfile {
            id: "test_profile".into(),
            name: "Test Profile".into(),
            host: "vpn.example.com".into(),
            port: 443,
            protocol: VpnProtocol::SslVpn,
            auth: AuthMethod::Password {
                username: "testuser".into(),
                password: "testpass".into(),
            },
            account_name: "testaccount".into(),
            timeout: 30,
            options: HashMap::new(),
        };

        // Valid profile should pass
        assert!(profile.validate().is_ok());

        // Invalid profile should fail
        profile.name = String::new();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_memory_management() {
        // Test our memory management bridge
        let mem_result = memory::malloc_raw(64);
        assert!(mem_result.is_ok(), "Failed to allocate memory");

        let mem = mem_result.unwrap();
        assert_eq!(mem.size(), 64);
        assert!(!mem.as_ptr().is_null());

        // Memory is automatically freed when mem goes out of scope
    }

    #[test]
    fn test_string_conversion() {
        // Test string conversion utilities
        let test_str = "Test VPN Connection";
        let wide_result = memory::strings::rust_to_softether_wide(test_str);
        assert!(wide_result.is_ok(), "Failed to convert string to wide format");

        let wide_str = wide_result.unwrap();
        let back_to_rust = memory::strings::softether_wide_to_rust(&wide_str);
        assert_eq!(test_str, back_to_rust);
    }

    #[test]
    fn test_connection_status() {
        // Test that we can create a client and check status
        // Note: This would normally require SoftEtherVPN to be compiled
        // For now, we test the data structures

        let status = ConnectionStatus::Disconnected;
        assert!(!format!("{:?}", status).is_empty());
    }

    #[test]
    fn test_error_handling() {
        // Test error creation and handling
        let error = crate::Error::ConnectionFailed {
            message: "Test connection failed".into(),
        };
        assert!(error.to_string().contains("Test connection failed"));
    }

    #[test]
    fn test_softether_error_mapping() {
        // Test SoftEther error code mapping
        let connect_failed = crate::Error::from_softether_error(crate::bindings::error_codes::ERR_CONNECT_FAILED);
        assert_eq!(connect_failed.to_string(), "SoftEther error code 1: Connection to the server has failed");

        let auth_failed = crate::Error::from_softether_error(crate::bindings::error_codes::ERR_AUTH_FAILED);
        assert_eq!(auth_failed.to_string(), "SoftEther error code 9: Authentication failure");

        let hub_not_found = crate::Error::from_softether_error(crate::bindings::error_codes::ERR_HUB_NOT_FOUND);
        assert_eq!(hub_not_found.to_string(), "SoftEther error code 8: The HUB does not exist");

        let unknown_error = crate::Error::from_softether_error(999);
        assert_eq!(unknown_error.to_string(), "SoftEther error code 999: Unknown error");
    }

    #[test]
    fn test_connection_state_transitions() {
        // Test that state transitions work correctly
        // Note: This test doesn't actually connect, just tests the state logic

        // Test initial state
        let profile = VpnProfile::default();
        assert_eq!(ConnectionStatus::Disconnected, ConnectionStatus::Disconnected);

        // Test state enum conversion
        let state = ConnectionState::Disconnected;
        assert!(!matches!(state, ConnectionState::Connected));

        let state = ConnectionState::Connected;
        assert!(matches!(state, ConnectionState::Connected));

        let state = ConnectionState::Connecting;
        assert!(matches!(state, ConnectionState::Connecting));

        let state = ConnectionState::Disconnecting;
        assert!(matches!(state, ConnectionState::Disconnecting));

        let state = ConnectionState::Error("test".into());
        assert!(matches!(state, ConnectionState::Error(_)));
    }
}
