//! `lora_budget` — time on air and duty-cycle budget for OASIS message sizes.
//!
//! Replaces `docs/lora_budget.py`, deleted on 2026-10-09 (rule 7: Rust only). The script
//! was a **second implementation** of `airtime::airtime_us` in another language, and
//! nothing compared the two while six documents quoted its numbers. The formula now has
//! one home, and the figures the documents cite are `cargo test` assertions in
//! `budget::tests` rather than assertions in a script nobody runs.
//!
//! ```text
//! cd oasis-lora-transport && cargo run --release --bin lora_budget
//! ```
//!
//! ⚠️ The time-on-air formula is **not verified at the source** — the Semtech datasheet is
//! behind a commercial portal. What is verified is that this code reproduces the figures
//! published in the documents that cite it (SF7/16 B = 51.456 ms, SF12/64 B = 2 793.5 ms).
//! Exact with respect to that formula, not to a measured radio, and **no radio has ever
//! transmitted**.

use oasis_lora_transport::budget::{
    frag_cost, lorawan_cap, msgs_per_hour, toa_us, DUTY_US_PER_HOUR_1PCT, LORAWAN_OVERHEAD,
};

/// Message sizes as read out of the silicon logs, on the wire.
const MSGS: &[(&str, usize)] = &[
    ("compact stop OAS1 (part J)", 99 + 11),
    ("supervision beacon OSB1", 131),
    ("Modbus order OMB1", 132),
    ("actuation order OAC1", 153),
    ("enrolment attestation OAU1 (Ed25519)", 221),
    ("revocation ORV1, 1 node", 234),
    ("revocation ORV1, 2 nodes", 242),
    (
        "revocation ORV1, 16 nodes [extrapolated +8/node]",
        234 + 15 * 8,
    ),
];

fn main() {
    println!("lora_budget — EU868, BW 125 kHz, CR 4/5, 8-symbol preamble, CRC on, explicit header");
    println!("duty cycle: {} s of air time per hour per BAND (1 %, Tobs = 1 h, ETSI EN 300 220-2 §4.4.3.2)", DUTY_US_PER_HOUR_1PCT / 1_000_000);
    println!();

    // Self-check first, so a wrong formula cannot print a table. Same two figures the
    // tests assert; printed because a reader of the output should see them too.
    let a = toa_us(7, 16);
    let b = toa_us(12, 64);
    println!(
        "self-check  SF7/16 B = {}.{:03} ms (published 51.456)",
        a / 1000,
        a % 1000
    );
    println!(
        "self-check  SF12/64 B = {}.{:03} ms (published 2793.5)",
        b / 1000,
        b % 1000
    );
    assert_eq!(
        a, 51_456,
        "the formula no longer reproduces the published SF7 figure"
    );
    assert_eq!(
        b, 2_793_472,
        "the formula no longer reproduces the published SF12 figure"
    );
    println!();

    println!("=== raw LoRa: 255-byte PHY payload, no application cap ===");
    println!("| message | bytes | SF7 | SF9 | SF10 | SF12 |");
    println!("|---|---:|---:|---:|---:|---:|");
    for (name, n) in MSGS {
        let mut cells = String::new();
        for sf in [7u8, 9, 10, 12] {
            let t = toa_us(sf, *n);
            cells.push_str(&format!(
                " {}.{:03} ms / {}/h |",
                t / 1000,
                t % 1000,
                msgs_per_hour(sf, *n)
            ));
        }
        println!("| {name} | {n} |{cells}");
    }

    println!();
    println!("=== LoRaWAN class A: application payload cap, RP002-1.0.3 tables 12/13 ===");
    print!("cap:");
    for sf in 7u8..=12 {
        print!(" SF{sf}={} B", lorawan_cap(sf));
    }
    println!();
    for (name, n) in MSGS {
        let fits: Vec<String> = (7u8..=12)
            .filter(|sf| *n <= lorawan_cap(*sf) as usize)
            .map(|sf| format!("SF{sf}"))
            .collect();
        if fits.is_empty() {
            println!("  {name} ({n} B): over the cap at every SF — does not fit on LoRaWAN at all");
        } else {
            println!("  {name} ({n} B): fits only at {}", fits.join(", "));
        }
    }
    println!();
    println!("The v0B header alone is 99 B and the cap is 51 B at SF10-SF12, so **no OASIS");
    println!("message fits at long range on LoRaWAN class A**. Raw LoRa is the usable column.");

    println!();
    println!("=== fragmenting a post-quantum payload (CRYPTO_MIGRATION.md §2, §6-d) ===");
    println!("charged against the same hourly band budget; {LORAWAN_OVERHEAD} B of frame overhead per fragment");
    // FIPS 204 Table 2 (ML-DSA-44), FIPS 203 Table 3 (ML-KEM-768).
    let cases: &[(&str, usize)] = &[
        ("ML-DSA-44 signature alone", 2420),
        ("ML-KEM-768 ciphertext, once per peer", 1088),
        ("ML-KEM-768 encapsulation key", 1184),
        ("OAQ1 k=2, keys carried", 51 + 1 + 2 * (1312 + 2420)),
        ("OAQ1 k=2, one-byte signer indices", 51 + 1 + 2 * (1 + 2420)),
    ];
    for (name, n) in cases {
        let (f12, a12, h12) = frag_cost(*n, 12);
        let (f7, a7, h7) = frag_cost(*n, 7);
        println!(
            "{name:38} {n:5} B | SF12 {f12:3} frags {}.{:01} s = {}.{:02} h of budget | SF7 {f7:3} frags {}.{:01} s = {}.{:02} h",
            a12 / 1_000_000,
            (a12 % 1_000_000) / 100_000,
            h12 / 100,
            h12 % 100,
            a7 / 1_000_000,
            (a7 % 1_000_000) / 100_000,
            h7 / 100,
            h7 % 100
        );
    }
}
