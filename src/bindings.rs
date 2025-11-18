//! FFI bindings for SoftEtherVPN
//!
//! This module contains the unsafe foreign function interface declarations
//! for interacting with the SoftEtherVPN C libraries.

use std::os::raw::{c_char, c_uint, c_void};

// Basic types from SoftEther
pub type UINT = c_uint;
pub type UINT64 = u64;
pub type UCHAR = u8;
pub type SoftEtherBool = std::os::raw::c_uchar; // SoftEther uses UCHAR for bool

// Maximum string lengths (from SoftEther constants)
pub const MAX_ACCOUNT_NAME_LEN: usize = 127;
pub const MAX_HOST_NAME_LEN: usize = 255;
pub const SHA1_SIZE: usize = 20;

// Memory management (SoftEther custom allocators)
extern "C" {
    /// SoftEther malloc
    pub fn Malloc(size: UINT) -> *mut c_void;

    /// SoftEther zero malloc
    pub fn ZeroMalloc(size: UINT) -> *mut c_void;

    /// SoftEther free
    pub fn Free(ptr: *mut c_void);

    /// SoftEther realloc
    pub fn ReAlloc(ptr: *mut c_void, size: UINT) -> *mut c_void;
}

// Client management functions
extern "C" {
    /// Create a new VPN client instance
    pub fn CiNewClient() -> *mut c_void;

    /// Free a VPN client instance
    pub fn CtReleaseClient(client: *mut c_void);

    /// Start the VPN client service
    pub fn CtStartClient();

    /// Stop the VPN client service
    pub fn CtStopClient();

    /// Connect to a VPN server
    pub fn CtConnect(
        client: *mut c_void,
        connect_req: *mut RPC_CLIENT_CONNECT,
    ) -> SoftEtherBool;

    /// Disconnect from VPN server
    pub fn CtDisconnect(
        client: *mut c_void,
        connect_req: *mut RPC_CLIENT_CONNECT,
        inner: SoftEtherBool,
    ) -> SoftEtherBool;

    /// Get connection status
    pub fn CtGetAccountStatus(
        client: *mut c_void,
        status: *mut RPC_CLIENT_GET_CONNECTION_STATUS,
    ) -> bool;
}

// Account management functions
extern "C" {
    /// Create a new account
    pub fn CtCreateAccount(
        client: *mut c_void,
        account_req: *mut RPC_CLIENT_CREATE_ACCOUNT,
        inner: bool,
    ) -> bool;

    /// Enumerate accounts
    pub fn CtEnumAccount(
        client: *mut c_void,
        accounts: *mut RPC_CLIENT_ENUM_ACCOUNT,
    ) -> bool;

    /// Delete an account
    pub fn CtDeleteAccount(
        client: *mut c_void,
        account_req: *mut RPC_CLIENT_DELETE_ACCOUNT,
        inner: bool,
    ) -> bool;

    /// Get account details
    pub fn CtGetAccount(
        client: *mut c_void,
        account_req: *mut RPC_CLIENT_GET_ACCOUNT,
    ) -> bool;

    /// Set account details
    pub fn CtSetAccount(
        client: *mut c_void,
        account_req: *mut RPC_CLIENT_CREATE_ACCOUNT,
        inner: bool,
    ) -> bool;
}

// RPC Structures (repr(C) for FFI compatibility)
#[repr(C)]
pub struct RPC_CLIENT_CONNECT {
    pub AccountName: [u16; MAX_ACCOUNT_NAME_LEN + 1], // Wide char array
}

#[repr(C)]
pub struct RPC_CLIENT_GET_CONNECTION_STATUS {
    pub AccountName: [u16; MAX_ACCOUNT_NAME_LEN + 1],
    pub Active: bool,
    pub Connected: bool,
    pub SessionStatus: UINT,
    pub ServerName: [c_char; MAX_HOST_NAME_LEN + 1],
    pub ServerPort: UINT,
    pub ServerProductName: [c_char; 256], // MAX_SIZE
    pub ServerProductVer: UINT,
    pub ServerProductBuild: UINT,
    // ... many more fields, simplified for now
}

