//! Phase 1.2 on silicon: identity generated on the board, owner record, enrolled-node
//! registry. Pure rules live in oasis-rt (`identity`, `enrollment`, `ownership`);
//! this file only samples the ring oscillator and reads/writes flash.
//! Spec: docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md §2-4.
//!
//! The seed never leaves through the firmware (no command prints it). It is NOT
//! protected by hardware: the RP2040 flash is readable in BOOTSEL (picotool) and over
//! SWD, and this test firmware reboots to BOOTSEL on `b`.

use alloc::vec;
use alloc::vec::Vec;
use oasis_rt::enrollment::Registry;
use oasis_rt::identity::{
    condition_seed, decode_identity, encode_identity, health_ok, rekey, seed_usable, RAW_SAMPLES,
};
use oasis_rt::ownership::{OwnerKeys, OwnerState};
use rp2040_hal::pac;

const IDENTITY_SECTOR: u32 = 0x1F_8000;
const OWNER_SECTORS: [u32; 2] = [0x1F_6000, 0x1F_7000];
const REGISTRY_SECTORS: [u32; 2] = [0x1F_4000, 0x1F_5000];
const XIP_BASE: usize = 0x1000_0000;
/// Core cycles between two RANDOMBIT reads (~0.8 µs at 125 MHz, several ROSC
/// periods): reading faster than the ROSC toggles would repeat bits.
pub const ROSC_SAMPLE_DELAY_CYCLES: u32 = 100;
const KEYGEN_ATTEMPTS: usize = 3;

// ── flash ────────────────────────────────────────────────────────────────────
pub fn read_sector(addr: u32) -> Vec<u8> {
    let p = (XIP_BASE + addr as usize) as *const u8;
    (0..4096)
        .map(|i| unsafe { core::ptr::read_volatile(p.add(i)) })
        .collect()
}

/// Erase the sector and program `data` (padded to whole 256-byte pages); then read
/// back. `false` if the read-back differs.
pub fn write_sector(addr: u32, data: &[u8]) -> bool {
    if data.len() > 4096 {
        return false;
    }
    let mut buf = vec![0xFFu8; data.len().div_ceil(256).max(1) * 256];
    buf[..data.len()].copy_from_slice(data);
    cortex_m::interrupt::free(|_| unsafe {
        rp2040_flash::flash::flash_range_erase(addr, 4096, true);
        rp2040_flash::flash::flash_range_program(addr, &buf, true);
    });
    read_sector(addr)[..data.len()] == *data
}

pub fn wipe_sectors() {
    cortex_m::interrupt::free(|_| unsafe {
        for s in [
            IDENTITY_SECTOR,
            OWNER_SECTORS[0],
            OWNER_SECTORS[1],
            REGISTRY_SECTORS[0],
            REGISTRY_SECTORS[1],
        ] {
            rp2040_flash::flash::flash_range_erase(s, 4096, true);
        }
    });
}

// ── entropy ──────────────────────────────────────────────────────────────────
/// The ROSC must be running (the system clock runs from the XOSC/PLL, as the
/// datasheet requires for RANDOMBIT to vary at all).
pub fn rosc_enable() {
    let rosc = unsafe { &*pac::ROSC::ptr() };
    rosc.ctrl().write(|w| w.enable().enable());
}

/// `nbits` raw RANDOMBIT samples, packed LSB first.
pub fn sample_bits(nbits: usize) -> Vec<u8> {
    let rosc = unsafe { &*pac::ROSC::ptr() };
    let mut out = vec![0u8; nbits.div_ceil(8)];
    for i in 0..nbits {
        cortex_m::asm::delay(ROSC_SAMPLE_DELAY_CYCLES);
        let b = rosc.randombit().read().randombit().bit() as u8;
        out[i / 8] |= b << (i % 8);
    }
    out
}

fn chip_id() -> [u8; 8] {
    let mut id = [0u8; 8];
    cortex_m::interrupt::free(|_| unsafe { rp2040_flash::flash::flash_unique_id(&mut id, true) });
    id
}

fn timer_ticks() -> u64 {
    let t = unsafe { &*pac::TIMER::ptr() };
    ((t.timerawh().read().bits() as u64) << 32) | t.timerawl().read().bits() as u64
}

pub enum IdBoot {
    Loaded {
        seed: [u8; 32],
        mixed: bool,
    },
    Generated {
        seed: [u8; 32],
        attempts: usize,
    },
    /// Health tests failed every time: no key, the node stays mute.
    EntropyFail,
    PersistFail,
}

/// Boot: the persisted identity, or a new one generated here (first boot).
pub fn boot_identity() -> IdBoot {
    if let Some((seed, mixed)) = decode_identity(&read_sector(IDENTITY_SECTOR)) {
        return IdBoot::Loaded { seed, mixed };
    }
    for attempt in 1..=KEYGEN_ATTEMPTS {
        let raw = sample_bits(RAW_SAMPLES);
        if !health_ok(&raw, RAW_SAMPLES) {
            continue;
        }
        let seed = condition_seed(&raw, timer_ticks(), &chip_id());
        if !seed_usable(&seed) {
            continue;
        }
        if !write_sector(IDENTITY_SECTOR, &encode_identity(&seed, false)) {
            return IdBoot::PersistFail;
        }
        return IdBoot::Generated {
            seed,
            attempts: attempt,
        };
    }
    IdBoot::EntropyFail
}

/// One-time mix-in of the tool's nonce (before enrollment). Persists `mixed = 1`.
pub fn rekey_identity(seed: &[u8; 32], nonce: &[u8; 32]) -> Option<[u8; 32]> {
    for _ in 0..KEYGEN_ATTEMPTS {
        let fresh = sample_bits(RAW_SAMPLES);
        if !health_ok(&fresh, RAW_SAMPLES) {
            continue;
        }
        let s = rekey(seed, nonce, &fresh);
        if seed_usable(&s) && write_sector(IDENTITY_SECTOR, &encode_identity(&s, true)) {
            return Some(s);
        }
    }
    None
}

// ── owner and registry ───────────────────────────────────────────────────────
/// Owner #1, compiled in (same keys as the E/F operator and Phase 1.1).
pub fn initial_owner() -> OwnerKeys {
    OwnerKeys::new(crate::ef::OPERATOR_PUB, &crate::pq::OPERATOR_MLDSA_PK[..]).unwrap()
}

pub fn load_owner() -> OwnerState {
    OwnerState::from_slots(
        &read_sector(OWNER_SECTORS[0]),
        &read_sector(OWNER_SECTORS[1]),
        initial_owner(),
    )
}

/// Write into the slot NOT holding the newest record, then read back.
pub fn persist_owner(s: &OwnerState) -> bool {
    let slot = OwnerState::slot_to_overwrite(
        &read_sector(OWNER_SECTORS[0]),
        &read_sector(OWNER_SECTORS[1]),
    );
    write_sector(OWNER_SECTORS[slot], &s.to_record())
}

pub fn load_registry() -> Registry {
    Registry::from_slots(
        &read_sector(REGISTRY_SECTORS[0]),
        &read_sector(REGISTRY_SECTORS[1]),
    )
}

pub fn persist_registry(r: &Registry) -> bool {
    let slot = Registry::slot_to_overwrite(
        &read_sector(REGISTRY_SECTORS[0]),
        &read_sector(REGISTRY_SECTORS[1]),
    );
    write_sector(REGISTRY_SECTORS[slot], &r.to_record())
}
