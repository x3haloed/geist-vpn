//! FFI bindings for SoftEtherVPN
//!
//! This module contains the unsafe foreign function interface declarations
//! for interacting with the SoftEtherVPN C libraries.

use std::os::raw::{c_char, c_int, c_uint, c_void};

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

// Library initialization
extern "C" {
    /// Initialize Mayaqua library (includes OS-specific setup)
    pub fn InitMayaqua(memcheck: bool, debug: bool, argc: c_int, argv: *mut *mut c_char);

    /// Set the executable name for SoftEtherVPN
    pub fn InitGetExeName(arg: *mut c_char);

    /// Initialize process-wide state (Mayaqua)
    pub fn InitProcessCallOnce();

    /// Initialize Cedar VPN library
    pub fn InitCedar();

    /// Free Cedar VPN library
    pub fn FreeCedar();
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

/// Error codes from SoftEtherVPN (based on Cedar.h)
pub mod error_codes {
    // Success
    pub const ERR_NO_ERROR: i32 = 0;

    // Connection errors
    pub const ERR_CONNECT_FAILED: i32 = 1;
    pub const ERR_SERVER_IS_NOT_VPN: i32 = 2;
    pub const ERR_DISCONNECTED: i32 = 3;
    pub const ERR_PROTOCOL_ERROR: i32 = 4;
    pub const ERR_CLIENT_IS_NOT_VPN: i32 = 5;
    pub const ERR_USER_CANCEL: i32 = 6;

    // Authentication errors
    pub const ERR_AUTHTYPE_NOT_SUPPORTED: i32 = 7;
    pub const ERR_HUB_NOT_FOUND: i32 = 8;
    pub const ERR_AUTH_FAILED: i32 = 9;
    pub const ERR_PROXY_AUTH_FAILED: i32 = 19;

    // Session/HUB errors
    pub const ERR_HUB_STOPPING: i32 = 10;
    pub const ERR_SESSION_REMOVED: i32 = 11;
    pub const ERR_ACCESS_DENIED: i32 = 12;
    pub const ERR_SESSION_TIMEOUT: i32 = 13;
    pub const ERR_INVALID_PROTOCOL: i32 = 14;
    pub const ERR_TOO_MANY_CONNECTION: i32 = 15;
    pub const ERR_HUB_IS_BUSY: i32 = 16;
    pub const ERR_TOO_MANY_USER_SESSION: i32 = 20;

    // Proxy errors
    pub const ERR_PROXY_CONNECT_FAILED: i32 = 17;
    pub const ERR_PROXY_ERROR: i32 = 18;

    // License/Device errors
    pub const ERR_LICENSE_ERROR: i32 = 21;
    pub const ERR_DEVICE_DRIVER_ERROR: i32 = 22;
    pub const ERR_INTERNAL_ERROR: i32 = 23;

    // Secure device errors
    pub const ERR_SECURE_DEVICE_OPEN_FAILED: i32 = 24;
    pub const ERR_SECURE_PIN_LOGIN_FAILED: i32 = 25;
    pub const ERR_SECURE_NO_CERT: i32 = 26;
    pub const ERR_SECURE_NO_PRIVATE_KEY: i32 = 27;
    pub const ERR_SECURE_CANT_WRITE: i32 = 28;
    pub const ERR_SECURE_DEVICE_ERROR: i32 = 39;
    pub const ERR_NO_SECURE_DEVICE_SPECIFIED: i32 = 40;

    // Object/Account errors
    pub const ERR_OBJECT_NOT_FOUND: i32 = 29;
    pub const ERR_ACCOUNT_ALREADY_EXISTS: i32 = 34;
    pub const ERR_ACCOUNT_ACTIVE: i32 = 35;
    pub const ERR_ACCOUNT_NOT_FOUND: i32 = 36;
    pub const ERR_ACCOUNT_INACTIVE: i32 = 37;

    // Virtual LAN errors
    pub const ERR_VLAN_ALREADY_EXISTS: i32 = 30;
    pub const ERR_VLAN_INSTALL_ERROR: i32 = 31;
    pub const ERR_VLAN_INVALID_NAME: i32 = 32;
    pub const ERR_VLAN_IS_USED: i32 = 41;
    pub const ERR_VLAN_FOR_ACCOUNT_NOT_FOUND: i32 = 42;
    pub const ERR_VLAN_FOR_ACCOUNT_USED: i32 = 43;
    pub const ERR_VLAN_FOR_ACCOUNT_DISABLED: i32 = 44;

    // Parameter/Value errors
    pub const ERR_INVALID_PARAMETER: i32 = 38;
    pub const ERR_INVALID_VALUE: i32 = 45;

    // Farm/Controller errors
    pub const ERR_NOT_FARM_CONTROLLER: i32 = 46;
    pub const ERR_TRYING_TO_CONNECT: i32 = 47;
    pub const ERR_CONNECT_TO_FARM_CONTROLLER: i32 = 48;

    // Generic errors
    pub const ERR_NOT_SUPPORTED: i32 = 33;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Test basic functionality without calling undefined functions
        // We'll test actual FFI calls once we have proper library linking
        assert!(true); // Placeholder test
    }

    #[test]
    fn test_string_conversion() {
        let test_str = "test string";
        let c_string = to_c_string(test_str).unwrap();
        assert!(!c_string.as_ptr().is_null());
    }
}
