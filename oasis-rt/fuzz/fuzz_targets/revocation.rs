//! ORV1 revocation lists, revocation bodies, OEP1 epoch beacons, persisted blob records.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::mesh_revocation::{decode_blob_record, parse_oep1, parse_orv1, parse_revocation_body};

fuzz_target!(|data: &[u8]| {
    common::track("revocation", data);
    let _ = parse_orv1(data);
    let _ = parse_revocation_body(*b"OASISnet", data);
    let _ = parse_oep1(data);
    let _ = decode_blob_record(data);
});
