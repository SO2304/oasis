//! Flash store for the decision journal (part I of `docs/AUTHORITY_HARDENING_SPEC.md`).
//!
//! The pure chain lives in `oasis_rt::journal`; this is the RP2040 side that persists it.
//!
//! # Layout
//!
//! The OASIS state region `0x1F_0000..0x1F_FFFF` is **full** (crumbs, floor, registry,
//! owner, identity, policy, revocation, lease, v0B window), so the journal takes six
//! sectors immediately below it. Everything from the end of DFU (`0x10_9000`) up to
//! `0x1F_0000` is unused.
//!
//! | Sectors | Use |
//! |---|---|
//! | `0x1E_A000`..`0x1E_D000` (4) | entry ring, **one entry per 256-byte page**, 16 per sector, 64 slots |
//! | `0x1E_E000`, `0x1E_F000` (2) | head record, two alternating slots |
//!
//! One entry per page wastes 224 bytes of every page. That is deliberate: NOR flash
//! programs a page once per erase, so packing 8 entries into a page would mean either
//! buffering them in RAM — losing up to 7 on a power cut — or re-programming a page,
//! which is not reliable. Per-page entries give the power-cut semantics part I promises:
//! **at most one unconfirmed entry**, never a silent loss.
//!
//! # Write order, and what a power cut leaves
//!
//! 1. program the entry page;
//! 2. then update the head (alternating slot, monotone `rec`).
//!
//! A cut between the two leaves an entry the head does not cover —
//! `oasis_rt::journal::verify` reports `Unconfirmed`, which is reported, not hidden. A cut
//! during either write damages at most that one page or one head slot; the other head slot
//! still holds the previous record.

use oasis_rt::journal::{JournalHead, ENTRY_LEN};

const XIP_BASE: usize = 0x1000_0000;
const PAGE: u32 = 256;
const SECTOR: u32 = 4096;
const SLOTS_PER_SECTOR: u32 = SECTOR / PAGE; // 16

const RING_SECTORS: [u32; 4] = [0x1E_A000, 0x1E_B000, 0x1E_C000, 0x1E_D000];
const HEAD_SECTORS: [u32; 2] = [0x1E_E000, 0x1E_F000];

/// Entry slots in the ring.
pub const CAPACITY: u32 = RING_SECTORS.len() as u32 * SLOTS_PER_SECTOR;

const ENTRY_MAGIC: [u8; 4] = *b"JRNE";
const HEAD_MAGIC: [u8; 4] = *b"JRNH";
/// `magic 4 | rec 4 | boot_id 8 | has_seq 1 | seq 4 | hash 32 | overwritten 4 | sum 4`
const HEAD_LEN: usize = 61;

fn read_at(off: u32, len: usize) -> &'static [u8] {
    unsafe { core::slice::from_raw_parts((XIP_BASE + off as usize) as *const u8, len) }
}

fn erase(sector: u32) {
    cortex_m::interrupt::free(|_| unsafe {
        rp2040_flash::flash::flash_range_erase(sector, SECTOR, true);
    });
}

fn program(off: u32, page: &[u8; 256]) {
    cortex_m::interrupt::free(|_| unsafe {
        rp2040_flash::flash::flash_range_program(off, page, true);
    });
}

/// Trivial additive checksum over the first `HEAD_LEN - 4` bytes. Detects a torn write;
/// it is **not** a security control — the head's authenticity is not protected here, and
/// an attacker with flash access rewrites both slots. See the journal module's note on
/// what may be claimed.
fn head_sum(b: &[u8]) -> u32 {
    let mut s: u32 = 0x9E37_79B9;
    for (i, x) in b.iter().take(HEAD_LEN - 4).enumerate() {
        s = s.rotate_left(5) ^ ((*x as u32) << (i % 24));
    }
    s
}

fn encode_head(h: &JournalHead, rec: u32) -> [u8; 256] {
    let mut p = [0xFFu8; 256];
    p[0..4].copy_from_slice(&HEAD_MAGIC);
    p[4..8].copy_from_slice(&rec.to_le_bytes());
    p[8..16].copy_from_slice(&h.boot_id.to_le_bytes());
    match h.seq {
        Some(s) => {
            p[16] = 1;
            p[17..21].copy_from_slice(&s.to_le_bytes());
        }
        None => {
            p[16] = 0;
            p[17..21].copy_from_slice(&0u32.to_le_bytes());
        }
    }
    p[21..53].copy_from_slice(&h.hash);
    p[53..57].copy_from_slice(&h.overwritten.to_le_bytes());
    let sum = head_sum(&p[..HEAD_LEN - 4]);
    p[57..61].copy_from_slice(&sum.to_le_bytes());
    p
}

fn decode_head(b: &[u8]) -> Option<(JournalHead, u32)> {
    if b.len() < HEAD_LEN || b[0..4] != HEAD_MAGIC {
        return None;
    }
    let stored = u32::from_le_bytes([b[57], b[58], b[59], b[60]]);
    if stored != head_sum(&b[..HEAD_LEN - 4]) {
        return None; // torn write
    }
    let rec = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let mut boot = [0u8; 8];
    boot.copy_from_slice(&b[8..16]);
    let seq = if b[16] == 1 { Some(u32::from_le_bytes([b[17], b[18], b[19], b[20]])) } else { None };
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&b[21..53]);
    Some((
        JournalHead {
            boot_id: u64::from_le_bytes(boot),
            seq,
            hash,
            overwritten: u32::from_le_bytes([b[53], b[54], b[55], b[56]]),
        },
        rec,
    ))
}

