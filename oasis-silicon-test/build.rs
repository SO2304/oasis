use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Make memory.x available to the linker (copied into OUT_DIR, which is on
    // the linker search path).
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");

    // Board identity, baked in at build time -> becomes the USB-CDC serial
    // string, so each flashed board enumerates with a stable, distinct id
    // (works around the clone boards' duplicate hardware flash serial).
    println!("cargo:rerun-if-env-changed=OASIS_BOARD_ID");
    let board = env::var("OASIS_BOARD_ID").unwrap_or_else(|_| "X".to_string());
    println!("cargo:rustc-env=OASIS_BOARD_ID={board}");

    // Short git hash for provenance (falls back to "unknown" off a repo).
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=OASIS_GIT_HASH={hash}");
}
