//! Phase 1.3 on silicon: signed A/B firmware update under the embassy-boot
//! bootloader (`oasis-bootloader`). Spec: docs/specs/FIRMWARE_UPDATE_SPEC.md.
//!
//! - `PartFlash`: `embedded-storage` NorFlash over `rp2040-flash`, written the way
//!   embassy-rp writes (WRITE_SIZE 1, partial 256-byte pages padded with 0xFF, 4 KiB
//!   erase), so this firmware and the bootloader read the same state partition.
//! - The install gate is `oasis_rt::firmware::install_decision` after
//!   `authority::verify_authority` of an OAU1 kind-3 manifest: embassy-boot's own
//!   signature check is not enabled.
//! - Anti-rollback floor: `tx_lease::DualSlotStore` (the counter-lease mechanism),
//!   two sectors at 0x1F2000/0x1F3000. Not wiped by the test harness's `!`.

use embassy_boot::{BlockingFirmwareUpdater, FirmwareUpdaterConfig, State};
use embedded_storage::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};
use oasis_rt::authority::{kind, verify_authority, AuthPolicy, AuthorityKeys};
use oasis_rt::firmware::{
    image_version, install_decision, parse_manifest, InstallInput, UpdateReject, HW_ID,
};
use oasis_rt::tx_lease::{CeilingStore, DualSlotStore, SlotIo, TX_LEASE_RECORD_LEN};
use sha2::{Digest, Sha256};

include!(concat!(env!("OUT_DIR"), "/fwinfo.rs"));

const XIP_BASE: usize = 0x1000_0000;
pub const STATE_OFFSET: u32 = 0x0000_7000;
pub const STATE_LEN: u32 = 4 * 1024;
pub const DFU_OFFSET: u32 = 0x0008_8000;
pub const DFU_LEN: u32 = 516 * 1024;
const FLOOR_SECTORS: [u32; 2] = [0x1F_2000, 0x1F_3000];

/// Version header, right after the vector table (`.oasis_fwinfo`, see
/// memory_bootloaded.x): `"OFWI" | version u32 LE`, covered by the image hash.
#[link_section = ".oasis_fwinfo"]
#[used]
pub static FWINFO: [u8; 8] = {
    let v = FW_VERSION.to_le_bytes();
    [b'O', b'F', b'W', b'I', v[0], v[1], v[2], v[3]]
};

// ── NorFlash over rp2040-flash ───────────────────────────────────────────────
#[derive(Debug, Clone, Copy)]
pub struct FlashErr(NorFlashErrorKind);
impl NorFlashError for FlashErr {
    fn kind(&self) -> NorFlashErrorKind {
        self.0
    }
}

/// One partition of the 2 MiB flash, addressed from 0.
pub struct PartFlash {
    base: u32,
    len: u32,
}

impl PartFlash {
    pub fn state() -> Self {
        PartFlash {
            base: STATE_OFFSET,
            len: STATE_LEN,
        }
    }
    pub fn dfu() -> Self {
        PartFlash {
            base: DFU_OFFSET,
            len: DFU_LEN,
        }
    }
    fn check(&self, offset: u32, len: usize) -> Result<(), FlashErr> {
        if offset as u64 + len as u64 > self.len as u64 {
            return Err(FlashErr(NorFlashErrorKind::OutOfBounds));
        }
        Ok(())
    }
}

impl ErrorType for PartFlash {
    type Error = FlashErr;
}

impl ReadNorFlash for PartFlash {
    const READ_SIZE: usize = 1;
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), FlashErr> {
        self.check(offset, bytes.len())?;
        let p = (XIP_BASE + (self.base + offset) as usize) as *const u8;
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile(p.add(i)) };
        }
        Ok(())
    }
    fn capacity(&self) -> usize {
        self.len as usize
    }
}

impl NorFlash for PartFlash {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = 4096;
    fn erase(&mut self, from: u32, to: u32) -> Result<(), FlashErr> {
        if from > to || from % 4096 != 0 || to % 4096 != 0 {
            return Err(FlashErr(NorFlashErrorKind::NotAligned));
        }
        self.check(from, (to - from) as usize)?;
        cortex_m::interrupt::free(|_| unsafe {
            rp2040_flash::flash::flash_range_erase(self.base + from, to - from, true);
        });
        Ok(())
    }
    /// Programs only clear bits, so 0xFF padding leaves the rest of a page as is.
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), FlashErr> {
        self.check(offset, bytes.len())?;
        let mut addr = self.base + offset;
        let mut rest = bytes;
        while !rest.is_empty() {
            let page = addr & !0xFF;
            let start = (addr - page) as usize;
            let n = (256 - start).min(rest.len());
            let mut buf = [0xFFu8; 256];
            buf[start..start + n].copy_from_slice(&rest[..n]);
            cortex_m::interrupt::free(|_| unsafe {
                rp2040_flash::flash::flash_range_program(page, &buf, true);
            });
            addr += n as u32;
            rest = &rest[n..];
        }
        Ok(())
    }
}