/// The persisted journal. Holds only the next slot index and the head's record counter;
/// everything else is read back from flash.
pub struct JStore {
    next_slot: u32,
    rec: u32,
}

impl JStore {
    /// Read the head back and position the ring after the last entry it covers.
    pub fn open() -> (Self, Option<JournalHead>) {
        let mut best: Option<(JournalHead, u32)> = None;
        for s in HEAD_SECTORS {
            if let Some((h, rec)) = decode_head(read_at(s, HEAD_LEN)) {
                if best.as_ref().map_or(true, |(_, r)| rec > *r) {
                    best = Some((h, rec));
                }
            }
        }
        match best {
            Some((h, rec)) => {
                let next = match h.seq {
                    Some(s) => s.wrapping_add(1) % CAPACITY,
                    None => 0,
                };
                (JStore { next_slot: next, rec }, Some(h))
            }
            None => (JStore { next_slot: 0, rec: 0 }, None),
        }
    }

    fn slot_offset(slot: u32) -> u32 {
        let s = slot % CAPACITY;
        RING_SECTORS[(s / SLOTS_PER_SECTOR) as usize] + (s % SLOTS_PER_SECTOR) * PAGE
    }

    /// Program one entry. Erases the sector first when this is its first slot, which is
    /// how the ring reclaims space; returns how many entries that erase discarded.
    pub fn append_entry(&mut self, entry: &[u8; ENTRY_LEN]) -> u32 {
        let slot = self.next_slot % CAPACITY;
        let mut discarded = 0;
        if slot % SLOTS_PER_SECTOR == 0 {
            let sector = RING_SECTORS[(slot / SLOTS_PER_SECTOR) as usize];
            // Count what is about to be lost, for IEC 62443-4-2 CR 2.9.
            for i in 0..SLOTS_PER_SECTOR {
                if read_at(sector + i * PAGE, 4) == ENTRY_MAGIC {
                    discarded += 1;
                }
            }
            erase(sector);
        }
        let mut p = [0xFFu8; 256];
        p[0..4].copy_from_slice(&ENTRY_MAGIC);
        p[4..4 + ENTRY_LEN].copy_from_slice(entry);
        program(Self::slot_offset(slot), &p);
        self.next_slot = (slot + 1) % CAPACITY;
        discarded
    }

    /// Update the head, in the slot the previous record did not use.
    pub fn commit_head(&mut self, h: &JournalHead) {
        self.rec = self.rec.wrapping_add(1);
        let target = HEAD_SECTORS[(self.rec % 2) as usize];
        erase(target);
        program(target, &encode_head(h, self.rec));
    }

    /// Entries in ring order, oldest slot first, as `(slot, bytes)`.
    pub fn iter_entries(&self) -> impl Iterator<Item = (u32, [u8; ENTRY_LEN])> + '_ {
        (0..CAPACITY).filter_map(|slot| {
            let off = Self::slot_offset(slot);
            if read_at(off, 4) != ENTRY_MAGIC {
                return None;
            }
            let mut e = [0u8; ENTRY_LEN];
            e.copy_from_slice(read_at(off + 4, ENTRY_LEN));
            Some((slot, e))
        })
    }

    /// Test harness only: erase every journal sector (factory reset of the journal).
    pub fn wipe() {
        for s in RING_SECTORS {
            erase(s);
        }
        for s in HEAD_SECTORS {
            erase(s);
        }
    }

    /// Test harness only: flip one bit of a stored entry page, to show that a modified
    /// journal is detected. Defensive: it targets this node's own flash.
    pub fn tamper(slot: u32, byte: usize) -> bool {
        if byte >= ENTRY_LEN {
            return false;
        }
        let off = Self::slot_offset(slot);
        if read_at(off, 4) != ENTRY_MAGIC {
            return false;
        }
        let mut page = [0xFFu8; 256];
        page[..4].copy_from_slice(&ENTRY_MAGIC);
        let mut e = [0u8; ENTRY_LEN];
        e.copy_from_slice(read_at(off + 4, ENTRY_LEN));
        e[byte] ^= 0x01;
        page[4..4 + ENTRY_LEN].copy_from_slice(&e);
        // A page cannot be re-programmed in place: erase the sector, then rewrite every
        // slot of it, with the one byte changed.
        let sector_i = (slot % CAPACITY) / SLOTS_PER_SECTOR;
        let sector = RING_SECTORS[sector_i as usize];
        let mut saved = [[0u8; 256]; SLOTS_PER_SECTOR as usize];
        let mut present = [false; SLOTS_PER_SECTOR as usize];
        for i in 0..SLOTS_PER_SECTOR as usize {
            let o = sector + i as u32 * PAGE;
            if read_at(o, 4) == ENTRY_MAGIC {
                saved[i].copy_from_slice(read_at(o, 256));
                present[i] = true;
            }
        }
        let target_i = ((slot % CAPACITY) % SLOTS_PER_SECTOR) as usize;
        saved[target_i] = page;
        erase(sector);
        for i in 0..SLOTS_PER_SECTOR as usize {
            if present[i] {
                program(sector + i as u32 * PAGE, &saved[i]);
            }
        }
        true
    }
}
