//! OASIS — mesh v0A vs v0B cost bench (security, not speed).
//!
//! v0B adds, over v0A's Ed25519 header signature:
//!   - a SHA-256 over the payload (so the content is signed), and
//!   - a domain tag + network_id + counter in the signed preimage.
//!
//! The dominant cost stays Ed25519; v0B's extra is one SHA-256 of the payload,
//! which is why v0B sign/verify grows slightly with payload size while v0A is
//! flat. This bench REPORTS that cost — it does not optimise it.
//!
//! K=10 repeats; median ns/op + (min-max) + half-spread %. Single laptop,
//! release, unloaded — not an A/B rig.

use oasis_rt::mesh::{mesh_v0b_verify, mesh_v10_pubkey_from_seed, mesh_v10_verify, MeshEdSeed, MeshPubRegistry, MeshRouter, MESH_ED_SIG_LEN};
use std::time::Instant;

const K_REPEATS: usize = 10;
const N_SIGN: u32 = 2_000;
const N_VERIFY: u32 = 2_000;

fn median_min_max(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[samples.len() / 2], samples[0], samples[samples.len() - 1])
}

fn report(label: &str, samples: &mut [f64]) {
    let (median, min, max) = median_min_max(samples);
    let half = (max - min) / (2.0 * median) * 100.0;
    println!("  {:<44} {:>9.0} ns/op  ({:>9.0}-{:>9.0})  ±{:>4.1}%  {:>10.0} ops/s", label, median, min, max, half, 1e9 / median);
}

fn bench<F: FnMut() -> f64>(mut op: F) -> Vec<f64> {
    (0..K_REPEATS).map(|_| op()).collect()
}

const NET: [u8; 8] = [0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8];

fn run_for_payload(payload: &[u8]) {
    let seed = MeshEdSeed([7u8; 32]);
    let fp = [1u8; 8];
    let pubkey = mesh_v10_pubkey_from_seed(&seed).unwrap();

    println!("payload = {} bytes:", payload.len());

    // --- sign / emit (origin_wrap) ---
    let mut s = bench(|| {
        let mut r = MeshRouter::new_ed25519_signed(fp, seed.clone(), MeshPubRegistry::new());
        let start = Instant::now();
        for _ in 0..N_SIGN {
            let _e = r.origin_wrap(payload); // v0A: Ed25519 over header only
        }
        start.elapsed().as_nanos() as f64 / N_SIGN as f64
    });
    report("v0A origin_wrap (Ed25519, header only)", &mut s);

    let mut s = bench(|| {
        let mut r = MeshRouter::new_v0b(fp, NET, seed.clone(), MeshPubRegistry::new());
        let start = Instant::now();
        for _ in 0..N_SIGN {
            let _e = r.origin_wrap_v0b(payload); // v0B: SHA-256(payload) + Ed25519
        }
        start.elapsed().as_nanos() as f64 / N_SIGN as f64
    });
    report("v0B origin_wrap (SHA-256 + Ed25519)", &mut s);

    // --- verify (raw verify fn, no dedup state) ---
    // Build one v0A and one v0B envelope, extract fields, verify in a loop.
    let mut r_a = MeshRouter::new_ed25519_signed(fp, seed.clone(), MeshPubRegistry::new());
    let ea = r_a.origin_wrap(payload);
    let msg_id_a = u64::from_le_bytes(ea[6..14].try_into().unwrap());
    let fp_a: [u8; 8] = ea[14..22].try_into().unwrap();
    let sig_a: [u8; MESH_ED_SIG_LEN] = ea[25..89].try_into().unwrap();

    let mut r_b = MeshRouter::new_v0b(fp, NET, seed.clone(), MeshPubRegistry::new());
    let eb = r_b.origin_wrap_v0b(payload).unwrap();
    let net_b: [u8; 8] = eb[6..14].try_into().unwrap();
    let fp_b: [u8; 8] = eb[14..22].try_into().unwrap();
    let counter_b = u64::from_le_bytes(eb[22..30].try_into().unwrap());
    let sig_b: [u8; MESH_ED_SIG_LEN] = eb[35..99].try_into().unwrap();
    let payload_b = &eb[99..];

    let mut s = bench(|| {
        let start = Instant::now();
        for _ in 0..N_VERIFY {
            let ok = mesh_v10_verify(&pubkey, msg_id_a, fp_a, &sig_a);
            assert!(ok);
        }
        start.elapsed().as_nanos() as f64 / N_VERIFY as f64
    });
    report("v0A verify (Ed25519, header only)", &mut s);

    let mut s = bench(|| {
        let start = Instant::now();
        for _ in 0..N_VERIFY {
            let ok = mesh_v0b_verify(&pubkey, net_b, fp_b, counter_b, payload_b, &sig_b);
            assert!(ok);
        }
        start.elapsed().as_nanos() as f64 / N_VERIFY as f64
    });
    report("v0B verify (SHA-256 + Ed25519)", &mut s);
    println!();
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ OASIS — v0A vs v0B mesh cost  (K={} repeats, median ± spread)   ║", K_REPEATS);
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    let kb = vec![0x5Au8; 1024];
    run_for_payload(b"lin:0.5 ang:0.1"); // 15 bytes
    run_for_payload(&kb);
    println!("Note: v0B > v0A by one SHA-256 of the payload (grows with size);");
    println!("Ed25519 dominates and is payload-independent. Not optimised.");
    println!("K={} per measurement. Numbers include min-max + half-spread %.", K_REPEATS);
}
