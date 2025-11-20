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

    // Set rpath so the executable can find the dynamic libraries at runtime
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());

    // Copy hamcore.se2 to the output directory so SoftEtherVPN can find it
    let hamcore_src = dst.join("build").join("hamcore.se2");
    let hamcore_dst = PathBuf::from("target").join("debug").join("hamcore.se2");
    let hamcore_dst_root = PathBuf::from("hamcore.se2"); // Also copy to project root

    if hamcore_src.exists() {
        std::fs::copy(&hamcore_src, &hamcore_dst).unwrap_or_else(|e| {
            panic!("Failed to copy hamcore.se2 to target/debug: {}", e);
        });
        std::fs::copy(&hamcore_src, &hamcore_dst_root).unwrap_or_else(|e| {
            panic!("Failed to copy hamcore.se2 to project root: {}", e);
        });
    } else {
        panic!("hamcore.se2 not found at {}", hamcore_src.display());
    }

    // Create symlink a.out -> target/debug/geist-vpn for SoftEtherVPN executable detection
    let exe_path = PathBuf::from("target").join("debug").join("geist-vpn");
    let aout_link = PathBuf::from("a.out");
    if exe_path.exists() {
        // Remove existing symlink if it exists
        let _ = std::fs::remove_file(&aout_link);
        // Create new symlink (using relative path)
        #[cfg(unix)]
        std::os::unix::fs::symlink(&exe_path, &aout_link).unwrap_or_else(|e| {
            panic!(
                "Failed to create symlink a.out -> {}: {}",
                exe_path.display(),
                e
            );
        });
    }

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
