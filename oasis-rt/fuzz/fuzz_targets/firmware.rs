//! Firmware manifest (OAU1 kind 3 content) and image version header. An accepted
//! manifest re-encodes to the same bytes.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::firmware::{encode_manifest, image_version, parse_manifest};

fuzz_target!(|data: &[u8]| {
    common::track("firmware", data);
    if let Some(m) = parse_manifest(data) {
        assert_eq!(&encode_manifest(&m)[..], data);
    }
    let _ = image_version(data);
});
