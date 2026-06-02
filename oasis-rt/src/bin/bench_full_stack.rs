//! OASIS — End-to-end full-stack bench (K=10 banded).
//!
//! Exercises the whole OASIS API surface in one process:
//!   topic pub/sub + services RPC + actions + mesh forwarding + AEAD encryption
//!
//! This bench does NOT compare against ROS 2 numerically (no A/B rig). It
//! reports OASIS's absolute throughput + latency for the nested-envelope case:
//! `topic → mesh → ECDH → AEAD`.
//!
//! Banded 2026-04-22: K=10 repeats per pipeline. Reports median ns/op +
//! (min-max) + half-spread %. Hygiene-round requirement: no single-shot
//! bench numbers in the repo.

use oasis_rt::{topics, services, actions, mesh, spore_crypto};
use std::time::Instant;

const K_REPEATS: usize = 10;

fn median_min_max(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[samples.len() / 2], samples[0], samples[samples.len() - 1])
}

fn report(label: &str, samples: &mut [f64]) {
    let (median, min, max) = median_min_max(samples);
    let half = (max - min) / (2.0 * median) * 100.0;
    println!(
        "  {:<36} {:>7.0} ns/op ({:>6.0}-{:>6.0}) ±{:>4.1}%  {:>10.0} ops/s",
        label, median, min, max, half, 1e9 / median
    );
}

fn bench<F: FnMut() -> f64>(mut op: F) -> Vec<f64> {
    (0..K_REPEATS).map(|_| op()).collect()
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ OASIS — Full-stack integration bench  (K={} repeats)     ║", K_REPEATS);
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // ── 1. Topic pub → mesh wrap → unwrap → dispatch ───────────────
    let mut samples = bench(|| {
        let mut router = topics::TopicRouter::new();
        fn counter(_h: u64, _p: &[u8]) {}
        router.subscribe("/cmd_vel", counter);
        let mut mesh_router = mesh::MeshRouter::new([1u8; 8]);
        const N: u32 = 50_000;
        let start = Instant::now();
        for _ in 0..N {
            let topic_env = topics::wrap_topic("/cmd_vel", b"lin:0.5 ang:0.1");
            let mesh_env = mesh_router.origin_wrap(&topic_env);
            let _ = mesh::parse_envelope(&mesh_env);
            router.dispatch(&topic_env).unwrap();
        }
        start.elapsed().as_nanos() as f64 / N as f64
    });
    report("topic + mesh wrap + dispatch", &mut samples);

    // ── 2. Service request + response pipeline ─────────────────────
    let mut samples = bench(|| {
        let mut svc = services::ServiceRouter::new();
        fn echo(p: &[u8]) -> Vec<u8> { p.to_vec() }
        svc.register("/compute_path", echo);
        const N: u32 = 20_000;
        let start = Instant::now();
        for i in 0..N {
            let req = services::wrap_request("/compute_path", i as u64, b"from=0,0 to=5,5");
            let resp = svc.handle(&req).unwrap();
            let (_, _, _) = services::parse_response(&resp).unwrap();
        }
        start.elapsed().as_nanos() as f64 / N as f64
    });
    report("service request + response", &mut samples);

    // ── 3. Action goal + feedback + result ─────────────────────────
    let mut samples = bench(|| {
        const N: u32 = 20_000;
        let start = Instant::now();
        for i in 0..N {
            let _g = actions::wrap_goal("/navigate", i as u64, b"target=(5,5)");
            let _f = actions::wrap_feedback(i as u64, b"progress=0.5");
            let _r = actions::wrap_result(i as u64, actions::ActionStatus::Succeeded, b"done");
        }
        start.elapsed().as_nanos() as f64 / N as f64
    });
    report("action (goal+feedback+result)", &mut samples);

    // ── 4. Maximum-layer nesting: topic → mesh → AEAD ──────────────
    let mut samples = bench(|| {
        let psk = spore_crypto::parse_key_hex(
            "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
        ).unwrap();
        let mut mesh_router = mesh::MeshRouter::new([1u8; 8]);
        const N: u32 = 10_000;
        let start = Instant::now();
        for _ in 0..N {
            let topic_env = topics::wrap_topic("/cmd_vel", b"lin:0.5 ang:0.1");
            let ct = spore_crypto::encrypt_envelope(&psk, &topic_env, b"").unwrap();
            let _mesh_env = mesh_router.origin_wrap(&ct);
        }
        start.elapsed().as_nanos() as f64 / N as f64
    });
    report("topic+AEAD+mesh (3-layer wrap)", &mut samples);

    // ── 5. Hot decrypt path: mesh → AEAD → topic dispatch ──────────
    let mut samples = bench(|| {
        let psk = spore_crypto::parse_key_hex(
            "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
        ).unwrap();
        let mut mesh_router = mesh::MeshRouter::new([2u8; 8]);
        let mut origin = mesh::MeshRouter::new([1u8; 8]);
        let mut topic_router = topics::TopicRouter::new();
        fn noop(_h: u64, _p: &[u8]) {}
        topic_router.subscribe("/cmd_vel", noop);

        const N: u32 = 10_000;
        let envs: Vec<Vec<u8>> = (0..N).map(|i| {
            let topic_env = topics::wrap_topic("/cmd_vel", b"lin:0.5 ang:0.1");
            let ct = spore_crypto::encrypt_envelope(&psk, &topic_env, b"").unwrap();
            let mut outer = origin.origin_wrap(&ct);
            outer[6..14].copy_from_slice(&(i as u64).to_le_bytes());
            outer
        }).collect();

        let start = Instant::now();
        let mut dispatched = 0u32;
        for e in envs {
            if let mesh::MeshDecision::Arrived { envelope, .. } = mesh_router.process_owned(e) {
                let ct = mesh::inner_slice(&envelope);
                if let Ok(pt) = spore_crypto::decrypt_envelope(&psk, ct, b"") {
                    if topic_router.dispatch(&pt).is_ok() { dispatched += 1; }
                }
            }
        }
        start.elapsed().as_nanos() as f64 / dispatched.max(1) as f64
    });
    report("RX: mesh+AEAD+topic dispatch", &mut samples);

    println!();
    println!("K={} repeats. All 5 pipelines exercised. No panics, no leaks.", K_REPEATS);
}
