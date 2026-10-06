use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Make memory.x available to the linker (copied into OUT_DIR, which is on
    // the linker search path).
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    // Phase 1.3: an application under oasis-bootloader links at ACTIVE (0x10008000).
    // The chosen map is written to OUT_DIR/memory.x; no memory.x may sit in the crate
    // root, or the linker would find that one first (it did, before the rename).
    let memory: &[u8] = if env::var_os("CARGO_FEATURE_BOOTLOADED").is_some() {
        include_bytes!("memory_bootloaded.x")
    } else {
        include_bytes!("memory_standalone.x")
    };
    File::create(out.join("memory.x")).unwrap().write_all(memory).unwrap();
    println!("cargo:rerun-if-changed=memory_bootloaded.x");
    // Firmware version for the update gate (Phase 1.3): OASIS_FW_VERSION, default 1.
    println!("cargo:rerun-if-env-changed=OASIS_FW_VERSION");
    let ver: u32 = env::var("OASIS_FW_VERSION").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    File::create(out.join("fwinfo.rs"))
        .unwrap()
        .write_all(format!("pub const FW_VERSION: u32 = {};\n", ver).as_bytes())
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory_standalone.x");
    println!("cargo:rerun-if-changed=build.rs");

    // Board identity, baked in at build time -> becomes the USB-CDC serial
    // string, so each flashed board enumerates with a stable, distinct id
    // (works around the clone boards' duplicate hardware flash serial).
    println!("cargo:rerun-if-env-changed=OASIS_BOARD_ID");
    let board = env::var("OASIS_BOARD_ID").unwrap_or_else(|_| "X".to_string());
    println!("cargo:rustc-env=OASIS_BOARD_ID={board}");

    // Re-run when HEAD (or the branch ref it points to) moves, so the stamped
    // git hash always matches the code that was built — not a cached value from
    // an earlier commit at the same OASIS_BOARD_ID.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    if let Ok(head) = std::fs::read_to_string("../.git/HEAD") {
        if let Some(r) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../.git/{}", r.trim());
        }
    }

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
