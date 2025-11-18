//! SoftEtherVPN Client wrapper
//!
//! Provides a safe Rust interface to SoftEtherVPN's client functionality.

use crate::error::{Error, Result};
use crate::profile::VpnProfile;
use tokio::sync::broadcast;

/// SoftEther VPN Client wrapper
pub struct SoftEtherClient {
    /// Internal client handle (pointer to SoftEther CLIENT struct)
    client_handle: *mut std::ffi::c_void,

    /// Current connection state
    connected: bool,

    /// Active profile ID if connected
    active_profile: Option<String>,

    /// Status update channel sender
    status_tx: broadcast::Sender<ConnectionStatus>,
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
                connected: false,
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
            unsafe {
                // Start the client service (required for threading)
                crate::bindings::CtStartClient();
                tracing::info!("SoftEtherVPN client service started");
            }
        });

        init_result
    }

    /// Global cleanup for SoftEtherVPN threading system
    ///
    /// This should be called when the application shuts down.
    pub fn global_cleanup() -> Result<()> {
        unsafe {
            crate::bindings::CtStopClient();
            tracing::info!("SoftEtherVPN client service stopped");
        }
        Ok(())
    }

    /// Subscribe to connection status updates
    ///
    /// Returns a receiver that will get status updates as they happen.
    pub fn subscribe_status(&self) -> broadcast::Receiver<ConnectionStatus> {
        self.status_tx.subscribe()
    }

    /// Send a status update to all subscribers
    fn send_status_update(&self, status: ConnectionStatus) {
        let _ = self.status_tx.send(status);
    }

    /// Connect to a VPN server using the provided profile
    pub async fn connect(&mut self, profile: &VpnProfile) -> Result<()> {
        if self.connected {
            return Err(Error::ConnectionFailed {
                message: "Client is already connected".into(),
            });
        }

        // Validate profile before attempting connection
        profile.validate()?;

        // Create connection request structure
        let connect_req = self.create_connect_request(profile)?;

        // Attempt connection using SoftEther FFI
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

        self.connected = true;
        self.active_profile = Some(profile.id.clone());

        // Send status update
        self.send_status_update(ConnectionStatus::Connected);

        tracing::info!("Connected to VPN: {}", profile.name);
        Ok(())
    }

    /// Disconnect from the current VPN connection
    pub async fn disconnect(&mut self) -> Result<()> {
        if !self.connected {
            return Ok(()); // Already disconnected
        }

        // For disconnect, we need to pass the same connection request
        // In a full implementation, we'd store this, but for now we'll create a minimal one
        if let Some(profile_id) = &self.active_profile {
            // Create a minimal disconnect request
            use crate::memory::strings;
            let account_name_wide = strings::rust_to_softether_wide(profile_id)?;

            let disconnect_req = crate::bindings::RPC_CLIENT_CONNECT {
                AccountName: account_name_wide,
            };

            let disconnect_req = crate::memory::malloc_box(disconnect_req)
                .map_err(|_| Error::FfiError {
                    message: "Failed to allocate disconnect request".into(),
                })?;

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
        }

        self.connected = false;
        self.active_profile = None;

        // Send status update
        self.send_status_update(ConnectionStatus::Disconnected);

        tracing::info!("Disconnected from VPN");
        Ok(())
    }

    /// Get the current connection status
    pub fn get_status(&self) -> ConnectionStatus {
        if !self.connected {
            ConnectionStatus::Disconnected
        } else {
            ConnectionStatus::Connected
        }
    }

    /// Check if the client is currently connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get the active profile ID if connected
    pub fn active_profile(&self) -> Option<&str> {
        self.active_profile.as_deref()
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

impl Drop for SoftEtherClient {
    fn drop(&mut self) {
        if self.connected {
            // Note: In a real implementation, we might want to make this async
            // For now, we'll just log the issue
            tracing::warn!("SoftEtherClient dropped while still connected");
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
        let mut profile = VpnProfile::default();

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
}
