//! Gap 2 — FHSS (Frequency Hopping Spread Spectrum) anti-narrowband-jam.
//!
//! Wraps any `LoRaRadio` with a per-packet frequency selector derived
//! from a shared Ed25519 seed (the same seed the mesh uses for v0A
//! signing — no new key distribution needed). A narrowband jammer that
//! sits on one fixed channel kills 1/N packets; OASIS survives at
//! (N-1)/N delivery before the existing repeat/FEC layer.
//!
//! The seed derivation: HKDF-style using SHA-256 over (seed || "oasis-fhss-v1" || counter).
//! Lightweight, deterministic, no PRNG state to serialize.
//!
//! Anti-jam model (honest bounds):
//!
//! - Against a **static narrowband jammer** (CW on one channel): delivery
//!   = (N-1)/N per packet, independent across packets.
//! - Against a **sweeping jammer** (covers all N channels in a cycle):
//!   performance depends on TX duty vs sweep rate; FHSS helps if TX
//!   dwell < sweep residency.
//! - Against a **wideband barrage jammer** (all N at once): FHSS does
//!   nothing. Need DSSS or physical separation.
//! - Against a **follower jammer** (listens then jams your channel in
//!   real time): FHSS helps only if hop interval < follower latency.
//!
//! This module implements the **static narrowband** model, which is
//! both the cheapest jammer to build AND the most common real-world
//! threat (unsophisticated adversary, commercial RF noise).

#![cfg(feature = "std")]

use crate::{LoRaError, LoRaParams, LoRaRadio};
use sha2::{Digest, Sha256};

/// Pseudo-random frequency selector using SHA-256 over seed + context.
/// Deterministic — both TX and RX derive the same channel for the same
/// counter without ever transmitting the seed.
pub fn hop_channel(seed: &[u8; 32], counter: u64, n_channels: u32) -> u32 {
    let mut h = Sha256::new();
    h.update(seed);
    h.update(b"oasis-fhss-v1");
    h.update(counter.to_le_bytes());
    let digest = h.finalize();
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&digest[..4]);
    u32::from_le_bytes(bytes) % n_channels
}

/// Multi-channel simulated radio. Extends the single-channel
/// `SimulatedLoRaRadio` with N virtual frequencies so a `JammerModel`
/// can target specific ones.
///
/// Not a PHY simulator — still byte-layer. The "channel" is a tag on
/// each packet, and the jammer drops by tag. A real SX1262 hopping
/// between 868.1 / 868.3 / 868.5 MHz sees exactly this behavior under
/// a CW jammer on one of those frequencies.
/// Abstract jammer model. Given a TX channel and TX start time (both
/// in virtual ms), decide whether the packet is corrupted. Each
/// jammer variant below captures a specific adversary class.
pub trait Jammer: Send + Sync + 'static {
    /// Returns true if the transmission on `channel` starting at
    /// `start_ms` (for `duration_ms`) is successfully jammed. The bus
    /// ORs over all registered jammers — any one can kill the packet.
    fn jams(&self, channel: u32, start_ms: u64, duration_ms: u64) -> bool;
}

/// Static narrowband jammer — bitmask of permanently-jammed channels.
/// Gap 2 round 1 adversary. Simulates a CW transmitter sitting on a
/// fixed frequency regardless of time.
pub struct StaticJammer { pub jammed_mask: u64 }
impl Jammer for StaticJammer {
    fn jams(&self, channel: u32, _start_ms: u64, _dur: u64) -> bool {
        self.jammed_mask & (1u64 << channel) != 0
    }
}

/// Sweep jammer — rotates through all N channels with period
/// `period_ms`. Each channel is jammed for `period_ms / n` ms per
/// cycle. Simulates a commodity sweep-generator jammer (low-cost EW
/// on tactical civilian bands).
pub struct SweepJammer {
    pub n_channels: u32,
    pub period_ms: u64,
    pub start_offset_ms: u64,
}
impl Jammer for SweepJammer {
    fn jams(&self, channel: u32, start_ms: u64, duration_ms: u64) -> bool {
        // The packet occupies [start_ms, start_ms+duration_ms]. Check
        // if the jammer sits on `channel` during any overlapping slice.
        // Approximation: sample at start + dur/2 (midpoint). Good enough
        // if sweep_dwell >= packet_duration; accurate for our typical
        // sweep=1s vs packet=100ms regime.
        let t = start_ms.wrapping_add(duration_ms / 2);
        let dwell = self.period_ms / self.n_channels as u64;
        let phase = (t.wrapping_sub(self.start_offset_ms)) % self.period_ms;
        let jammed_ch = (phase / dwell) as u32;
        channel == jammed_ch
    }
}

