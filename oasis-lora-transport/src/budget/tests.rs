//! Every figure the deleted `docs/lora_budget.py` asserted, re-asserted here.
//!
//! The script's whole value was its assertions: they pinned numbers quoted in
//! `CLAUDE.md`, `docs/CRYPTO_MIGRATION.md`, `docs/compliance/PQC.md`,
//! `docs/AUTHORITY_HARDENING_SPEC.md` and `partners/POSITIONING_GAPS.md`. A script nobody
//! runs pins nothing, so they are tests now — and `cargo test` in this crate is run by
//! `tools/check_claims.sh --full`, because this crate is deliberately **not** a workspace
//! member (it unconditionally enables `oasis-rt/mesh_bloom_mcu`) and `--workspace`
//! therefore never reached it. Its 22 tests had been running nowhere at all.

use super::*;

/// The cross-check the script performed on itself, and the reason this module exists:
/// the Rust time-on-air and the Python one had to agree, and nothing compared them.
#[test]
fn reproduces_the_published_2793_5_ms_at_sf12_64_bytes() {
    // docs/compliance/PQC.md: PL = 64 at SF12 is 2793.5 ms.
    let t = toa_us(12, 64);
    assert_eq!(
        t, 2_793_472,
        "SF12/64 B is {t} µs, not the published 2 793.5 ms"
    );
    // Within 0.1 ms of the published figure, which is the tolerance the script used.
    assert!(
        t.abs_diff(2_793_500) < 100,
        "{t} µs is further than 0.1 ms from 2 793 500"
    );
}

/// The SF7 figure the crate already pinned, kept here so both ends of the range are in
/// one place.
#[test]
fn reproduces_the_published_51_456_ms_at_sf7_16_bytes() {
    assert_eq!(toa_us(7, 16), 51_456);
}

/// Low-data-rate optimisation turns on at SF11 for 125 kHz, which is the only place the
/// deleted script and this code could have disagreed: the script keyed it off `sf >= 11`
/// directly, this keys it off a symbol time ≥ 16 ms via `sx126x::ldro`. At 125 kHz the two
/// rules give the same answer, and that is worth an assertion rather than a paragraph.
#[test]
fn ldro_boundary_matches_the_sf_ge_11_rule_at_125_khz() {
    for sf in 7u8..=12 {
        let t_sym_us = (1u64 << sf) * 1_000_000 / 125_000;
        let script_rule = sf >= 11;
        let this_rule = t_sym_us >= 16_000;
        assert_eq!(
            script_rule, this_rule,
            "SF{sf}: t_sym {t_sym_us} µs disagrees with the sf>=11 rule"
        );
    }
}

/// The LoRaWAN cap is the point of the whole exercise: at long range nothing of ours fits.
#[test]
fn no_oasis_message_fits_on_lorawan_at_sf10_and_above() {
    const V0B_HEADER: usize = 99;
    for sf in 10u8..=12 {
        assert_eq!(lorawan_cap(sf), 51, "RP002-1.0.3 says 51 B at SF{sf}");
        assert!(
            V0B_HEADER > lorawan_cap(sf) as usize,
            "the v0B header alone must exceed the cap at SF{sf}"
        );
    }
    assert_eq!(lorawan_cap(9), 115);
    assert_eq!(lorawan_cap(7), 242);
    assert_eq!(lorawan_cap(8), 242);
}

/// The raw-LoRa SF12 envelope. Sizes are the ones read out of the silicon logs.
///
/// ⚠️ **This test found a wrong figure in `CLAUDE.md` and in section G**, which said the
/// SF12 envelope was "6 orders, **3** revocations, 7 beacons per hour per band". The order
/// and beacon figures are right; a 234-byte revocation costs 8 364.032 ms and so allows
/// **4** per hour, not 3. Three per hour would need a **251**-byte message.
///
/// How it survived: the deleted `docs/lora_budget.py` **never computed any of these**. Its
/// table applied the LoRaWAN application cap, which is 51 B at SF10–12, so every OASIS
/// message printed `**over cap**` and no per-hour figure at SF12 was ever produced by it.
/// The raw-LoRa envelope was attributed to the script and arrived from somewhere else.
#[test]
fn reproduces_the_sf12_hourly_envelope() {
    assert_eq!(msgs_per_hour(12, 153), 6, "actuation order OAC1, 153 B");
    assert_eq!(msgs_per_hour(12, 131), 7, "supervision beacon OSB1, 131 B");
    assert_eq!(msgs_per_hour(12, 132), 7, "Modbus order OMB1, 132 B");
    assert_eq!(
        msgs_per_hour(12, 221),
        4,
        "enrolment attestation OAU1, 221 B"
    );
    assert_eq!(
        msgs_per_hour(12, 234),
        4,
        "revocation ORV1 for one node, 234 B — NOT 3"
    );
    assert_eq!(msgs_per_hour(12, 242), 4, "revocation for two nodes, 242 B");
    assert_eq!(
        toa_us(12, 234),
        8_364_032,
        "the time on air the 4/h comes from"
    );
    assert_eq!(
        msgs_per_hour(12, 251),
        3,
        "3 per hour needs 251 B, which no message is"
    );

    // Part J: the compact stop is 99 + 11 on the wire, and buys two more stops an hour.
    assert_eq!(
        msgs_per_hour(12, 99 + 11),
        8,
        "compact stop OAS1, 110 B on the wire"
    );
    assert!(
        msgs_per_hour(12, 99 + 11) > msgs_per_hour(12, 153),
        "the compact stop must buy something"
    );
}

