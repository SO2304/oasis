//! Host-only Sim Secure Element. Holds the seed in process memory
//! (which is exactly what production must NOT do) — the simulation is
//! about the contract + tamper-wipe semantics, not about actually
//! protecting keys against host-level attackers.
//!
//! For unit / integration tests of the OASIS-SE contract.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{SeError, SecureElement, TamperReason};

/// Realistic ATECC608B sign() latency on real hardware:
/// - Wakeup: ~1.5 ms
/// - I2C cmd transfer (Sign cmd, 7 bytes @ 400 kHz): ~0.2 ms
/// - Internal Ed25519 compute on chip: ~50 ms (per Microchip datasheet)
/// - I2C response transfer (64-byte sig + framing @ 400 kHz): ~1.7 ms
/// - Sleep: ~0.05 ms
/// Total: ~53 ms typical, ~60 ms worst-case.
pub const REALISTIC_ATECC608B_SIGN_MS: u64 = 60;

/// Pubkey readback (GenKey command, no compute, just I2C read of slot):
/// ~3 ms total at 400 kHz I2C.
pub const REALISTIC_ATECC608B_PUBKEY_MS: u64 = 3;

pub struct SimSecureElement {
    /// Box so the seed lives on heap (mirrors how an SE driver would
    /// keep it across an internal interface) and so we can zero it out
    /// on wipe by overwriting through &mut self.
    seed: Option<Box<[u8; 32]>>,
    wiped_reason: Option<TamperReason>,
    /// Simulated I2C+chip latency for sign() in milliseconds. Default
    /// matches a real ATECC608B (~60 ms). Set to 0 to disable for
    /// pure-functional tests; set otherwise to model the real I2C-bus
    /// throughput cost on mesh signing.
    sign_latency_ms: u64,
    /// Latency for pubkey() readback (much cheaper — just I2C read of
    /// slot, no chip compute). Default ~3 ms.
    pubkey_latency_ms: u64,
    /// Counter exposed for benchmarks: total simulated latency (ms)
    /// charged across all operations on this SE instance.
    pub total_latency_ms: AtomicU64,
}

impl SimSecureElement {
    pub fn new() -> Self {
        Self {
            seed: None,
            wiped_reason: None,
            sign_latency_ms: REALISTIC_ATECC608B_SIGN_MS,
            pubkey_latency_ms: REALISTIC_ATECC608B_PUBKEY_MS,
            total_latency_ms: AtomicU64::new(0),
        }
    }

    /// Construct an SE with ZERO simulated latency — for fast tests
    /// that exercise the contract / state machine but don't model the
    /// real-hardware throughput cost.
    pub fn new_no_latency() -> Self {
        Self {
            seed: None,
            wiped_reason: None,
            sign_latency_ms: 0,
            pubkey_latency_ms: 0,
            total_latency_ms: AtomicU64::new(0),
        }
    }

    /// Override the simulated latencies. Useful for sweeping the
    /// parameter space (slow ATECC608A vs faster SE050).
    pub fn set_latency(&mut self, sign_ms: u64, pubkey_ms: u64) {
        self.sign_latency_ms = sign_ms;
        self.pubkey_latency_ms = pubkey_ms;
    }

    /// Inspect the wipe reason if the SE has been wiped. None means
    /// either not wiped, or wiped without a recorded reason (real SEs
    /// always have a reason).
    pub fn wipe_reason(&self) -> Option<TamperReason> {
        self.wiped_reason
    }

    fn keypair(&self) -> Result<ed25519_compact::KeyPair, SeError> {
        if self.is_wiped() {
            return Err(SeError::Wiped);
        }
        let seed = self.seed.as_ref().ok_or(SeError::NotProvisioned)?;
        let s = ed25519_compact::Seed::from_slice(seed.as_ref())
            .map_err(|_| SeError::InternalCrypto)?;
        Ok(ed25519_compact::KeyPair::from_seed(s))
    }
}

impl SecureElement for SimSecureElement {
    fn provision(&mut self, seed: &[u8; 32]) -> Result<(), SeError> {
        if self.is_wiped() {
            return Err(SeError::Wiped);
        }
        self.seed = Some(Box::new(*seed));
        Ok(())
    }

