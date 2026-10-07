//! Enrollment attestations, ownership offers and acceptances, persisted identity
//! records, sender-lease records.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::enrollment::parse_attestation;
use oasis_rt::identity::decode_identity;
use oasis_rt::ownership::{parse_accept, parse_offer};
use oasis_rt::tx_lease::{decode_record, TX_LEASE_RECORD_LEN};

fuzz_target!(|data: &[u8]| {
    common::track("enrollment", data);
    let _ = parse_attestation(data);
    let _ = parse_offer(data);
    let _ = parse_accept(data);
    let _ = decode_identity(data);
    if let Ok(r) = <&[u8; TX_LEASE_RECORD_LEN]>::try_from(data) {
        let _ = decode_record(r);
    }
});
