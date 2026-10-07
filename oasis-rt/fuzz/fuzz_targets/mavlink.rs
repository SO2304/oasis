//! MAVLink v2 frames (with and without the replay state).
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::mavlink_min::{parse_frame, parse_frame_checked, ReplayState};

fuzz_target!(|data: &[u8]| {
    common::track("mavlink", data);
    let _ = parse_frame(data);
    let mut st = ReplayState::new();
    let _ = parse_frame_checked(data, &mut st);
});