    fn pubkey(&self) -> Result<[u8; 32], SeError> {
        let kp = self.keypair()?;
        if self.pubkey_latency_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.pubkey_latency_ms));
            self.total_latency_ms
                .fetch_add(self.pubkey_latency_ms, Ordering::SeqCst);
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(kp.pk.as_ref());
        Ok(out)
    }

    fn sign(&self, msg: &[u8]) -> Result<[u8; 64], SeError> {
        let kp = self.keypair()?;
        // Model the I2C + on-chip latency. On a real ATECC608B this is
        // dominated by the chip's internal Ed25519 compute (~50 ms);
        // the I2C transfer is small at 400 kHz. Sleeping here is the
        // honest way to make benchmarks predict real-hardware throughput.
        if self.sign_latency_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.sign_latency_ms));
            self.total_latency_ms
                .fetch_add(self.sign_latency_ms, Ordering::SeqCst);
        }
        let sig = kp.sk.sign(msg, None);
        let mut out = [0u8; 64];
        out.copy_from_slice(sig.as_ref());
        Ok(out)
    }

    fn wipe(&mut self, reason: TamperReason) -> Result<(), SeError> {
        // Real SE: fuse-blow + key-slot zero. Simulated: drop the Box
        // (the heap allocation is freed; if Rust reuses the page
        // immediately, a memory walker still might find the pattern,
        // but on a real SE this is hardware-enforced erasure).
        if let Some(mut s) = self.seed.take() {
            // Manual zeroize before drop to make the intent explicit.
            for b in s.iter_mut() {
                *b = 0;
            }
            drop(s);
        }
        self.wiped_reason = Some(reason);
        Ok(())
    }

    fn is_wiped(&self) -> bool {
        self.wiped_reason.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oasis_rt::mesh::{
        mesh_v10_pubkey_from_seed, MeshDecision, MeshEdPub, MeshEdSeed, MeshPubRegistry, MeshRouter,
    };

    fn fp(b: u8) -> [u8; 8] {
        [b, 0, 0, 0, 0, 0, 0, 0]
    }

    #[test]
    fn provision_then_sign_then_pubkey_consistent() {
        let mut se = SimSecureElement::new_no_latency();
        assert_eq!(se.pubkey(), Err(SeError::NotProvisioned));
        assert_eq!(se.sign(b"x"), Err(SeError::NotProvisioned));
        se.provision(&[0x55; 32]).unwrap();
        let pk = se.pubkey().unwrap();
        let sig = se.sign(b"hello").unwrap();
        // Verify with ed25519-compact directly
        let pk_obj = ed25519_compact::PublicKey::from_slice(&pk).unwrap();
        let sig_obj = ed25519_compact::Signature::from_slice(&sig).unwrap();
        assert!(pk_obj.verify(b"hello", &sig_obj).is_ok());
    }

    #[test]
    fn wipe_makes_all_operations_fail() {
        let mut se = SimSecureElement::new_no_latency();
        se.provision(&[0x55; 32]).unwrap();
        assert!(!se.is_wiped());
        se.wipe(TamperReason::ChassisSwitch).unwrap();
        assert!(se.is_wiped());
        assert_eq!(se.pubkey(), Err(SeError::Wiped));
        assert_eq!(se.sign(b"anything"), Err(SeError::Wiped));
        assert_eq!(se.wipe_reason(), Some(TamperReason::ChassisSwitch));
        // Re-provision should fail too — wipe is permanent.
        assert_eq!(se.provision(&[0x77; 32]), Err(SeError::Wiped));
    }

    #[test]
    fn se_signed_envelope_verifies_via_standard_meshrouter() {
        // The KEY integration test. Build an envelope using the SE-backed
        // sign path; verify it on the receiver side using the unmodified
        // MeshRouter::process(). The wire format must be byte-compatible.
        //
        // We use a regular MeshRouter for the sender (it builds the
        // envelope frame), and our SE-helper for the signature itself.
        // In production, MeshRouter would be refactored to call into
        // the SE for the signing step instead of holding the seed —
        // that's a separate API-design round.
        let seed_bytes = [0x88; 32];

        // Plaintext path — the "would-be" sender builds an envelope normally
        // using its in-process seed (this is what we want to REPLACE with
        // SE-signing in production).
        let pk_a = mesh_v10_pubkey_from_seed(&MeshEdSeed(seed_bytes)).unwrap();
        let mut router_a = MeshRouter::new_ed25519_signed(
            fp(0xAA),
            MeshEdSeed(seed_bytes),
            MeshPubRegistry::new(),
        );
        let env = router_a.origin_wrap(b"telemetry");

        // Receiver — same as production
        let mut reg_b = MeshPubRegistry::new();
        reg_b.insert(fp(0xAA), pk_a);
        let mut router_b = MeshRouter::new_ed25519_signed(fp(0xBB), MeshEdSeed([0x99; 32]), reg_b);

        match router_b.process(&env) {
            MeshDecision::Arrived { .. } => {}
            other => panic!("standard verify should accept: {:?}", other),
        }

        // Now show the SE produces the same signature for the same preimage.
        // The msg_id is hashed from origin_fp + counter inside MeshRouter,
        // so we extract it from the envelope to feed the SE.
        let mut msg_id_bytes = [0u8; 8];
        msg_id_bytes.copy_from_slice(&env[6..14]);
        let msg_id = u64::from_le_bytes(msg_id_bytes);

        let mut se = SimSecureElement::new_no_latency();
        se.provision(&seed_bytes).unwrap();
        let sig_via_se = crate::sign_v10_envelope_via_se(&se, msg_id, fp(0xAA)).unwrap();

        // The signature in the envelope sits at bytes 25..89.
        let sig_in_envelope = &env[25..89];
        assert_eq!(
            &sig_via_se[..],
            sig_in_envelope,
            "SE-produced signature must match in-process signature byte-for-byte"
        );
    }

    #[test]
    fn pubkey_matches_oasis_rt_helper() {
        // SE.pubkey() must equal oasis_rt::mesh::mesh_v10_pubkey_from_seed.
        // This is what enables the receiver's registry to be populated
        // either way — provisioning identity, then later enrolling via SE,
        // produces the same fingerprint.
        let seed_bytes = [0xAB; 32];
        let mut se = SimSecureElement::new_no_latency();
        se.provision(&seed_bytes).unwrap();
        let se_pk = se.pubkey().unwrap();

        let oasis_pk: MeshEdPub = mesh_v10_pubkey_from_seed(&MeshEdSeed(seed_bytes)).unwrap();
        assert_eq!(
            &se_pk[..],
            &oasis_pk.0[..],
            "SE pubkey must equal the standard helper output"
        );
    }
}
