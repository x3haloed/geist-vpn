/// Standalone test program to debug SoftEtherVPN FFI issues
/// This runs outside the test framework to eliminate test environment issues

fn main() {
    println!("SoftEtherVPN FFI Standalone Test");
    println!("=================================");

    unsafe {
        println!("Step 1: InitMayaqua (includes OS-specific setup)");
        geist_vpn::bindings::InitMayaqua(false, false, 0, std::ptr::null_mut());
        println!("✓ InitMayaqua succeeded");

        println!("Step 2: InitCedar");
        geist_vpn::bindings::InitCedar();
        println!("✓ InitCedar succeeded");

        println!("Step 3: Testing CtStartClient");
        println!("WARNING: This may crash with SIGSEGV...");
        geist_vpn::bindings::CtStartClient();
        println!("✓ CtStartClient succeeded! (If you see this, the issue is test-environment specific)");

        println!("Step 4: CtStopClient");
        geist_vpn::bindings::CtStopClient();
        println!("✓ CtStopClient succeeded");

        println!("Step 5: FreeCedar");
        geist_vpn::bindings::FreeCedar();
        println!("✓ FreeCedar succeeded");
    }

    println!("\nTest completed successfully!");
}
