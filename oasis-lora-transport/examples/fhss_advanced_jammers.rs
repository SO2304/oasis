//! Gap 2 round 2 — measures FHSS performance against sweep and
//! follower jammers. Produces the numbers for SHADOW_AUDIT_GAP2_FHSS.md
//! advanced-jammer section.

use std::sync::Arc;
use oasis_lora_transport::fhss::channelized::ChannelizedRadio;
use oasis_lora_transport::fhss::{FhssRadio, SweepJammer, FollowerJammer, Jammer};
use oasis_lora_transport::LoRaRadio;

fn trial(seed: [u8; 32], n_channels: u32,
         jammer: Option<Arc<dyn Jammer>>,
         packets: u32, packet_ms: u64) -> f64 {
    let (radio_a, radio_b, bus) = ChannelizedRadio::pair(n_channels);
    bus.set_packet_duration(packet_ms);
    if let Some(j) = jammer { bus.add_jammer(j); }

    let mut tx = FhssRadio::new(radio_a, seed, n_channels);
    let mut rx = FhssRadio::new(radio_b, seed, n_channels);

    let mut got = 0u32;
    for i in 0..packets {
        let _ = tx.tx_hopped(format!("{}", i).as_bytes()).unwrap();
        let mut buf = [0u8; 64];
        if rx.rx_hopped(&mut buf, 20).is_ok() { got += 1; }
    }
    got as f64 / packets as f64
}

fn banded<F: Fn(u8) -> Option<Arc<dyn Jammer>>>(label: &str, seed_base: u8,
    n_channels: u32, jammer_factory: F, packets: u32, packet_ms: u64)
{
    let mut r = Vec::new();
    for k in 0..10u8 {
        let seed = [seed_base.wrapping_add(k); 32];
        r.push(trial(seed, n_channels, jammer_factory(k), packets, packet_ms));
    }
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (mn, med, mx) = (r[0], r[5], r[9]);
    let mean: f64 = r.iter().sum::<f64>() / 10.0;
    let spread = (mx - mn) / 2.0;
    println!("{:62}  K=10  min={:.3} med={:.3} max={:.3}  mean={:.3}  ±{:.3}",
             label, mn, med, mx, mean, spread);
}

fn main() {
    println!("OASIS Gap 2 round 2 — sweep + follower jammers vs FHSS (8 ch, 80 pkts/trial)");
    println!("packet_duration_ms = 100 (approximates SX1262 SF8 BW125 125B payload)\n");

    println!("── SWEEP JAMMER ──");

    // Sweep period = 8 × packet_ms → one dwell per packet → 1/8 of packets jammed.
    banded("FHSS 8ch, sweep P=800 ms (one dwell per packet)",
           0xA1, 8, |_| Some(Arc::new(SweepJammer {
               n_channels: 8, period_ms: 800, start_offset_ms: 0,
           })), 80, 100);

    // Fast sweep — period < packet time means we straddle dwell boundaries.
    banded("FHSS 8ch, sweep P=80 ms (fast — sub-packet dwell)",
           0xA2, 8, |_| Some(Arc::new(SweepJammer {
               n_channels: 8, period_ms: 80, start_offset_ms: 0,
           })), 80, 100);

    // Slow sweep — period 10 × packet time → each channel jammed for
    // 10 consecutive packets. On those 10, our hops will hit the jammer
    // channel ~10/8 = 1-2 times.
    banded("FHSS 8ch, sweep P=8000 ms (slow — many packets per dwell)",
           0xA3, 8, |_| Some(Arc::new(SweepJammer {
               n_channels: 8, period_ms: 8000, start_offset_ms: 0,
           })), 80, 100);

    println!("\n── FOLLOWER JAMMER ──");

    // Follower faster than packet — jams all packets.
    banded("FHSS 8ch, follower latency=10 ms (very fast)",
           0xB1, 8, |_| Some(Arc::new(FollowerJammer {
               latency_ms: 10, catch_threshold_pct: 20,
           })), 80, 100);

    // Follower ~half packet time — catches the CRC tail.
    banded("FHSS 8ch, follower latency=50 ms (half packet)",
           0xB2, 8, |_| Some(Arc::new(FollowerJammer {
               latency_ms: 50, catch_threshold_pct: 20,
           })), 80, 100);

    // Follower ≈ packet duration — jams tail but under threshold.
    banded("FHSS 8ch, follower latency=85 ms (near-packet, below threshold)",
           0xB3, 8, |_| Some(Arc::new(FollowerJammer {
               latency_ms: 85, catch_threshold_pct: 20,
           })), 80, 100);

    // Follower slower than packet — never in time.
    banded("FHSS 8ch, follower latency=120 ms (slower than packet)",
           0xB4, 8, |_| Some(Arc::new(FollowerJammer {
               latency_ms: 120, catch_threshold_pct: 20,
           })), 80, 100);

    println!("\n── COMBINED: FOLLOWER + SHORTER PACKET BEATS IT ──");

    // Short packet (10 ms) vs follower latency 20 ms → follower always late.
    banded("FHSS 8ch, short-packet 10ms + follower lat=20ms",
           0xC1, 8, |_| Some(Arc::new(FollowerJammer {
               latency_ms: 20, catch_threshold_pct: 20,
           })), 80, 10);
}
