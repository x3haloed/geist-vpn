//! Basic usage example for Geist VPN FFI bindings
//!
//! This example demonstrates how to use the SoftEtherVPN FFI bindings
//! to create a client, manage profiles, and perform basic operations.
//!
//! Run with: cargo run --example basic_usage

use geist_vpn::*;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 Geist VPN - Basic FFI Binding Test");
    println!("=====================================");

    // Initialize the SoftEtherVPN library
    println!("📚 Initializing SoftEtherVPN library...");
    init()?;

    // Create a VPN profile
    println!("📝 Creating VPN profile...");
    let profile = VpnProfile::new(
        "Demo VPN Server".into(),
        "demo.vpn.example.com".into(),
        443,
        profile::VpnProtocol::SslVpn,
    );

    println!(
        "Profile created: {} -> {}:{}",
        profile.name, profile.host, profile.port
    );

    // Validate the profile
    println!("✅ Validating profile...");
    profile.validate()?;
    println!("Profile validation passed!");

    // Test profile serialization
    println!("💾 Testing profile serialization...");
    let yaml = serde_yaml::to_string(&profile)?;
    println!(
        "Profile YAML (first 200 chars): {}",
        &yaml[..yaml.len().min(200)]
    );

    // Create a SoftEther client
    println!("🔧 Creating SoftEther client...");
    let client = SoftEtherClient::new()?;
    println!("Client created successfully!");

    // Test client status
    println!("📊 Client status: {:?}", client.get_status());
    println!("Connected: {}", client.is_connected());

    // Test memory management
    println!("🧠 Testing memory management...");
    let test_mem = memory::malloc_raw(256)?;
    println!("Allocated {} bytes of memory", test_mem.size());

    // Test string conversion
    println!("🔤 Testing string conversion...");
    let test_string = "Test VPN Connection String";
    let wide_string = memory::strings::rust_to_softether_wide(test_string)?;
    let back_to_rust = memory::strings::softether_wide_to_rust(&wide_string);
    assert_eq!(test_string, back_to_rust);
    println!(
        "String conversion successful: '{}' ↔ wide string",
        test_string
    );

    // Test profile manager
    println!("📁 Testing profile manager...");
    let profile_manager = ProfileManager::new()?;
    profile_manager.save_profile(&profile)?;
    println!("Profile saved successfully");

    let loaded_profiles = profile_manager.load_profiles()?;
    println!("Loaded {} profiles", loaded_profiles.len());

    // Clean up profile
    profile_manager.delete_profile(&profile.id)?;
    println!("Profile cleaned up");

    // Test status monitoring
    println!("📡 Testing status monitoring...");
    let mut status_rx = client.subscribe_status();

    // Note: In a real application, you would attempt a connection here
    // For this demo, we just test the monitoring infrastructure

    println!("🎯 Attempting mock connection operations...");

    // Test disconnect (should work even when not connected)
    match client.disconnect().await {
        Ok(_) => println!("✅ Disconnect operation completed"),
        Err(e) => println!("⚠️  Disconnect returned error (expected): {}", e),
    }

    // Final status check
    println!("📊 Final client status: {:?}", client.get_status());

    // Global cleanup
    println!("🧹 Performing global cleanup...");
    SoftEtherClient::global_cleanup()?;
    cleanup()?;

    println!("");
    println!("🎉 All FFI binding tests completed successfully!");
    println!("");
    println!("Next steps:");
    println!("1. Run 'cargo build --release' to compile with SoftEtherVPN");
    println!("2. Test actual VPN connections with 'cargo tauri dev'");
    println!("3. Run integration tests with 'cargo test --test ffi_integration'");

    Ok(())
}