/// Follower jammer — observes the TX, detects the active channel, and
/// switches its own output to that channel after `latency_ms`. If the
/// latency is less than the packet duration, the jammer overlaps the
/// packet's tail and LoRa CRC fails at the receiver.
///
/// Honest simplification: we treat any overlap >= 20 % of packet
/// duration as a successful jam. Real LoRa packets with explicit-header
/// CRC on fail if roughly the last 15-20 % of the payload is corrupted.
pub struct FollowerJammer {
    pub latency_ms: u64,
    pub catch_threshold_pct: u32,
}
impl Jammer for FollowerJammer {
    fn jams(&self, _channel: u32, _start_ms: u64, duration_ms: u64) -> bool {
        // Follower detects the start of the packet, switches after
        // latency_ms. Overlap window = duration_ms - latency_ms. If this
        // window covers >= threshold % of packet, packet fails.
        if self.latency_ms >= duration_ms { return false; }
        let overlap_pct = ((duration_ms - self.latency_ms) * 100) / duration_ms;
        overlap_pct >= self.catch_threshold_pct as u64
    }
}

pub mod channelized {
    use std::sync::mpsc::{Receiver, Sender, RecvTimeoutError};
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use crate::{LoRaError, LoRaParams, LoRaRadio};
    use super::Jammer;

    /// Shared bus carrying (channel_index, bytes) pairs across the link.
    /// Supports multiple registered jammer models via `Jammer` trait;
    /// maintains a virtual clock that advances on each TX so time-based
    /// jammers (sweep, follower) get a coherent view of packet timing.
    #[derive(Clone)]
    pub struct ChannelBus {
        tx_ab: Sender<(u32, u64, Vec<u8>)>,   // (channel, start_ms, bytes)
        tx_ba: Sender<(u32, u64, Vec<u8>)>,
        rx_ab: Arc<Mutex<Receiver<(u32, u64, Vec<u8>)>>>,
        rx_ba: Arc<Mutex<Receiver<(u32, u64, Vec<u8>)>>>,
        pub n_channels: u32,
        /// Default packet duration used for time accounting. SX1262 at
        /// SF7/BW125/CR4-5/20B ≈ 60 ms; at SF9/BW125 ≈ 200 ms. Held in
        /// Arc<AtomicU64> so mutations via `set_packet_duration` on the
        /// bus are visible to the endpoint radios after clone.
        packet_duration_ms: Arc<AtomicU64>,
        clock_ms: Arc<AtomicU64>,
        /// Registered jammers; each TX is ORed against all of them.
        jammers: Arc<Mutex<Vec<Arc<dyn Jammer>>>>,
    }

