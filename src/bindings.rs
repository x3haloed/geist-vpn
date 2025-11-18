//! FFI bindings for SoftEtherVPN
//!
//! This module contains the unsafe foreign function interface declarations
//! for interacting with the SoftEtherVPN C libraries.

use std::os::raw::{c_char, c_int, c_void};

// Client management functions
extern "C" {
    /// Create a new VPN client instance
    pub fn CiNewClient() -> *mut c_void;

    /// Free a VPN client instance
    pub fn CiFreeClient(client: *mut c_void);

    /// Start the VPN client service
    pub fn CtStartClient() -> c_int;

    /// Stop the VPN client service
    pub fn CtStopClient() -> c_int;

    /// Connect to a VPN server
    pub fn CtConnect(
        client: *mut c_void,
        connect_req: *mut c_void,
    ) -> c_int;

    /// Disconnect from VPN server
    pub fn CtDisconnect(client: *mut c_void) -> c_int;

    /// Get connection status
    pub fn CtGetStatus(client: *mut c_void) -> c_int;

    /// Enumerate available accounts
    pub fn CtEnumAccount(
        client: *mut c_void,
        accounts: *mut c_void,
    ) -> c_int;
}

// Account management functions
extern "C" {
    /// Create a new account
    pub fn CtCreateAccount(
        client: *mut c_void,
        account_req: *mut c_void,
    ) -> c_int;

    /// Delete an account
    pub fn CtDeleteAccount(
        client: *mut c_void,
        account_name: *const c_char,
    ) -> c_int;

    /// Set account password
    pub fn CtSetPassword(
        client: *mut c_void,
        account_name: *const c_char,
        password: *const c_char,
    ) -> c_int;
}

// Library initialization functions
extern "C" {
    /// Initialize SoftEtherVPN library
    pub fn init_softether_library() -> c_int;

    /// Cleanup SoftEtherVPN library
    pub fn cleanup_softether_library() -> c_int;

    /// Get library version
    pub fn GetSoftEtherVersion() -> *const c_char;
}

// Error handling
extern "C" {
    /// Get last error message
    pub fn GetLastError() -> *const c_char;

    /// Get last error code
    pub fn GetLastErrorCode() -> c_int;
}

// Network utility functions
extern "C" {
    /// Test network connectivity
    pub fn CtTestConnection(
        hostname: *const c_char,
        port: c_int,
        timeout: c_int,
    ) -> c_int;
}

// Memory management (SoftEther custom allocators)
extern "C" {
    /// SoftEther malloc
    pub fn Malloc(size: usize) -> *mut c_void;

    /// SoftEther free
    pub fn Free(ptr: *mut c_void);

    /// SoftEther realloc
    pub fn ReAlloc(ptr: *mut c_void, size: usize) -> *mut c_void;
}

// String utilities
extern "C" {
    /// Copy string (SoftEther style)
    pub fn CopyStr(dst: *mut c_char, src: *const c_char) -> *mut c_char;

    /// Copy unicode string
    pub fn CopyUniStr(dst: *mut u16, src: *const u16) -> *mut u16;

    /// Free string
    pub fn FreeStr(str: *mut c_char);
}

// Thread management
extern "C" {
    /// Sleep for specified milliseconds
    pub fn SleepThread(millis: c_int);

    /// Get current thread ID
    pub fn ThreadId() -> u64;
}

// Logging functions
extern "C" {
    /// Write to log
    pub fn WriteLog(
        level: c_int,
        tag: *const c_char,
        message: *const c_char,
    );

    /// Set log level
    pub fn SetLogLevel(level: c_int);
}

/// Safe wrapper for getting version string
pub fn get_version() -> Option<String> {
    unsafe {
        let ptr = GetSoftEtherVersion();
        if ptr.is_null() {
            None
        } else {
            let c_str = std::ffi::CStr::from_ptr(ptr);
            Some(c_str.to_string_lossy().into_owned())
        }
    }
}

/// Safe wrapper for getting last error
pub fn get_last_error() -> Option<String> {
    unsafe {
        let ptr = GetLastError();
        if ptr.is_null() {
            None
        } else {
            let c_str = std::ffi::CStr::from_ptr(ptr);
            Some(c_str.to_string_lossy().into_owned())
        }
    }
}

/// Safe wrapper for SoftEther malloc
pub fn softether_malloc(size: usize) -> Option<std::ptr::NonNull<c_void>> {
    unsafe {
        let ptr = Malloc(size);
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
pub fn to_c_string(s: &str) -> Result<CString, std::ffi::NulError> {
    std::ffi::CString::new(s)
}

/// Log levels for SoftEther
pub mod log_level {
    pub const ERROR: i32 = 0;
    pub const WARNING: i32 = 1;
    pub const INFO: i32 = 2;
    pub const DEBUG: i32 = 3;
}

/// Error codes
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
