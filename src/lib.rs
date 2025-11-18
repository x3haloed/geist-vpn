//! # Geist VPN - Modern Rust GUI for SoftEtherVPN
//!
//! This crate provides a safe, modern interface to SoftEtherVPN's client functionality
//! through FFI bindings, wrapped in a cross-platform GUI built with Tauri.

pub mod client;
pub mod profile;
pub mod error;
pub mod bindings;
pub mod memory;

pub use client::SoftEtherClient;
pub use profile::{VpnProfile, ProfileManager};
pub use error::{Result, Error};

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
        // Initialize SoftEtherVPN internals
        // This will call the appropriate FFI functions
        unsafe {
            // Initialize process-wide state
            bindings::InitProcessCallOnce();
            // Initialize Cedar VPN library
            bindings::InitCedar();
            // Start the SoftEther client service
            bindings::CtStartClient();
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
