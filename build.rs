use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=mojo/aegis_simd.mojo");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("libaegis_simd.so");

    if let Ok(status) = Command::new("mojo")
        .args([
            "build",
            "--emit",
            "shared-lib",
            "mojo/aegis_simd.mojo",
            "-o",
        ])
        .arg(&dest_path)
        .status()
    {
        if status.success() {
            println!(
                "cargo:rustc-env=AEGIS_SIMD_SO_BUILT={}",
                dest_path.display()
            );
            let _ = std::fs::copy(&dest_path, "mojo/libaegis_simd.so");
        } else {
            println!("cargo:warning=mojo build exited with non-zero status");
        }
    } else {
        println!("cargo:warning=mojo binary not found on PATH, skipping libaegis_simd.so build");
    }
}
