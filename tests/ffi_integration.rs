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

/// Test basic library loading
#[test]
fn test_library_loading() {
    println!("Testing basic library loading...");

    // Just try to call InitProcessCallOnce - this should be the minimal initialization
    unsafe {
        println!("Calling InitProcessCallOnce...");
        geist_vpn::bindings::InitProcessCallOnce();
        println!("InitProcessCallOnce succeeded");
    }
}

/// Test cedar initialization
#[test]
fn test_cedar_init() {
    println!("Testing cedar initialization...");

    unsafe {
        println!("Calling InitProcessCallOnce...");
        geist_vpn::bindings::InitProcessCallOnce();
        println!("InitProcessCallOnce succeeded");

        println!("Calling InitCedar...");
        geist_vpn::bindings::InitCedar();
        println!("InitCedar succeeded");

        println!("Calling FreeCedar...");
        geist_vpn::bindings::FreeCedar();
        println!("FreeCedar succeeded");
    }
}

/// Test client service initialization following vpnclient pattern
/// TODO: This currently crashes in CtStartClient - needs further investigation
#[test]
#[ignore = "CtStartClient causes segmentation fault - needs debugging"]
fn test_client_start() {
    println!("Testing client service initialization following vpnclient pattern...");

    unsafe {
        // This mimics the vpnclient service initialization order
        println!("=== Process Initialization (main) ===");
        println!("Calling InitProcessCallOnce...");
        geist_vpn::bindings::InitProcessCallOnce();
        println!("InitProcessCallOnce succeeded");

        println!("\n=== Service Start (StartProcess) ===");
        println!("Calling InitCedar...");
        geist_vpn::bindings::InitCedar();
        println!("InitCedar succeeded");

        println!("Calling CtStartClient (this starts threads)...");
        geist_vpn::bindings::CtStartClient();
        println!("CtStartClient succeeded");

        // Give threads time to initialize
        std::thread::sleep(std::time::Duration::from_millis(100));

        println!("\n=== Service Stop (StopProcess) ===");
        println!("Calling CtStopClient...");
        geist_vpn::bindings::CtStopClient();
        println!("CtStopClient succeeded");

        println!("Calling FreeCedar...");
        geist_vpn::bindings::FreeCedar();
        println!("FreeCedar succeeded");
    }
}

/// Test step-by-step initialization to isolate crash location
#[test]
fn test_step_by_step_initialization() {
    println!("Testing step-by-step initialization...");

    unsafe {
        println!("Step 1: InitMayaqua (includes OS-specific setup)");
        geist_vpn::bindings::InitMayaqua(false, false, 0, std::ptr::null_mut());
        println!("✓ InitMayaqua succeeded");

        println!("Step 2: InitCedar");
        geist_vpn::bindings::InitCedar();
        println!("✓ InitCedar succeeded");

        // Test if we can access basic functions before client creation
        println!("Step 3: Testing basic function availability");
        // Just check if the library is loaded by trying a simple function
        println!("✓ Library appears to be loaded");

        println!("Step 4: Attempting CtStartClient (this may crash)");
        geist_vpn::bindings::CtStartClient();
        println!("✓ CtStartClient succeeded - this should not print if it crashes");

        println!("Step 5: CtStopClient");
        geist_vpn::bindings::CtStopClient();
        println!("✓ CtStopClient succeeded");

        println!("Step 6: FreeCedar");
        geist_vpn::bindings::FreeCedar();
        println!("✓ FreeCedar succeeded");
    }
}

/// Test if the issue is with direct CiNewClient calls
/// Based on code analysis, CiNewClient might be internal-only
#[test]
#[ignore = "CiNewClient appears to be internal function - causes segfault"]
fn test_direct_cinewclient_call() {
    println!("Testing direct CiNewClient call (this may crash)...");

    unsafe {
        println!("InitMayaqua...");
        geist_vpn::bindings::InitMayaqua(false, false, 0, std::ptr::null_mut());
        println!("InitCedar...");
        geist_vpn::bindings::InitCedar();

        println!("CiNewClient (this crashes)...");
        let client_ptr = geist_vpn::bindings::CiNewClient();
        println!("CiNewClient returned: {:?}", client_ptr);

        if !client_ptr.is_null() {
            println!("CtReleaseClient...");
            geist_vpn::bindings::CtReleaseClient(client_ptr);
        }

        println!("FreeCedar...");
        geist_vpn::bindings::FreeCedar();
    }
}

/// Test memory management with SoftEther allocators
#[test]
fn test_memory_allocation() {
    println!("Testing memory allocation...");

    // For integration tests, we need to actually initialize the library
    unsafe {
        println!("Initializing Mayaqua...");
        geist_vpn::bindings::InitMayaqua(false, false, 0, std::ptr::null_mut());
        println!("Initializing cedar...");
        geist_vpn::bindings::InitCedar();
        println!("Starting client...");
        geist_vpn::bindings::CtStartClient();
    }

    // Test raw memory allocation
    println!("Testing memory allocation...");
    let mem = memory::malloc_raw(128).expect("Failed to allocate memory");
    assert_eq!(mem.size(), 128);
    assert!(!mem.as_ptr().is_null());
    println!("Memory allocation succeeded");

    // Memory should be automatically freed when mem goes out of scope

    // Clean up
    unsafe {
        println!("Cleaning up client...");
        geist_vpn::SoftEtherClient::global_cleanup().expect("Failed to cleanup client");
        println!("Cleaning up cedar...");
        geist_vpn::bindings::FreeCedar();
    }
    println!("Cleanup completed");
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
    // Create a test profile with valid credentials
    let mut profile = VpnProfile::new(
        "Test VPN".into(),
        "test.vpn.server.com".into(),
        443,
        profile::VpnProtocol::SslVpn,
    );

    // Set valid credentials for validation
    if let profile::AuthMethod::Password { username, password } = &mut profile.auth {
        *username = "testuser".into();
        *password = "testpass".into();
    }
    profile.account_name = "testaccount".into();

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