    impl ChannelBus {
        pub fn new(n_channels: u32) -> Self {
            assert!(n_channels <= 64, "n_channels ≤ 64");
            let (tx_ab, rx_ab) = std::sync::mpsc::channel();
            let (tx_ba, rx_ba) = std::sync::mpsc::channel();
            Self {
                tx_ab, tx_ba,
                rx_ab: Arc::new(Mutex::new(rx_ab)),
                rx_ba: Arc::new(Mutex::new(rx_ba)),
                n_channels,
                packet_duration_ms: Arc::new(AtomicU64::new(100)),
                clock_ms: Arc::new(AtomicU64::new(0)),
                jammers: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn add_jammer(&self, j: Arc<dyn Jammer>) {
            self.jammers.lock().unwrap().push(j);
        }

        /// Override packet duration — shared across all clones of this bus.
        pub fn set_packet_duration(&self, ms: u64) {
            self.packet_duration_ms.store(ms, Ordering::SeqCst);
        }

        pub fn packet_duration(&self) -> u64 {
            self.packet_duration_ms.load(Ordering::SeqCst)
        }

        /// Convenience for the original Static API.
        pub fn set_jammed(&self, channel: u32, jammed: bool) {
            if !jammed { return; } // simplification — clearing static jammer unused
            self.add_jammer(Arc::new(super::StaticJammer {
                jammed_mask: 1u64 << channel,
            }));
        }

        fn advance_clock(&self) -> u64 {
            let d = self.packet_duration_ms.load(Ordering::SeqCst);
            self.clock_ms.fetch_add(d, Ordering::SeqCst)
        }

        fn is_jammed(&self, channel: u32, start_ms: u64) -> bool {
            let d = self.packet_duration_ms.load(Ordering::SeqCst);
            let guard = self.jammers.lock().unwrap();
            guard.iter().any(|j| j.jams(channel, start_ms, d))
        }
    }

    /// One endpoint of a channelized link. `role` = 0 for "A" (TX→B), 1 for "B".
    pub struct ChannelizedRadio {
        bus: ChannelBus,
        role: u8,
        current_channel: u32,
    }

    impl ChannelizedRadio {
        pub fn pair(n_channels: u32) -> (Self, Self, ChannelBus) {
            let bus = ChannelBus::new(n_channels);
            let a = Self { bus: bus.clone(), role: 0, current_channel: 0 };
            let b = Self { bus: bus.clone(), role: 1, current_channel: 0 };
            (a, b, bus)
        }

        /// Change the TX/RX channel for the next packet. In a real SX1262,
        /// this triggers a frequency re-tune (~1 ms on SX1262, documented).
        pub fn set_channel(&mut self, channel: u32) {
            assert!(channel < self.bus.n_channels, "channel out of range");
            self.current_channel = channel;
        }

        pub fn current_channel(&self) -> u32 { self.current_channel }
    }

    impl LoRaRadio for ChannelizedRadio {
        fn init(&mut self, _params: &LoRaParams) -> Result<(), LoRaError> { Ok(()) }

        fn tx_payload(&mut self, payload: &[u8]) -> Result<(), LoRaError> {
            let start_ms = self.bus.advance_clock();
            if self.bus.is_jammed(self.current_channel, start_ms) {
                return Ok(());
            }
            let out = match self.role {
                0 => &self.bus.tx_ab,
                _ => &self.bus.tx_ba,
            };
            out.send((self.current_channel, start_ms, payload.to_vec()))
                .map_err(|_| LoRaError::Driver("bus closed"))?;
            Ok(())
        }

        fn rx_payload(&mut self, buf: &mut [u8], timeout_ms: u32)
            -> Result<usize, LoRaError>
        {
            let rx = match self.role {
                0 => &self.bus.rx_ba,
                _ => &self.bus.rx_ab,
            };
            let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms as u64);
            loop {
                let now = std::time::Instant::now();
                if now >= deadline { return Err(LoRaError::Timeout); }
                let remaining = deadline - now;
                let rx_guard = rx.lock().unwrap();
                match rx_guard.recv_timeout(remaining) {
                    Ok((chan, _start_ms, bytes)) => {
                        if chan == self.current_channel {
                            if bytes.len() > buf.len() {
                                return Err(LoRaError::PayloadTooLarge(bytes.len()));
                            }
                            buf[..bytes.len()].copy_from_slice(&bytes);
                            return Ok(bytes.len());
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => return Err(LoRaError::Timeout),
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err(LoRaError::Driver("bus closed"));
                    }
                }
            }
        }

        fn max_payload(&self) -> usize { 255 }
    }
}

/// FHSS orchestrator — wraps any `LoRaRadio` that supports channel
/// switching and rotates the channel per packet using `hop_channel`.
/// Caller retains the seed; this type holds only a packet counter.
pub struct FhssRadio<R> {
    inner: R,
    seed: [u8; 32],
    n_channels: u32,
    packet_counter: u64,
}

impl<R> FhssRadio<R> {
    pub fn new(inner: R, seed: [u8; 32], n_channels: u32) -> Self {
        Self { inner, seed, n_channels, packet_counter: 0 }
    }

    pub fn counter(&self) -> u64 { self.packet_counter }

    /// The channel this radio WOULD use for the next packet. Useful
    /// for RX-side synchronization or logging.
    pub fn next_channel(&self) -> u32 {
        hop_channel(&self.seed, self.packet_counter, self.n_channels)
    }
}

/// Requires `ChannelizedRadio` specifically — we need the ability to
/// re-tune between packets. Real SX1262 also supports this via
/// `set_rf_frequency()` at standby.
impl FhssRadio<channelized::ChannelizedRadio> {
    pub fn tune_next(&mut self) {
        let ch = self.next_channel();
        self.inner.set_channel(ch);
    }

    pub fn tx_hopped(&mut self, payload: &[u8]) -> Result<u32, LoRaError> {
        let ch = self.next_channel();
        self.inner.set_channel(ch);
        self.inner.tx_payload(payload)?;
        self.packet_counter = self.packet_counter.wrapping_add(1);
        Ok(ch)
    }

