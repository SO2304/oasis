//! OASIS — signed firmware update: manifest and install rule (Phase 1.3).
//!
//! Spec: `docs/specs/FIRMWARE_UPDATE_SPEC.md` §3-4. The A/B swap itself is
//! embassy-boot's; this module is the gate the RUNNING firmware applies before
//! asking for a swap. The manifest is the content of an `OAU1` kind-3 message
//! (`authority::kind::FIRMWARE_MANIFEST`), signed by the current owner, hybrid by
//! default:
//!
//! `version u32 LE | image_len u32 LE | sha256(image)[32] | hw_id[8]`
//!
//! The image carries its own version in a header placed right after the vector
//! table (`FWINFO_OFFSET`): `"OFWI" | version u32 LE`, covered by the hash.
//!
//! Anti-rollback floor: a monotone `u64` stored with `tx_lease::DualSlotStore`.
//! Install iff `version >= floor && version > running`; a new image raises the floor
//! to its version before confirming itself.

pub const MANIFEST_LEN: usize = 4 + 4 + 32 + 8;
/// Board class accepted by these nodes.
pub const HW_ID: [u8; 8] = *b"RP2040U1";
/// Size of the ACTIVE partition (the largest installable image).
pub const ACTIVE_MAX: usize = 512 * 1024;
/// Cortex-M0+ vector table with the RP2040's 32 interrupts: 48 words.
pub const FWINFO_OFFSET: usize = 0xC0;
pub const FWINFO_MAGIC: [u8; 4] = *b"OFWI";
pub const FWINFO_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub image_len: u32,
    pub sha256: [u8; 32],
    pub hw_id: [u8; 8],
}

pub fn parse_manifest(c: &[u8]) -> Option<Manifest> {
    if c.len() != MANIFEST_LEN {
        return None;
    }
    let mut sha256 = [0u8; 32];
    sha256.copy_from_slice(&c[8..40]);
    let mut hw_id = [0u8; 8];
    hw_id.copy_from_slice(&c[40..48]);
    Some(Manifest {
        version: u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
        image_len: u32::from_le_bytes([c[4], c[5], c[6], c[7]]),
        sha256,
        hw_id,
    })
}

pub fn encode_manifest(m: &Manifest) -> [u8; MANIFEST_LEN] {
    let mut c = [0u8; MANIFEST_LEN];
    c[0..4].copy_from_slice(&m.version.to_le_bytes());
    c[4..8].copy_from_slice(&m.image_len.to_le_bytes());
    c[8..40].copy_from_slice(&m.sha256);
    c[40..48].copy_from_slice(&m.hw_id);
    c
}

/// `"OFWI" | version u32` at `FWINFO_OFFSET` of an image, if present.
pub fn image_version(image: &[u8]) -> Option<u32> {
    let h = image.get(FWINFO_OFFSET..FWINFO_OFFSET + FWINFO_LEN)?;
    if h[0..4] != FWINFO_MAGIC {
        return None;
    }
    Some(u32::from_le_bytes([h[4], h[5], h[6], h[7]]))
}

pub fn encode_fwinfo(version: u32) -> [u8; FWINFO_LEN] {
    let mut h = [0u8; FWINFO_LEN];
    h[0..4].copy_from_slice(&FWINFO_MAGIC);
    h[4..8].copy_from_slice(&version.to_le_bytes());
    h
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateReject {
    /// The manifest did not pass `authority::verify_authority` (signature, network,
    /// policy) or is not a firmware manifest.
    NotAuthorized,
    Malformed,
    WrongHardware,
    TooLarge,
    /// SHA-256 of the uploaded image differs from the manifest.
    HashMismatch,
    /// The image's own version header is missing or differs from the manifest.
    VersionMismatch,
    /// Below the anti-rollback floor.
    Rollback,
    /// Not newer than the running firmware.
    NotNewer,
}

/// Everything the install rule depends on, already evaluated by the caller.
#[derive(Debug, Clone, Copy)]
pub struct InstallInput {
    pub manifest_authorized: bool,
    pub hw_ok: bool,
    pub image_len: u32,
    pub hash_ok: bool,
    pub image_version: Option<u32>,
    pub manifest_version: u32,
    pub floor: u64,
    pub running_version: u32,
}

/// The install rule (pure). `Ok` only if every condition holds; otherwise the first
/// failing one, in this order.
pub fn install_decision(i: &InstallInput) -> Result<(), UpdateReject> {
    if !i.manifest_authorized {
        return Err(UpdateReject::NotAuthorized);
    }
    if !i.hw_ok {
        return Err(UpdateReject::WrongHardware);
    }
    if i.image_len == 0 || i.image_len as usize > ACTIVE_MAX {
        return Err(UpdateReject::TooLarge);
    }
    if !i.hash_ok {
        return Err(UpdateReject::HashMismatch);
    }
    if i.image_version != Some(i.manifest_version) {
        return Err(UpdateReject::VersionMismatch);
    }
    if (i.manifest_version as u64) < i.floor {
        return Err(UpdateReject::Rollback);
    }
    if i.manifest_version <= i.running_version {
        return Err(UpdateReject::NotNewer);
    }
    Ok(())
}

/// Floor after a new image confirms itself: never lower.
pub fn raised_floor(floor: u64, running_version: u32) -> u64 {
    floor.max(running_version as u64)
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
