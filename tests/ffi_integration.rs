//! Integration tests for SoftEtherVPN FFI bindings
//!
//! These tests validate that our FFI bindings work correctly with the actual
//! SoftEtherVPN library. They require SoftEtherVPN to be compiled and linked.

use geist_vpn::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test basic client creation and destruction
#[tokio::test]
#[ignore] // Requires SoftEtherVPN library to be linked
async fn test_client_lifecycle() {
    // Initialize the library
    init().expect("Failed to initialize SoftEtherVPN library");

    // Test client creation
    let client_result = SoftEtherClient::new();
    assert!(client_result.is_ok(), "Failed to create SoftEther client");

    let client = client_result.unwrap();

    // Test that client is initially disconnected
    assert!(!client.is_connected());
    assert_eq!(client.get_status(), client::ConnectionStatus::Disconnected);

    // Test cleanup
    cleanup().expect("Failed to cleanup SoftEtherVPN library");
}

/// Test memory management with SoftEther allocators
#[test]
fn test_memory_allocation() {
    // For integration tests, we need to actually initialize the library
    unsafe {
        geist_vpn::bindings::InitProcessCallOnce();
        geist_vpn::bindings::InitCedar();
        geist_vpn::bindings::CtStartClient();
    }

    // Test raw memory allocation
    let mem = memory::malloc_raw(128).expect("Failed to allocate memory");
    assert_eq!(mem.size(), 128);
    assert!(!mem.as_ptr().is_null());

    // Memory should be automatically freed when mem goes out of scope

    // Clean up
    unsafe {
        geist_vpn::SoftEtherClient::global_cleanup().expect("Failed to cleanup client");
        geist_vpn::bindings::FreeCedar();
    }
}

/// Test string encoding/decoding
#[test]
fn test_string_encoding() {
    let test_string = "Test VPN Server Connection";
    let account_name = "test_user";

    // Test conversion to SoftEther wide string format
    let wide_account = memory::strings::rust_to_softether_wide(account_name)
        .expect("Failed to convert account name");

    // Test conversion back
    let back_to_rust = memory::strings::softether_wide_to_rust(&wide_account);
    assert_eq!(account_name, back_to_rust);

    // Test with longer string
    let wide_server = memory::strings::rust_to_softether_wide(test_string)
        .expect("Failed to convert server name");

    let back_server = memory::strings::softether_wide_to_rust(&wide_server);
    assert_eq!(test_string, back_server);
}

/// Test profile creation and validation
#[test]
fn test_profile_operations() {
    // Create a test profile
    let profile = VpnProfile::new(
        "Test VPN".into(),
        "test.vpn.server.com".into(),
        443,
        profile::VpnProtocol::SslVpn,
    );

    // Test validation
    assert!(profile.validate().is_ok());

    // Test profile serialization (this would be used for saving/loading)
    let yaml = serde_yaml::to_string(&profile).expect("Failed to serialize profile");
    assert!(yaml.contains("Test VPN"));
    assert!(yaml.contains("test.vpn.server.com"));
}

/// Test error handling and conversion
#[test]
fn test_error_handling() {
    // Test various error types
    let conn_error = Error::ConnectionFailed {
        message: "Network timeout".into(),
    };
    assert!(conn_error.to_string().contains("Network timeout"));

    let mem_error = Error::MemoryError {
        message: "Allocation failed".into(),
    };
    assert!(mem_error.to_string().contains("Allocation failed"));

    let encoding_error = Error::EncodingError {
        message: "Invalid UTF-8".into(),
    };
    assert!(encoding_error.to_string().contains("Invalid UTF-8"));
}

/// Test the async client wrapper (without actual connection)
#[tokio::test]
#[ignore] // Requires SoftEtherVPN library to be linked
async fn test_async_client_wrapper() {
    init().expect("Failed to initialize library");

    let client = SoftEtherClient::new().expect("Failed to create client");

    // Test status subscription
    let mut status_rx = client.subscribe_status();

    // Client should be disconnected initially
    assert!(!client.is_connected());
    assert_eq!(client.get_status(), client::ConnectionStatus::Disconnected);

    // Test that we can receive status updates (should be empty initially)
    // Note: In a real scenario, this would receive connection status updates
    let _status_result = status_rx.try_recv(); // Should not panic

    cleanup().expect("Failed to cleanup");
}

/// Test profile manager operations
#[tokio::test]
#[ignore] // Requires SoftEtherVPN library to be linked
async fn test_profile_manager() {
    // Create profile manager (uses temp directory for tests)
    let manager = ProfileManager::new().expect("Failed to create profile manager");

    // Create and save a test profile
    let profile = VpnProfile::new(
        "Integration Test VPN".into(),
        "integration.test.com".into(),
        4443,
        profile::VpnProtocol::SslVpn,
    );

    manager.save_profile(&profile).expect("Failed to save profile");

    // Load and verify the profile
    let loaded = manager.get_profile(&profile.id).expect("Failed to load profile");
    assert_eq!(loaded.name, profile.name);
    assert_eq!(loaded.host, profile.host);
    assert_eq!(loaded.port, profile.port);

    // List profiles
    let profiles = manager.load_profiles().expect("Failed to list profiles");
    assert!(!profiles.is_empty());

    // Clean up
    manager.delete_profile(&profile.id).expect("Failed to delete profile");
}

/// Integration test that attempts basic SoftEther operations
/// Note: This test will only pass if SoftEtherVPN is properly compiled and linked
#[tokio::test]
#[ignore] // Ignored by default since it requires SoftEtherVPN to be compiled
async fn test_softether_basic_operations() {
    println!("Testing basic SoftEtherVPN operations...");

    // Initialize library
    init().expect("SoftEtherVPN initialization failed");

    // Create client
    let client = SoftEtherClient::new().expect("Client creation failed");

    // Test basic client operations (these would fail gracefully if library isn't linked)
    assert!(!client.is_connected());

    // Clean up
    SoftEtherClient::global_cleanup().expect("Global cleanup failed");
    cleanup().expect("Library cleanup failed");

    println!("Basic SoftEtherVPN operations test completed");
}

/// Comprehensive connection test (mock - doesn't connect to real server)
#[tokio::test]
#[ignore] // Requires SoftEtherVPN library to be linked
async fn test_connection_workflow() {
    init().expect("Library initialization failed");

    let mut client = SoftEtherClient::new().expect("Client creation failed");

    // Create a mock profile for testing
    let profile = VpnProfile::new(
        "Mock VPN Server".into(),
        "mock.vpn.test".into(),
        443,
        profile::VpnProtocol::SslVpn,
    );

    // In a real test, this would attempt connection but fail due to no server
    // For now, we just validate the profile and client setup
    assert!(profile.validate().is_ok());
    assert!(!client.is_connected());

    // Test disconnecting when not connected (should not error)
    let disconnect_result = client.disconnect().await;
    assert!(disconnect_result.is_ok());

    cleanup().expect("Cleanup failed");
}
