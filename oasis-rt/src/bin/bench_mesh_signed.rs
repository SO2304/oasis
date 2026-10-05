//! OASIS — mesh v8 vs v9 vs v0A throughput bench.
//!
//! Measures the per-op cost added by:
//!   v9  — HMAC-SHA256-8 (external-attacker resistance, shared MAC key)
//!   v0A — Ed25519 per-node signature (insider-resistance, per-node key)
//!
//! Each measurement is repeated K=10 times. Reports median ns/op plus
//! (min-max) range and half-spread %. This lets callers distinguish
//! a real speedup from single-run Windows-scheduler noise.
//!
//! Caveat: single-laptop, release mode, unloaded. No A/B vs ROS 2 rig.

use oasis_rt::mesh::{mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshMacKey, MeshPubRegistry, MeshRouter, SPORE_V10_MAGIC, SPORE_V8_MAGIC, SPORE_V9_MAGIC};
use oasis_rt::{spore_crypto, topics};
use std::time::Instant;

const K_REPEATS: usize = 10;
const N_WRAP: u32 = 200_000;
const N_PROC: u32 = 50_000;
const N_V10: u32 = 1_000; // v0A is ~1000× slower; use smaller N

fn median_min_max(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[samples.len() / 2], samples[0], samples[samples.len() - 1])
}

fn report(label: &str, samples: &mut [f64]) {
    let (median, min, max) = median_min_max(samples);
    let half = (max - min) / (2.0 * median) * 100.0;
    println!("  {:<48} {:>8.0} ns/op  ({:>8.0}-{:>8.0})  ±{:>4.1}%  {:>10.0} ops/s", label, median, min, max, half, 1e9 / median);
}

fn bench<F: FnMut() -> f64>(mut op: F) -> Vec<f64> {
    (0..K_REPEATS).map(|_| op()).collect()
}