    /// RX side: compute the NEXT expected channel, tune to it, receive.
    /// Requires TX and RX counters are in sync — this implementation
    /// advances the counter on every successful RX (and on timeout, so
    /// both sides stay aligned even if a packet is lost to the jammer).
    pub fn rx_hopped(&mut self, buf: &mut [u8], timeout_ms: u32)
        -> Result<(u32, usize), LoRaError>
    {
        let ch = self.next_channel();
        self.inner.set_channel(ch);
        let res = self.inner.rx_payload(buf, timeout_ms);
        // Advance counter whether we got a packet or timed out — keeps TX/RX
        // slot-aligned even under jamming. This is the same invariant real
        // FHSS systems maintain: slot progression is driven by shared clock,
        // not by successful reception.
        self.packet_counter = self.packet_counter.wrapping_add(1);
        match res {
            Ok(n) => Ok((ch, n)),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::channelized::ChannelizedRadio;

    fn run_trial(seed: [u8; 32], n_channels: u32, jam: Option<u32>, packets: u32)
        -> (u32, u32)  // (sent, received)
    {
        let (radio_a, radio_b, bus) = ChannelizedRadio::pair(n_channels);
        if let Some(j) = jam { bus.set_jammed(j, true); }

        let mut fhss_tx = FhssRadio::new(radio_a, seed, n_channels);
        let mut fhss_rx = FhssRadio::new(radio_b, seed, n_channels);

        let mut received = 0u32;
        for i in 0..packets {
            let _ = fhss_tx.tx_hopped(format!("pkt-{}", i).as_bytes()).unwrap();
            let mut buf = [0u8; 64];
            if fhss_rx.rx_hopped(&mut buf, 50).is_ok() {
                received += 1;
            }
        }
        (packets, received)
    }

    #[test]
    fn no_jam_no_fhss_100pct_baseline() {
        // Sanity: 1 channel, no jam — every packet gets through.
        let (sent, recv) = run_trial([0x11u8; 32], 1, None, 50);
        assert_eq!(recv, sent, "lossless baseline must deliver 100%");
    }

    #[test]
    fn jam_on_single_channel_0pct() {
        // 1 channel, jammer on it — 0 delivery. Shows the jammer
        // actually works and kills packets end-to-end.
        let (_sent, recv) = run_trial([0x11u8; 32], 1, Some(0), 50);
        assert_eq!(recv, 0, "narrowband jam on sole channel must deliver 0");
    }

    #[test]
    fn fhss_8_channels_single_jam_survives_approx_7_8() {
        // 8 channels, 1 jammed. Expected delivery = 7/8 = 87.5%.
        // Allow wide band for N=80 trials.
        let (sent, recv) = run_trial([0x22u8; 32], 8, Some(3), 80);
        let ratio = recv as f64 / sent as f64;
        assert!(ratio > 0.70 && ratio < 0.95,
            "FHSS 8-chan with 1 jammer should deliver 70-95%, got {}/{} = {:.2}",
            recv, sent, ratio);
    }

    #[test]
    fn hop_channel_is_deterministic_and_uniform() {
        // Same seed + counter → same channel, always.
        let seed = [0x33u8; 32];
        assert_eq!(hop_channel(&seed, 0, 8), hop_channel(&seed, 0, 8));
        // Different counters → should mostly hit different channels.
        // Sample 1024 counters, check that all 8 channels get hit at
        // least once (uniform enough).
        let mut hits = [0u32; 8];
        for c in 0..1024 {
            hits[hop_channel(&seed, c, 8) as usize] += 1;
        }
        for (i, h) in hits.iter().enumerate() {
            assert!(*h > 0, "channel {} never picked in 1024 samples", i);
            // Rough uniformity: each gets ~128; allow 96-160.
            assert!(*h > 96 && *h < 160,
                "channel {} hit count {} outside [96,160]", i, h);
        }
    }

    #[test]
    fn fhss_16_channels_single_jam_approaches_15_16() {
        // More channels → better anti-jam ratio.
        let (sent, recv) = run_trial([0x44u8; 32], 16, Some(7), 160);
        let ratio = recv as f64 / sent as f64;
        assert!(ratio > 0.85,
            "FHSS 16-chan with 1 jammer should deliver ≥85%, got {}/{} = {:.2}",
            recv, sent, ratio);
    }
}
