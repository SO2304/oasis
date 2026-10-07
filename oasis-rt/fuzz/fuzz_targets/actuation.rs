//! OAC1 actuation commands and OTM1 time beacons. An accepted command re-encodes to the
//! same bytes.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::actuation::{encode_oac1, encode_otm1, parse_oac1, parse_otm1};

fuzz_target!(|data: &[u8]| {
    common::track("actuation", data);
    if let Some(c) = parse_oac1(data) {
        assert_eq!(&encode_oac1(&c)[..], data);
    }
    if let Some((boot_id, now_ms)) = parse_otm1(data) {
        assert_eq!(&encode_otm1(boot_id, now_ms)[..], data);
    }
});
