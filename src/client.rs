//! SoftEtherVPN Client wrapper
//!
//! Provides a safe Rust interface to SoftEtherVPN's client functionality.

use crate::error::{Error, Result};
use crate::profile::{VpnProfile, VpnProtocol, AuthMethod};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

/// SoftEther VPN Client wrapper
pub struct SoftEtherClient {
    /// Internal client handle (pointer to SoftEther CLIENT struct)
    client_handle: *mut std::ffi::c_void,

    /// Current connection state
    connected: bool,

    /// Active profile ID if connected
    active_profile: Option<String>,
}

impl SoftEtherClient {
    /// Create a new SoftEther client instance
    pub fn new() -> Result<Self> {
        unsafe {
            let handle = crate::bindings::CiNewClient();
            if handle.is_null() {
                return Err(Error::InitializationFailed {
                    message: "Failed to create SoftEther client".into(),
                });
            }

            Ok(Self {
                client_handle: handle,
                connected: false,
                active_profile: None,
            })
        }
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

        // Attempt connection (this would call SoftEther FFI)
        unsafe {
            let result = crate::bindings::CtConnect(
                self.client_handle,
                &connect_req as *const _ as *mut std::ffi::c_void,
            );

            if result != 0 {
                return Err(Error::from_softether_error(result));
            }
        }

        self.connected = true;
        self.active_profile = Some(profile.id.clone());

        tracing::info!("Connected to VPN: {}", profile.name);
        Ok(())
    }

    /// Disconnect from the current VPN connection
    pub async fn disconnect(&mut self) -> Result<()> {
        if !self.connected {
            return Ok(()); // Already disconnected
        }

        unsafe {
            let result = crate::bindings::CtDisconnect(self.client_handle);
            if result != 0 {
                return Err(Error::from_softether_error(result));
            }
        }

        self.connected = false;
        let profile_name = self.active_profile.take();

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
    fn create_connect_request(&self, profile: &VpnProfile) -> Result<ConnectionRequest> {
        let account_name = CString::new(profile.account_name.clone())?;
        let hostname = CString::new(profile.host.clone())?;

        // Convert auth method to SoftEther format
        let auth_info = match &profile.auth {
            AuthMethod::Password { username, password } => {
                let username_c = CString::new(username.clone())?;
                let password_c = CString::new(password.clone())?;

                AuthInfo::Password {
                    username: username_c,
                    password: password_c,
                }
            }
            _ => return Err(Error::ConnectionFailed {
                message: "Unsupported authentication method".into(),
            }),
        };

        Ok(ConnectionRequest {
            account_name,
            hostname,
            port: profile.port,
            protocol: profile.protocol.clone(),
            auth_info,
            timeout: profile.timeout,
        })
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
                crate::bindings::CiFreeClient(self.client_handle);
            }
        }
    }
}

/// Internal connection request structure
struct ConnectionRequest {
    account_name: CString,
    hostname: CString,
    port: u16,
    protocol: VpnProtocol,
    auth_info: AuthInfo,
    timeout: u32,
}

/// Authentication information
enum AuthInfo {
    Password {
        username: CString,
        password: CString,
    },
    // Add other auth methods as needed
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