fn mac_key() -> MeshMacKey {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = (i as u8).wrapping_mul(17);
    }
    MeshMacKey(k)
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ OASIS — v8/v9/v0A mesh bench  (K={} repeats, median ± spread)       ║", K_REPEATS);
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    println!("origin_wrap (16-byte payload, N={} per repeat):", N_WRAP);

    // v8 origin_wrap
    let mut samples = bench(|| {
        let mut r = MeshRouter::new([1u8; 8]);
        let start = Instant::now();
        for _ in 0..N_WRAP {
            let _e = r.origin_wrap(b"lin:0.5 ang:0.1");
        }
        start.elapsed().as_nanos() as f64 / N_WRAP as f64
    });
    report("v8 origin_wrap + build envelope", &mut samples);

    // v9 origin_wrap
    let key = mac_key();
    let mut samples = bench(|| {
        let mut r = MeshRouter::new_signed([1u8; 8], key.clone());
        let start = Instant::now();
        for _ in 0..N_WRAP {
            let _e = r.origin_wrap(b"lin:0.5 ang:0.1");
        }
        start.elapsed().as_nanos() as f64 / N_WRAP as f64
    });
    report("v9 origin_wrap + HMAC-SHA256-8 tag", &mut samples);

    // v0A origin_wrap — uses cached KeyPair since last round
    let seed = MeshEdSeed([7u8; 32]);
    let mut samples = bench(|| {
        let mut r = MeshRouter::new_ed25519_signed([1u8; 8], seed.clone(), MeshPubRegistry::new());
        let start = Instant::now();
        for _ in 0..N_V10 {
            let _e = r.origin_wrap(b"lin:0.5 ang:0.1");
        }
        start.elapsed().as_nanos() as f64 / N_V10 as f64
    });
    report("v0A origin_wrap + Ed25519 sign (cached kp)", &mut samples);

    println!();
    println!("process() forwarding path, unique msg_ids (N={} per repeat):", N_PROC);

    // v8 process
    let mut samples = bench(|| {
        let mut origin = MeshRouter::new([1u8; 8]);
        let envs: Vec<Vec<u8>> = (0..N_PROC).map(|_| origin.origin_wrap(b"x")).collect();
        let mut hop = MeshRouter::new([2u8; 8]);
        assert_eq!(&envs[0][..6], SPORE_V8_MAGIC);
        let start = Instant::now();
        for e in &envs {
            let _ = hop.process(e);
        }
        start.elapsed().as_nanos() as f64 / N_PROC as f64
    });
    report("v8 process (verify-free, bloom insert)", &mut samples);

    // v9 process
    let mut samples = bench(|| {
        let key = mac_key();
        let mut origin = MeshRouter::new_signed([1u8; 8], key.clone());
        let envs: Vec<Vec<u8>> = (0..N_PROC).map(|_| origin.origin_wrap(b"x")).collect();
        let mut hop = MeshRouter::new_signed([2u8; 8], key);
        assert_eq!(&envs[0][..6], SPORE_V9_MAGIC);
        let start = Instant::now();
        for e in &envs {
            let _ = hop.process(e);
        }
        start.elapsed().as_nanos() as f64 / N_PROC as f64
    });
    report("v9 process (HMAC verify + bloom insert)", &mut samples);

    // v0A process
    let mut samples = bench(|| {
        let origin_fp = [1u8; 8];
        let hop_fp = [2u8; 8];
        let seed_o = MeshEdSeed([7u8; 32]);
        let seed_h = MeshEdSeed([9u8; 32]);
        let pk_o = mesh_v10_pubkey_from_seed(&seed_o).unwrap();
        let pk_h = mesh_v10_pubkey_from_seed(&seed_h).unwrap();
        let mut origin_reg = MeshPubRegistry::new();
        origin_reg.insert(hop_fp, pk_h);
        let mut hop_reg = MeshPubRegistry::new();
        hop_reg.insert(origin_fp, pk_o);
        let mut origin = MeshRouter::new_ed25519_signed(origin_fp, seed_o, origin_reg);
        let mut hop = MeshRouter::new_ed25519_signed(hop_fp, seed_h, hop_reg);
        let envs: Vec<Vec<u8>> = (0..N_V10).map(|_| origin.origin_wrap(b"x")).collect();
        assert_eq!(&envs[0][..6], SPORE_V10_MAGIC);
        let start = Instant::now();
        for e in &envs {
            let _ = hop.process(e);
        }
        start.elapsed().as_nanos() as f64 / N_V10 as f64
    });
    report("v0A process (Ed25519 verify + bloom insert)", &mut samples);

    println!();
    println!("process() rejection paths (N={} per repeat):", N_PROC);

    // v8 duplicate drop
    let mut samples = bench(|| {
        let mut origin = MeshRouter::new([1u8; 8]);
        let env = origin.origin_wrap(b"x");
        let mut hop = MeshRouter::new([2u8; 8]);
        let _ = hop.process(&env);
        let start = Instant::now();
        for _ in 0..N_PROC {
            let _ = hop.process(&env);
        }
        start.elapsed().as_nanos() as f64 / N_PROC as f64
    });
    report("v8 duplicate drop (exact HashSet hit)", &mut samples);

    // v9 bad-MAC drop
    let mut samples = bench(|| {
        let key = mac_key();
        let mut origin = MeshRouter::new_signed([1u8; 8], key.clone());
        let mut env = origin.origin_wrap(b"x");
        env[25] ^= 0x01;
        let mut hop = MeshRouter::new_signed([2u8; 8], key);
        let start = Instant::now();
        for _ in 0..N_PROC {
            let _ = hop.process(&env);
        }
        start.elapsed().as_nanos() as f64 / N_PROC as f64
    });
    report("v9 bad-MAC drop (full HMAC + rejection)", &mut samples);

    // Bloom FPR sweep unchanged (single run — not a perf claim, curve match).
    println!();
    println!("Bloom FPR sweep (probe size = 5 000 per n, fresh router each, K=1):");
    println!("  {:>8} {:>10} {:>10} {:>10}", "n", "observed", "theory", "delta");
    for &n in &[5_000u32, 13_000, 20_000, 40_000, 52_000, 80_000] {
        let mut origin = MeshRouter::new([1u8; 8]);
        let mut hop = MeshRouter::new([2u8; 8]);
        for _ in 0..n {
            let _ = hop.process(&origin.origin_wrap(b"x"));
        }
        let mut probe_origin = MeshRouter::new([9u8; 8]);
        let probes: Vec<Vec<u8>> = (0..5_000).map(|_| probe_origin.origin_wrap(b"x")).collect();
        let mut fp_count = 0u32;
        for e in &probes {
            if matches!(hop.process(e), MeshDecision::Drop("duplicate")) {
                fp_count += 1;
            }
        }
        let observed = fp_count as f64 / 5_000.0;
        let m = (oasis_rt::mesh::BLOOM_WORDS as f64) * 64.0;
        let theory = (1.0 - (-5.0 * n as f64 / m).exp()).powf(5.0);
        println!("  {:>8} {:>9.2}% {:>9.2}% {:>+9.2}%", n, observed * 100.0, theory * 100.0, (observed - theory) * 100.0);
    }

    let _ = spore_crypto::parse_key_hex;
    let _ = topics::wrap_topic;
    println!();
    println!("Done. K={} per measurement. Numbers include min-max + half-spread %.", K_REPEATS);
}
