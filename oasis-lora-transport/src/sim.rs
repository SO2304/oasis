//! Host-only simulated LoRa radio. Routes bytes through std channels,
//! optional packet-loss injection, optional latency. Used for unit /
//! integration tests of the full OASIS → LoRa stack without hardware.
//!
//! Air interface model:
//! - Two instances share two channels (A→B and B→A).
//! - Each `tx_payload` push drops the bytes onto the outbound channel.
//! - Each `rx_payload` pulls from the inbound channel (with timeout).
//! - Loss model: uniform-random drop per packet (configurable).
//!
//! This is NOT a physical-layer simulator — it assumes bytes sent are
//! bytes received (post-loss). The SX1262 PHY is honest about what gets
//! through when conditions are good; loss comes from range, jamming, or
//! obstructions. The simulation covers the **byte-layer** (everything
//! above the radio's internal FEC), which is what OASIS cares about.

use crate::{LoRaError, LoRaParams, LoRaRadio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

pub struct SimulatedLoRaRadio {
    /// Bytes we send go out via this channel.
    tx: Sender<Vec<u8>>,
    /// Bytes we receive come in via this channel.
    rx: Receiver<Vec<u8>>,
    /// Uniform drop probability [0.0, 1.0] applied per-packet on TX side.
    pub loss_rate: f64,
    /// PRNG state (xorshift) for loss decisions. Deterministic for testing.
    prng: u64,
    params: Option<LoRaParams>,
}

impl SimulatedLoRaRadio {
    /// Construct a linked pair of radios. Returned tuple is (node_a, node_b)
    /// — A's TX goes to B's RX and vice versa, as real air would.
    pub fn linked_pair(seed: u64) -> (Self, Self) {
        let (tx_ab, rx_ab) = std::sync::mpsc::channel();
        let (tx_ba, rx_ba) = std::sync::mpsc::channel();
        let a = Self {
            tx: tx_ab,
            rx: rx_ba,
            loss_rate: 0.0,
            prng: seed,
            params: None,
        };
        let b = Self {
            tx: tx_ba,
            rx: rx_ab,
            loss_rate: 0.0,
            prng: seed ^ 0xDEAD_BEEF,
            params: None,
        };
        (a, b)
    }

    /// Set per-packet uniform drop rate. `0.0` = perfect link (default),
    /// `1.0` = always drop. Use to simulate degraded range or mild jamming.
    pub fn set_loss_rate(&mut self, rate: f64) {
        self.loss_rate = rate.clamp(0.0, 1.0);
    }

    fn next_rand(&mut self) -> f64 {
        // xorshift64
        self.prng ^= self.prng << 13;
        self.prng ^= self.prng >> 7;
        self.prng ^= self.prng << 17;
        (self.prng as f64) / (u64::MAX as f64)
    }
}

impl LoRaRadio for SimulatedLoRaRadio {
    fn init(&mut self, params: &LoRaParams) -> Result<(), LoRaError> {
        // No real hardware to configure; just cache the params so they're
        // visible to callers (and future assertions, e.g. "both nodes on
        // the same freq before TX").
        self.params = Some(*params);
        Ok(())
    }

    fn tx_payload(&mut self, payload: &[u8]) -> Result<(), LoRaError> {
        if self.loss_rate > 0.0 && self.next_rand() < self.loss_rate {
            // Simulated RF loss: silently drop. The real radio would have
            // sent it but the other side wouldn't lock onto the preamble.
            return Ok(());
        }
        self.tx
            .send(payload.to_vec())
            .map_err(|_| LoRaError::Driver("channel closed"))?;
        Ok(())
    }

    fn rx_payload(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, LoRaError> {
        let timeout = Duration::from_millis(timeout_ms as u64);
        match self.rx.recv_timeout(timeout) {
            Ok(bytes) => {
                if bytes.len() > buf.len() {
                    return Err(LoRaError::PayloadTooLarge(bytes.len()));
                }
                buf[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Err(RecvTimeoutError::Timeout) => Err(LoRaError::Timeout),
            Err(RecvTimeoutError::Disconnected) => Err(LoRaError::Driver("channel closed")),
        }
    }

    fn max_payload(&self) -> usize {
        255
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LoRaTransport;
    use oasis_rt::mesh::{
        mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    };

    fn fp(b: u8) -> [u8; 8] {
        [b, 0, 0, 0, 0, 0, 0, 0]
    }

    #[test]
    fn two_node_v10_roundtrip_over_simulated_lora() {
        let seed_a = MeshEdSeed([0x0Au8; 32]);
        let seed_b = MeshEdSeed([0x0Bu8; 32]);
        let pk_a = mesh_v10_pubkey_from_seed(&seed_a).unwrap();
        let pk_b = mesh_v10_pubkey_from_seed(&seed_b).unwrap();

        let mut reg_a = MeshPubRegistry::new();
        reg_a.insert(fp(0xBB), pk_b);
        let mut reg_b = MeshPubRegistry::new();
        reg_b.insert(fp(0xAA), pk_a);

        let mut router_a = MeshRouter::new_ed25519_signed(fp(0xAA), seed_a, reg_a);
        let mut router_b = MeshRouter::new_ed25519_signed(fp(0xBB), seed_b, reg_b);

        let (radio_a, radio_b) = SimulatedLoRaRadio::linked_pair(42);
        let mut tx = LoRaTransport::new(radio_a, LoRaParams::default()).unwrap();
        let mut rx = LoRaTransport::new(radio_b, LoRaParams::default()).unwrap();

        let envelope = router_a.origin_wrap(b"hello over lora");
        tx.send_envelope(&envelope, 0).unwrap();

        let mut buf = [0u8; 255];
        let env_rx = rx.recv_envelope(&mut buf, 500).unwrap();
        assert_eq!(env_rx, &envelope[..], "bytes survive the frame + channel");

        match router_b.process(env_rx) {
            MeshDecision::Arrived { .. } => {}
            other => panic!("B must accept v0A from A, got {:?}", other),
        }
    }

    #[test]
    fn loss_injection_drops_packets() {
        let (mut radio_a, mut radio_b) = SimulatedLoRaRadio::linked_pair(42);
        radio_a.set_loss_rate(1.0); // 100% loss one-way
        radio_a.init(&LoRaParams::default()).unwrap();
        radio_b.init(&LoRaParams::default()).unwrap();

        radio_a.tx_payload(b"drops").unwrap();

        let mut buf = [0u8; 32];
        let res = radio_b.rx_payload(&mut buf, 100);
        assert!(
            matches!(res, Err(LoRaError::Timeout)),
            "100% loss rate must produce Timeout on RX side"
        );
    }

    #[test]
    fn signed_envelope_survives_partial_loss() {
        // At 30% loss with no repeat layer, we'd expect most envelopes
        // through but some drops. This is the pre-FEC baseline; OASIS
        // already has repeat/FEC above the radio layer.
        let seed_a = MeshEdSeed([0x0Au8; 32]);
        let pk_a = mesh_v10_pubkey_from_seed(&seed_a).unwrap();
        let mut reg_b = MeshPubRegistry::new();
        reg_b.insert(fp(0xAA), pk_a);
        let mut router_a = MeshRouter::new_ed25519_signed(fp(0xAA), seed_a, MeshPubRegistry::new());
        let mut router_b =
            MeshRouter::new_ed25519_signed(fp(0xBB), MeshEdSeed([0x0Bu8; 32]), reg_b);

        let (mut radio_a, radio_b) = SimulatedLoRaRadio::linked_pair(7);
        radio_a.set_loss_rate(0.30);

        let mut tx = LoRaTransport::new(radio_a, LoRaParams::default()).unwrap();
        let mut rx = LoRaTransport::new(radio_b, LoRaParams::default()).unwrap();

        let mut arrived = 0;
        let mut sent = 0;
        for i in 0..50 {
            let env = router_a.origin_wrap(format!("msg {}", i).as_bytes());
            tx.send_envelope(&env, 0).unwrap();
            sent += 1;
            let mut buf = [0u8; 255];
            if let Ok(env_rx) = rx.recv_envelope(&mut buf, 50) {
                if let MeshDecision::Arrived { .. } = router_b.process(env_rx) {
                    arrived += 1;
                }
            }
        }
        // Expect ~70 % delivery at 30 % loss. Allow wide band (binomial noise
        // at N=50). The point is: delivery is neither 0 nor 100 — real channel.
        assert!(
            arrived < sent,
            "loss injection must drop at least one packet"
        );
        assert!(
            arrived > sent / 3,
            "majority should survive at 30% loss (got {}/{})",
            arrived,
            sent
        );
    }
}
