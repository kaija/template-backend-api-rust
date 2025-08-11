use std::process::Command;

fn main() {
    // Set build timestamp
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", chrono::Utc::now().to_rfc3339());
    
    // Set Rust version
    if let Ok(output) = Command::new("rustc").arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("cargo:rustc-env=RUSTC_VERSION={}", version.trim());
    } else {
        println!("cargo:rustc-env=RUSTC_VERSION=unknown");
    }
    
    // Set target triple
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string()));
}