pub type Updater<'a> = BlockingFirmwareUpdater<'a, PartFlash, PartFlash>;

pub fn updater(aligned: &mut [u8; 1]) -> Updater<'_> {
    BlockingFirmwareUpdater::new(
        FirmwareUpdaterConfig {
            dfu: PartFlash::dfu(),
            state: PartFlash::state(),
        },
        aligned,
    )
}

// ── anti-rollback floor: the counter-lease dual-slot store ───────────────────
pub struct FloorSlots;
impl SlotIo for FloorSlots {
    fn read(&self, slot: usize) -> [u8; TX_LEASE_RECORD_LEN] {
        let p = (XIP_BASE + FLOOR_SECTORS[slot] as usize) as *const u8;
        let mut r = [0u8; TX_LEASE_RECORD_LEN];
        for (i, b) in r.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile(p.add(i)) };
        }
        r
    }
    fn write(&mut self, slot: usize, rec: &[u8; TX_LEASE_RECORD_LEN]) -> bool {
        let mut page = [0xFFu8; 256];
        page[..TX_LEASE_RECORD_LEN].copy_from_slice(rec);
        cortex_m::interrupt::free(|_| unsafe {
            rp2040_flash::flash::flash_range_erase(FLOOR_SECTORS[slot], 4096, true);
            rp2040_flash::flash::flash_range_program(FLOOR_SECTORS[slot], &page, true);
        });
        true
    }
}

pub fn floor_store() -> DualSlotStore<FloorSlots> {
    DualSlotStore::new(FloorSlots)
}

pub fn floor() -> u64 {
    floor_store().load().unwrap_or(0)
}

/// Raise the floor to this image's version (never lowers). `false` if not durable.
pub fn raise_floor() -> bool {
    let f = floor();
    let target = oasis_rt::firmware::raised_floor(f, FW_VERSION);
    target == f || floor_store().store(target)
}

// ── boot breadcrumbs (Phase 1.3 bring-up diagnostics) ─────────────────────────
// Same sector and scheme as oasis-bootloader: one byte appended at the first 0xFF of
// 0x1F0000. App codes: 0xA1 entry, 0xA2 clocks, 0xA3 USB objects, 0xA4 state loaded,
// 0xA5 confirmed + watchdog stopped, 0xA6 main loop, 0xAE panic, 0xAF HardFault.
pub const CRUMB_OFFSET: u32 = 0x1F_0000;

pub fn crumb(b: u8) {
    let base = XIP_BASE + CRUMB_OFFSET as usize;
    for off in 0..4096usize {
        if unsafe { core::ptr::read_volatile((base + off) as *const u8) } == 0xFF {
            let addr = CRUMB_OFFSET + off as u32;
            let page = addr & !0xFF;
            let mut buf = [0xFFu8; 256];
            buf[(addr - page) as usize] = b;
            cortex_m::interrupt::free(|_| unsafe {
                rp2040_flash::flash::flash_range_program(page, &buf, true);
            });
            return;
        }
    }
}

// ── boot guard (owned by oasis-bootloader) ──────────────────────────────────
// The bootloader counts, in watchdog SCRATCH2, boots that did not reach this main
// loop and enters BOOTSEL after 3 in a row; this firmware only clears the count once
// its main loop runs. SCRATCH3 = last init stage reached (the bootloader copies it
// into its breadcrumbs on the next boot). Both persist across watchdog/soft resets.
const GUARD_MAGIC: u32 = 0x0A5E_0000;

fn wd() -> &'static rp2040_hal::pac::watchdog::RegisterBlock {
    unsafe { &*rp2040_hal::pac::WATCHDOG::ptr() }
}

/// First thing in `main`: (failed boots before this one, stage the previous boot reached).
pub fn boot_guard_enter() -> (u32, u32) {
    let s2 = wd().scratch2().read().bits();
    let prev_stage = wd().scratch3().read().bits();
    let this_boot = if s2 & 0xFFFF_0000 == GUARD_MAGIC {
        s2 & 0xFFFF
    } else {
        1
    };
    wd().scratch3().write(|w| unsafe { w.bits(1) });
    (this_boot.saturating_sub(1), prev_stage)
}

/// Bring-up diagnostics: write any watchdog scratch register (0..=3; the bootrom's
/// USB boot only uses 0/1 and 4..7 when it is entered).
pub fn scratch_mark(n: usize, v: u32) {
    let base = rp2040_hal::pac::WATCHDOG::ptr() as *mut u32;
    // SCRATCH0 is at +0x0C; registers are 4 bytes apart.
    unsafe { base.add(3 + n.min(3)).write_volatile(v) };
}

