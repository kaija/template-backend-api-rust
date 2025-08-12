use std::env;
use std::process::Command;

fn main() {
    // Set RUSTC_VERSION if not already set
    if env::var("RUSTC_VERSION").is_err() {
        let output = Command::new("rustc")
            .arg("--version")
            .output()
            .expect("Failed to execute rustc --version");
        
        let version_string = String::from_utf8(output.stdout)
            .expect("Failed to parse rustc version output");
        
        let version = version_string
            .split_whitespace()
            .nth(1)
            .unwrap_or("unknown")
            .to_string();
        
        println!("cargo:rustc-env=RUSTC_VERSION={}", version);
    }

    // Set TARGET if not already set
    if env::var("TARGET").is_err() {
        let target = env::var("TARGET").unwrap_or_else(|_| {
            let output = Command::new("rustc")
                .args(&["-vV"])
                .output()
                .expect("Failed to execute rustc -vV");
            
            let output_string = String::from_utf8(output.stdout)
                .expect("Failed to parse rustc -vV output");
            
            for line in output_string.lines() {
                if line.starts_with("host: ") {
                    return line.replace("host: ", "");
                }
            }
            "unknown".to_string()
        });
        
        println!("cargo:rustc-env=TARGET={}", target);
    }

    // Set BUILD_TIMESTAMP if not already set
    if env::var("BUILD_TIMESTAMP").is_err() {
        let timestamp = chrono::Utc::now().to_rfc3339();
        println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    }

    // Tell Cargo to rerun this build script if any of these change
    println!("cargo:rerun-if-env-changed=RUSTC_VERSION");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=BUILD_TIMESTAMP");
}