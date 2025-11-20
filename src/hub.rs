//! Utilities for interacting with SoftEther RPC endpoints.
//!
//! Currently provides helper functions to enumerate virtual hubs
//! without requiring full JSON-RPC credentials.

use crate::bindings;
use crate::bindings::{
    CLIENT_OPTION, TOKEN_LIST, CEDAR, SESSION, MAX_HOST_NAME_LEN, PROXY_DIRECT, UINT,
};
use crate::error::{Error, Result};
use std::ffi::{c_char, CStr};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tracing::error;

/// Enumerate the list of virtual hubs exposed by the specified server.
pub fn enumerate_virtual_hubs(host: &str, port: u16) -> Result<Vec<String>> {
    if host.trim().is_empty() {
        return Err(Error::ProfileError {
            message: "Server host is required to fetch Virtual Hub names".into(),
        });
    }

    if port == 0 {
        return Err(Error::ProfileError {
            message: "Server port must be greater than zero".into(),
        });
    }

    let cedar = unsafe { bindings::NewCedar(std::ptr::null_mut(), std::ptr::null_mut()) };
    if cedar.is_null() {
        return Err(Error::InitializationFailed {
            message: "Unable to initialize SoftEther Cedar context".into(),
        });
    }
    let cedar_guard = CedarGuard { ptr: cedar };

    let mut option = CLIENT_OPTION::default();
    prepare_client_option(&mut option, host, port)?;

    let mut error_code: UINT = 0;
    let session = unsafe {
        bindings::NewRpcSessionEx(
            cedar_guard.ptr,
            &mut option as *mut CLIENT_OPTION,
            &mut error_code as *mut UINT,
            std::ptr::null(),
        )
    };
    if session.is_null() {
        let diagnostics = connection_diagnostics(host, port);

        if error_code != 0 {
            let se_error = Error::from_softether_error(error_code as i32).to_string();
            let message = format!(
                "SoftEther RPC failed while enumerating hubs on {}:{} ({}). {}",
                host, port, se_error, diagnostics
            );
            error!(target: "geist_vpn::hub", "{}", message);
            return Err(Error::ConnectionFailed { message });
        } else {
            let message = format!(
                "Unable to establish RPC session with VPN server {}:{}. {}",
                host, port, diagnostics
            );
            error!(target: "geist_vpn::hub", "{}", message);
            return Err(Error::ConnectionFailed { message });
        }
    }
    let session_guard = SessionGuard { ptr: session };

    let tokens = unsafe { bindings::EnumHub(session_guard.ptr) };
    if tokens.is_null() {
        return Err(Error::ConnectionFailed {
            message: "Server did not return any Virtual Hub information".into(),
        });
    }

    let hub_names = unsafe { collect_hub_names(tokens) };
    unsafe {
        bindings::FreeToken(tokens);
    }

    Ok(hub_names)
}

/// RAII guard for Cedar pointers.
struct CedarGuard {
    ptr: *mut CEDAR,
}

impl Drop for CedarGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { bindings::ReleaseCedar(self.ptr) };
        }
    }
}

/// RAII guard for Session pointers.
struct SessionGuard {
    ptr: *mut SESSION,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { bindings::ReleaseSession(self.ptr) };
        }
    }
}

fn prepare_client_option(option: &mut CLIENT_OPTION, host: &str, port: u16) -> Result<()> {
    bindings::to_wide_string("geist-hub-enum", &mut option.AccountName)
        .map_err(|e| Error::EncodingError {
            message: format!("Failed to encode account name: {}", e),
        })?;

    write_c_string(&mut option.Hostname, host, MAX_HOST_NAME_LEN)?;
    option.Port = port as bindings::UINT;
    option.PortUDP = 0;
    option.ProxyType = PROXY_DIRECT;
    option.ProxyPort = 0;
    option.NumRetry = 1;
    option.RetryInterval = 1;
    option.MaxConnection = 1;
    option.UseEncrypt = true;
    option.UseCompress = true;
    option.NoRoutingTracking = true;
    option.HideStatusWindow = true;
    option.HideNicInfoWindow = true;
    option.AdditionalConnectionInterval = 1;
    option.ConnectionDisconnectSpan = 0;
    option.NoUdpAcceleration = false;
    option.RequireMonitorMode = false;
    option.RequireBridgeRoutingMode = false;
    option.DisableQoS = false;
    option.FromAdminPack = false;
    option.HalfConnection = false;

    Ok(())
}

unsafe fn collect_hub_names(tokens: *mut TOKEN_LIST) -> Vec<String> {
    if tokens.is_null() {
        return Vec::new();
    }

    let token_ref = &*tokens;
    let num = token_ref.NumTokens as usize;
    if num == 0 || token_ref.Token.is_null() {
        return Vec::new();
    }

    let mut hubs = Vec::with_capacity(num);
    for i in 0..num {
        let entry_ptr = *token_ref.Token.add(i);
        if entry_ptr.is_null() {
            continue;
        }

        let name = CStr::from_ptr(entry_ptr).to_string_lossy().into_owned();
        if !name.is_empty() {
            hubs.push(name);
        }
    }

    hubs
}

fn write_c_string(buffer: &mut [c_char], value: &str, max_len: usize) -> Result<()> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() > max_len {
        return Err(Error::ProfileError {
            message: format!("Value '{}' exceeds maximum length {}", trimmed, max_len),
        });
    }

    for slot in buffer.iter_mut() {
        *slot = 0;
    }

    for (idx, byte) in bytes.iter().enumerate() {
        buffer[idx] = *byte as c_char;
    }

    Ok(())
}

fn connection_diagnostics(host: &str, port: u16) -> String {
    let endpoint = format!("{}:{}", host, port);
    match endpoint.to_socket_addrs() {
        Ok(mut addrs) => {
            let mut last_error: Option<std::io::Error> = None;
            while let Some(addr) = addrs.next() {
                match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                    Ok(_) => {
                        return "TCP connection succeeded, but the VPN server rejected the RPC session (verify SSL certificates and server settings)."
                            .to_string();
                    }
                    Err(err) => {
                        last_error = Some(err);
                    }
                }
            }

            if let Some(err) = last_error {
                format!("TCP connection to {} failed: {}", endpoint, err)
            } else {
                "DNS lookup returned no usable addresses.".to_string()
            }
        }
        Err(err) => format!("DNS resolution for {} failed: {}", host, err),
    }
}
