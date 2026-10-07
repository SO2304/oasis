//! Shared by every target: keep the current input and, on a panic, write it to
//! `fuzz/artifacts/<target>/crash-panic-<fnv64>` before aborting. On Windows a Rust
//! panic aborts with `__fastfail`, which libFuzzer cannot intercept, so without this
//! hook a failing input is lost (2026-10-07, first `actuation` run).
use std::cell::RefCell;
use std::sync::Once;

thread_local!(static CURRENT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) });
static HOOK: Once = Once::new();

fn fnv64(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x0100_0000_01b3))
}

pub fn track(target: &'static str, data: &[u8]) {
    HOOK.call_once(|| {
        std::panic::set_hook(Box::new(move |info| {
            let input = CURRENT.with(|c| c.borrow().clone());
            let dir = format!("fuzz/artifacts/{target}");
            let _ = std::fs::create_dir_all(&dir);
            let path = format!("{dir}/crash-panic-{:016x}", fnv64(&input));
            let _ = std::fs::write(&path, &input);
            eprintln!("panic: {info}\ninput ({} bytes) saved to {path}", input.len());
            std::process::abort();
        }));
    });
    CURRENT.with(|c| {
        let mut v = c.borrow_mut();
        v.clear();
        v.extend_from_slice(data);
    });
}
