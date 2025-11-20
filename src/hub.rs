//! Utilities for interacting with SoftEther RPC endpoints.
//!
//! Currently provides helper functions to enumerate virtual hubs
//! without requiring full JSON-RPC credentials.

use crate::bindings;
use crate::bindings::{CLIENT_OPTION, TOKEN_LIST, CEDAR, SESSION, MAX_HOST_NAME_LEN, PROXY_DIRECT};
use crate::error::{Error, Result};
use std::ffi::{c_char, CStr};

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

    let session =
        unsafe { bindings::NewRpcSession(cedar_guard.ptr, &mut option as *mut CLIENT_OPTION) };
    if session.is_null() {
        return Err(Error::ConnectionFailed {
            message: format!("Unable to connect to VPN server {}:{}", host, port),
        });
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
