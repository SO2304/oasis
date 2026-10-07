//! OASIS — supervision beacon `OSB1` (part H of `docs/AUTHORITY_HARDENING_SPEC.md`).
//!
//! Règlement (UE) 2023/1230, annexe III, partie 3, verbatim: « **Si la fonction de
//! supervision n'est pas active, la machine ne peut pas fonctionner.** » So an `Act` order
//! needs a *live* supervision link, not merely a fresh order — a condition of **presence**,
//! distinct from the freshness of part F and from the stop asymmetry of part G.
//!
//! The beacon travels as kind 4 of the existing `OAU1` authority envelope, so it inherits
//! the signed suite byte, the per-kind minimum-suite policy and the downgrade refusal built
//! in phase 1.1 — nothing new is needed on the wire for the crypto.
//!
//! Format (little-endian): `"OSB1" | supervisor_fp [u8;8] | actuator_boot_id u64
//! | beacon_seq u32 | validity_ms u64` (32 bytes).
//!
//! `actuator_boot_id` binds the beacon to one boot of one actuator, for the same reason an
//! order is bound: a beacon from an earlier boot must not extend authorisation across a
//! restart.
//!
//! **This is not a safety function.** A dead supervision link withdraws the authorisation
//! to act; it does not bring the machine to a safe state by itself — that belongs to the
//! machine's own safety-related control system.

pub const OSB1_MAGIC: [u8; 4] = *b"OSB1";
pub const OSB1_LEN: usize = 32;

/// Longest authorisation a single beacon can grant. A beacon asking for more is clamped,
/// so a replayed or malformed `validity_ms` cannot grant an unbounded licence to act —
/// `beacon_seq` already refuses the replay, and this is the second barrier.
///
/// 5 minutes: at SF12 the EU868 duty cycle allows ~12 frames of 64 bytes per hour
/// (`docs/compliance/PQC.md`), so a shorter ceiling would be unreachable over long-range
/// LoRa. Fine-grained supervision needs a faster link, and that is a documented limit.
pub const MAX_SUPERVISION_MS: u64 = 300_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisionBeacon {
    pub supervisor_fp: [u8; 8],
    pub actuator_boot_id: u64,
    pub beacon_seq: u32,
    pub validity_ms: u64,
}

pub fn encode_osb1(b: &SupervisionBeacon) -> [u8; OSB1_LEN] {
    let mut o = [0u8; OSB1_LEN];
    o[0..4].copy_from_slice(&OSB1_MAGIC);
    o[4..12].copy_from_slice(&b.supervisor_fp);
    o[12..20].copy_from_slice(&b.actuator_boot_id.to_le_bytes());
    o[20..24].copy_from_slice(&b.beacon_seq.to_le_bytes());
    o[24..32].copy_from_slice(&b.validity_ms.to_le_bytes());
    o
}

/// Total on every input: any slice that is not exactly one well-formed beacon yields
/// `None`. Proved by `proof_osb1_parse_total`.
pub fn parse_osb1(b: &[u8]) -> Option<SupervisionBeacon> {
    if b.len() != OSB1_LEN || b[0..4] != OSB1_MAGIC {
        return None;
    }
    let mut fp = [0u8; 8];
    fp.copy_from_slice(&b[4..12]);
    let mut boot = [0u8; 8];
    boot.copy_from_slice(&b[12..20]);
    let mut val = [0u8; 8];
    val.copy_from_slice(&b[24..32]);
    Some(SupervisionBeacon {
        supervisor_fp: fp,
        actuator_boot_id: u64::from_le_bytes(boot),
        beacon_seq: u32::from_le_bytes([b[20], b[21], b[22], b[23]]),
        validity_ms: u64::from_le_bytes(val),
    })
}