#[repr(C)]
pub struct RPC_CLIENT_CREATE_ACCOUNT {
    // This is a complex structure, simplified for initial implementation
    pub AccountName: [u16; MAX_ACCOUNT_NAME_LEN + 1],
    // ... other fields to be added
}

#[repr(C)]
pub struct RPC_CLIENT_ENUM_ACCOUNT {
    // Structure for enumerating accounts
    pub NumItem: UINT,
    // ... other fields
}

#[repr(C)]
pub struct RPC_CLIENT_DELETE_ACCOUNT {
    pub AccountName: [u16; MAX_ACCOUNT_NAME_LEN + 1],
}

#[repr(C)]
pub struct RPC_CLIENT_GET_ACCOUNT {
    pub AccountName: [u16; MAX_ACCOUNT_NAME_LEN + 1],
    // ... other fields
}

// Library initialization (these may not exist, need to check)
extern "C" {
    /// Initialize SoftEtherVPN library (if available)
    pub fn InitSoftEther() -> bool;

    /// Cleanup SoftEtherVPN library (if available)
    pub fn FreeSoftEther() -> bool;
}

// String utilities
extern "C" {
    /// Copy unicode string
    pub fn CopyUniStr(dst: *mut u16, src: *const u16) -> *mut u16;

    /// Free string
    pub fn FreeStr(str: *mut c_char);
}

// Thread management
extern "C" {
    /// Sleep for specified milliseconds
    pub fn SleepThread(millis: UINT);
}

/// Safe wrapper for SoftEther malloc
pub fn softether_malloc(size: UINT) -> Option<std::ptr::NonNull<c_void>> {
    unsafe {
        let ptr = Malloc(size);
        std::ptr::NonNull::new(ptr)
    }
}

/// Safe wrapper for SoftEther zero malloc
pub fn softether_zero_malloc(size: UINT) -> Option<std::ptr::NonNull<c_void>> {
    unsafe {
        let ptr = ZeroMalloc(size);
        std::ptr::NonNull::new(ptr)
    }
}

/// Safe wrapper for SoftEther free
pub fn softether_free(ptr: *mut c_void) {
    unsafe {
        Free(ptr);
    }
}

/// Convert Rust string to C string pointer
pub fn to_c_string(s: &str) -> Result<std::ffi::CString, std::ffi::NulError> {
    std::ffi::CString::new(s)
}

/// Convert Rust string to wide char array (UTF-16)
pub fn to_wide_string(s: &str, buffer: &mut [u16]) -> Result<(), Box<dyn std::error::Error>> {
    let wide_chars: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();

    if wide_chars.len() > buffer.len() {
        return Err("String too long for buffer".into());
    }

    buffer[..wide_chars.len()].copy_from_slice(&wide_chars);
    Ok(())
}

/// Log levels for SoftEther
pub mod log_level {
    pub const ERROR: i32 = 0;
    pub const WARNING: i32 = 1;
    pub const INFO: i32 = 2;
    pub const DEBUG: i32 = 3;
}

/// Error codes (based on common VPN error patterns)
pub mod error_codes {
    pub const SUCCESS: i32 = 0;
    pub const CONNECTION_TIMEOUT: i32 = 1;
    pub const AUTH_FAILED: i32 = 2;
    pub const NETWORK_ERROR: i32 = 3;
    pub const INVALID_PARAMETER: i32 = 4;
    pub const ALREADY_CONNECTED: i32 = 5;
    pub const NOT_CONNECTED: i32 = 6;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_access() {
        // Test that we can safely call version functions
        // (Note: This will return None if library isn't loaded)
        let _version = get_version();
        let _error = get_last_error();
    }

    #[test]
    fn test_string_conversion() {
        let test_str = "test string";
        let c_string = to_c_string(test_str).unwrap();
        assert!(!c_string.as_ptr().is_null());
    }
}