/// `docs/CRYPTO_MIGRATION.md` quotes four fragment costs. The script asserted them; if
/// they are wrong the migration plan is wrong about what a post-quantum payload costs.
#[test]
fn reproduces_the_crypto_migration_fragment_costs() {
    // FIPS 204 Table 2 (ML-DSA-44) and FIPS 203 Table 3 (ML-KEM-768).
    const MLDSA44_SIG: usize = 2420;
    const MLDSA44_PK: usize = 1312;
    const MLKEM768_CT: usize = 1088;

    let (frames, _, hours100) = frag_cost(MLKEM768_CT, 12);
    assert_eq!(frames, 22, "ML-KEM-768 ciphertext is 22 fragments at SF12");
    assert!(
        hours100.abs_diff(170) <= 5,
        "1.7 h of hourly budget, got {}.{}",
        hours100 / 100,
        hours100 % 100
    );

    // OAQ1 k=2 carrying both keys and both signatures.
    let both = 51 + 1 + 2 * (MLDSA44_PK + MLDSA44_SIG);
    let (frames, _, hours100) = frag_cost(both, 12);
    assert_eq!(frames, 148);
    assert!(
        hours100.abs_diff(1150) <= 5,
        "11.5 h, got {}.{}",
        hours100 / 100,
        hours100 % 100
    );

    // OAQ1 k=2 with one-byte signer indices instead of carried keys.
    let indices = 51 + 1 + 2 * (1 + MLDSA44_SIG);
    let (frames, _, hours100) = frag_cost(indices, 12);
    assert_eq!(frames, 96);
    assert!(
        hours100.abs_diff(740) <= 5,
        "7.4 h, got {}.{}",
        hours100 / 100,
        hours100 % 100
    );

    // PQC.md §4's own figure, recomputed as a cross-check of the method itself.
    let (frames, _, hours100) = frag_cost(MLDSA44_SIG, 12);
    assert_eq!(
        frames, 48,
        "PQC.md says 48 frames for one ML-DSA-44 signature"
    );
    assert!(
        hours100.abs_diff(372) <= 5,
        "PQC.md says 3.72 h, got {}.{}",
        hours100 / 100,
        hours100 % 100
    );
}

/// The classical `OAQ1` sizes the script asserted against `quorum::tests`. Re-derived from
/// the same arithmetic rather than imported, because this crate must stay `no_std` and
/// buildable without reaching into `oasis-rt`'s test-only items.
#[test]
fn oaq1_length_matches_the_quorum_module() {
    // quorum::oaq1_len — 51-byte body, one count byte, 96 bytes per inline signature.
    let oaq1_len = |n: usize| 51 + 1 + 96 * n;
    assert_eq!(oaq1_len(1), 148);
    assert_eq!(oaq1_len(2), 244);
    // 343 B with the v0B header against a 255-byte PHY limit: two frames, as CLAUDE.md
    // says, which is why a two-person rule costs two frames.
    assert!(
        99 + oaq1_len(2) > 255,
        "a k=2 order must not fit in one PHY frame"
    );
}

/// A message that does not fit its cap is zero per hour, not a division by zero, and an
/// unknown SF is zero rather than a panic — the budget is consulted from a `no_std` node.
#[test]
fn out_of_range_inputs_are_zero_not_a_panic() {
    assert_eq!(lorawan_cap(6), 0);
    assert_eq!(lorawan_cap(13), 0);
    assert_eq!(
        frag_cost(100, 6),
        (0, 0, 0),
        "an unknown SF has no cap, so no cost"
    );
    assert_eq!(frag_cost(0, 12), (0, 0, 0), "nothing to send costs nothing");
    // The duty cycle always allows at least one of the smallest message.
    assert!(msgs_per_hour(12, 1) >= 1);
}

/// Monotonicity, which no single figure would catch: more payload and more spreading can
/// never cost less.
#[test]
fn airtime_is_monotone_in_payload_and_in_sf() {
    for sf in 7u8..=12 {
        let mut prev = 0;
        for pl in [1usize, 16, 51, 64, 110, 153, 234, 255] {
            let t = toa_us(sf, pl);
            assert!(
                t >= prev,
                "SF{sf}: {pl} B costs {t} µs, less than the smaller payload's {prev}"
            );
            prev = t;
        }
    }
    for pl in [16usize, 110, 255] {
        let mut prev = 0;
        for sf in 7u8..=12 {
            let t = toa_us(sf, pl);
            assert!(
                t > prev,
                "SF{sf} at {pl} B costs {t} µs, not more than SF{}",
                sf - 1
            );
            prev = t;
        }
    }
}