/// Record the init stage reached (read back by the next boot).
pub fn stage(n: u32) {
    wd().scratch3().write(|w| unsafe { w.bits(n) });
}

/// The main loop is running: this boot counts as good.
pub fn boot_guard_ok() {
    wd().scratch2().write(|w| unsafe { w.bits(0) });
    stage(100);
}

// ── boot: confirm a freshly swapped image ────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootOutcome {
    /// Normal boot of a confirmed image.
    Normal,
    /// First boot after a swap: self-test passed, floor raised, marked booted.
    ConfirmedAfterSwap,
    /// The bootloader reverted a previous image that did not confirm itself.
    Reverted,
    /// Self-test failed after a swap: not marked booted; a reset will revert.
    SelfTestFailed,
    StateUnreadable,
}

/// Self-test of a new image before it confirms itself: verify the compiled hybrid
/// test vector (signed by owner #1) with the full gate. With `fw_selftest_hang`, the
/// test image hangs here instead, so the watchdog resets it and the bootloader reverts.
fn self_test() -> bool {
    #[cfg(feature = "fw_selftest_hang")]
    loop {
        cortex_m::asm::nop();
    }
    #[allow(unreachable_code)]
    {
        let keys = AuthorityKeys {
            ed25519: crate::ef::OPERATOR_PUB,
            mldsa44: crate::pq::OPERATOR_MLDSA_PK,
        };
        let blob: &[u8] = include_bytes!("pq/rev_hybrid_e1.oau1");
        verify_authority(&AuthPolicy::default(), &crate::NETWORK_ID, &keys, blob).is_ok()
    }
}

/// At the main-loop entry, once the whole init has run (the bootloader's watchdog is
/// still running until the caller stops it).
pub fn boot_confirm(up: &mut Updater) -> BootOutcome {
    match up.get_state() {
        Ok(State::Swap) => {
            if !self_test() {
                return BootOutcome::SelfTestFailed;
            }
            // Floor first: a cut before mark_booted reverts, but the floor already
            // covers this version, so the old image cannot come back by update.
            if !raise_floor() || up.mark_booted().is_err() {
                return BootOutcome::SelfTestFailed;
            }
            BootOutcome::ConfirmedAfterSwap
        }
        Ok(State::Revert) => BootOutcome::Reverted,
        Ok(_) => {
            raise_floor();
            BootOutcome::Normal
        }
        Err(_) => BootOutcome::StateUnreadable,
    }
}

pub fn state_name(up: &mut Updater) -> &'static str {
    match up.get_state() {
        Ok(State::Boot) => "Boot",
        Ok(State::Swap) => "Swap",
        Ok(State::Revert) => "Revert",
        Ok(State::DfuDetach) => "DfuDetach",
        Err(_) => "unreadable",
    }
}

// ── install: the gate before asking for a swap ───────────────────────────────
/// SHA-256 and version header of the first `len` bytes of DFU (read through XIP).
fn dfu_digest_and_version(len: usize) -> ([u8; 32], Option<u32>) {
    let p = (XIP_BASE + DFU_OFFSET as usize) as *const u8;
    let img = unsafe { core::slice::from_raw_parts(p, len) };
    (Sha256::digest(img).into(), image_version(img))
}

/// Verify the staged manifest against the uploaded image; on success mark the
/// update (the caller resets so the bootloader swaps).
pub fn install(
    up: &mut Updater,
    policy: &AuthPolicy,
    owner: &AuthorityKeys,
    msg: &[u8],
) -> Result<(u32, [u8; 32]), UpdateReject> {
    let p = match verify_authority(policy, &crate::NETWORK_ID, owner, msg) {
        Ok(p) if p.kind == kind::FIRMWARE_MANIFEST => p,
        _ => return Err(UpdateReject::NotAuthorized),
    };
    let m = parse_manifest(p.content).ok_or(UpdateReject::Malformed)?;
    let len = (m.image_len as usize).min(DFU_LEN as usize);
    let (digest, ver) = dfu_digest_and_version(len);
    install_decision(&InstallInput {
        manifest_authorized: true,
        hw_ok: m.hw_id == HW_ID,
        image_len: m.image_len,
        hash_ok: digest == m.sha256,
        image_version: ver,
        manifest_version: m.version,
        floor: floor(),
        running_version: FW_VERSION,
    })?;
    up.mark_updated().map_err(|_| UpdateReject::Malformed)?;
    // The digest travels back with the version. It is already computed and compared
    // above; returning it lets the caller record **which bytes** were installed, which
    // is what Annex III 1.1.9 para 4 asks a machine to be able to say about the software
    // on it — a measured digest rather than a compile-time constant.
    Ok((m.version, digest))
}
