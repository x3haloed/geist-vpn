use std::env;
use std::path::PathBuf;

fn main() {
    // Get the SoftEtherVPN source directory
    let softether_path = PathBuf::from("SoftEtherVPN");

    // Ensure SoftEtherVPN submodule is available
    if !softether_path.exists() {
        panic!("SoftEtherVPN submodule not found. Run 'git submodule update --init --recursive'");
    }

    // Configure CMake for static library builds
    let mut cmake_config = cmake::Config::new(&softether_path);

    // Set build options for static library builds
    cmake_config
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("BUILD_ONLY_LIBRARIES", "ON") // Custom flag to build only libraries
        .profile("Release");

    // Build the project
    let dst = cmake_config.build();

    // Tell cargo where to find the built libraries
    let lib_dir = dst.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Link against the core SoftEtherVPN libraries (dynamic linking)
    println!("cargo:rustc-link-lib=dylib=cedar");
    println!("cargo:rustc-link-lib=dylib=mayaqua");

    // Additional system libraries that SoftEtherVPN depends on
    // Use pkg-config for OpenSSL if available, otherwise link manually
    if pkg_config::Config::new().probe("openssl").is_ok() {
        // pkg-config found OpenSSL
    } else {
        // Fallback: manually link OpenSSL libraries
        println!("cargo:rustc-link-lib=dylib=crypto");
        println!("cargo:rustc-link-lib=dylib=ssl");
    }

    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=m");

    // macOS specific libraries
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "macos" {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
    }

    // Windows specific libraries
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=iphlpapi");
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=gdi32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=crypt32");
        println!("cargo:rustc-link-lib=dylib=wininet");
    }

    // Rebuild if SoftEtherVPN source changes
    println!("cargo:rerun-if-changed=SoftEtherVPN/");

    // Also rebuild if our build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
