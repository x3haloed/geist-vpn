//! # Geist VPN - Modern Rust GUI for SoftEtherVPN
//!
//! This crate provides a safe, modern interface to SoftEtherVPN's client functionality
//! through FFI bindings, wrapped in a cross-platform GUI built with Iced.
//!
//! ## Features
//!
//! - Native Rust GUI using Iced
//! - SoftEther VPN client integration
//! - Profile management
//! - Connection status monitoring
//! - Cross-platform support (macOS, Windows, Linux)

pub mod bindings;
pub mod cert_prompt;
pub mod client;
pub mod error;
pub mod hub;
pub mod memory;
pub mod profile;

pub use client::SoftEtherClient;
pub use error::{Error, Result};
pub use profile::{ProfileManager, VpnProfile};

// Re-export commonly used types
pub use bindings::*;

// Constants
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the SoftEtherVPN library
///
/// This must be called before using any VPN functionality.
/// It sets up the necessary internal state and allocates resources.
pub fn init() -> Result<()> {
    // Skip actual initialization during tests to avoid FFI issues
    #[cfg(not(test))]
    {
        tracing::info!("SoftEther init: Starting SoftEtherVPN library initialization");

        // Note: SoftEtherVPN expects executable detection to work properly

        // Initialize SoftEtherVPN internals
        // This will call the appropriate FFI functions
        unsafe {
            tracing::info!("SoftEther init: Calling InitMayaqua()");
            // Initialize Mayaqua first (includes OS-specific setup like locks, resource limits)
            // This matches what the official vpnclient does in UnixServiceMain
            bindings::InitMayaqua(false, false, 0, std::ptr::null_mut());
            tracing::info!("SoftEther init: InitMayaqua() completed");

            tracing::info!("SoftEther init: Calling InitGetExeName()");
            // Set the executable name (pass NULL to default to "./a.out")
            // InitGetExeName must be called after InitMayaqua according to SoftEtherVPN source
            bindings::InitGetExeName(std::ptr::null_mut());
            tracing::info!("SoftEther init: InitGetExeName() completed");

            tracing::info!("SoftEther init: Calling InitCedar()");
            // Initialize Cedar VPN library
            bindings::InitCedar();
            tracing::info!("SoftEther init: InitCedar() completed");

            tracing::info!("SoftEther init: Skipping CtStartClient() to avoid RPC server hang");
            // Instead of calling CtStartClient() which includes RPC server that hangs,
            // we'll manually initialize what we need:
            // - Client is created by CtStartClient -> we'll handle this in SoftEtherClient::new
            // - RPC server is started by CtStartClient -> we'll skip this entirely
            tracing::info!("SoftEther init: CtStartClient() skipped (RPC server disabled)");
        }

        tracing::info!("SoftEtherVPN library initialized");
    }

    #[cfg(test)]
    {
        tracing::info!("SoftEtherVPN library initialization skipped during tests");
    }

    Ok(())
}

/// Cleanup SoftEtherVPN resources
///
/// Call this when shutting down the application to free resources.
pub fn cleanup() -> Result<()> {
    // Skip actual cleanup during tests
    #[cfg(not(test))]
    {
        // Stop the SoftEther client service
        SoftEtherClient::global_cleanup()?;
        unsafe {
            // Cleanup Cedar VPN library
            bindings::FreeCedar();
        }
        tracing::info!("SoftEtherVPN library cleaned up");
    }

    #[cfg(test)]
    {
        tracing::info!("SoftEtherVPN library cleanup skipped during tests");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
