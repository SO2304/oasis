//! OASIS — sender-side counter lease (v0B liveness fix).
//!
//! A v0B receiver persists the highest counter it accepted per origin. If the
//! *sender* reboots and restarts its counter at 1, every receiver that remembers
//! it refuses all its new messages as stale (found on silicon by T8, 2026-10-06).
//!
//! Fix: the sender persists a counter **ceiling** ahead of use, in blocks of
//! [`TX_LEASE_BLOCK`] counters, and never issues a counter above the durable
//! ceiling. On boot it resumes **at** the persisted ceiling, so the first counter
//! issued after any reboot is strictly greater than every counter issued before
//! it. Cost: one durable write per block (1/256 per message) and up to
//! `block - 1` counters skipped per reboot, which is harmless — receivers require
//! strict progress, not contiguity.
//!
//! Durability contract ([`CeilingStore`]): a write that does not complete must
//! leave a readable ceiling that is still `>=` every counter already issued.
//! [`DualSlotStore`] meets it on erase-then-program flash by alternating two
//! checksummed slots: a torn write damages only the slot being written, and the
//! other slot still holds the previous (valid) ceiling.
//!
//! `no_std`, no allocation, no crypto beyond the SHA-256 the crate already uses
//! (as a record checksum, not for security).

use sha2::{Digest, Sha256};

/// Counters reserved per durable ceiling write.
pub const TX_LEASE_BLOCK: u64 = 256;

/// Durable storage for the lease ceiling.
pub trait CeilingStore {
    /// The persisted ceiling, or `None` if none was ever written.
    fn load(&self) -> Option<u64>;
    /// Make `ceiling` durable. Return `false` if it may not be durable (the
    /// lease then refuses to issue counters above the previous ceiling).
    fn store(&mut self, ceiling: u64) -> bool;
}

/// Sender-side counter lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxLease {
    /// Last counter issued (or the resume point right after boot).
    last: u64,
    /// Highest counter covered by a durable ceiling.
    ceiling: u64,
    block: u64,
    /// Durable writes performed since boot (for the wear measurement).
    writes: u64,
}

impl TxLease {
    /// Resume after boot at the persisted ceiling (0 on a fresh device).
    pub fn boot<S: CeilingStore>(store: &S, block: u64) -> Self {
        let c = store.load().unwrap_or(0);
        TxLease { last: c, ceiling: c, block: block.max(1), writes: 0 }
    }

    /// The counter the sender must resume *after*: the next issued counter is
    /// `resume_point() + 1`.
    pub fn resume_point(&self) -> u64 {
        self.last
    }

    /// Highest counter currently covered by a durable ceiling.
    pub fn ceiling(&self) -> u64 {
        self.ceiling
    }

    /// Durable writes since boot.
    pub fn writes(&self) -> u64 {
        self.writes
    }

    /// Make sure `counter` is covered by a durable ceiling, persisting a new
    /// ceiling first if needed. Returns `false` (and changes nothing) if the
    /// ceiling cannot be made durable or would overflow; the caller must then
    /// NOT emit `counter`.
    pub fn ensure<S: CeilingStore>(&mut self, counter: u64, store: &mut S) -> bool {
        if counter <= self.ceiling {
            return true;
        }
        let new_ceiling = match counter.checked_add(self.block - 1) {
            Some(c) => c,
            None => return false,
        };
        if !store.store(new_ceiling) {
            return false;
        }
        self.ceiling = new_ceiling;
        self.writes += 1;
        true
    }

    /// Reserve and return the next counter, persisting a ceiling first when the
    /// current block is exhausted. `None` if it cannot be made durable.
    pub fn next<S: CeilingStore>(&mut self, store: &mut S) -> Option<u64> {
        let candidate = self.last.checked_add(1)?;
        if !self.ensure(candidate, store) {
            return None;
        }
        self.last = candidate;
        Some(candidate)
    }

    /// Record that a counter was issued by someone else (e.g. a router that owns
    /// its own `tx_counter`) — only after `ensure(counter)` returned `true`.
    pub fn note_issued(&mut self, counter: u64) {
        if counter > self.last {
            self.last = counter;
        }
    }
}

/// Raw access to two record slots (e.g. two flash sectors).
pub trait SlotIo {
    fn read(&self, slot: usize) -> [u8; TX_LEASE_RECORD_LEN];
    /// Write one slot. May be torn by a power loss; must not touch the other slot.
    fn write(&mut self, slot: usize, rec: &[u8; TX_LEASE_RECORD_LEN]) -> bool;
}

/// `magic(4) | ceiling u64 LE (8) | check (4)`.
pub const TX_LEASE_RECORD_LEN: usize = 16;
const TX_LEASE_MAGIC: [u8; 4] = *b"TXL1";

fn record_check(ceiling: u64) -> [u8; 4] {
    let mut h = Sha256::new();
    h.update(TX_LEASE_MAGIC);
    h.update(ceiling.to_le_bytes());
    let d = h.finalize();
    [d[0], d[1], d[2], d[3]]
}

/// Encode a ceiling record.
pub fn encode_record(ceiling: u64) -> [u8; TX_LEASE_RECORD_LEN] {
    let mut r = [0u8; TX_LEASE_RECORD_LEN];
    r[0..4].copy_from_slice(&TX_LEASE_MAGIC);
    r[4..12].copy_from_slice(&ceiling.to_le_bytes());
    r[12..16].copy_from_slice(&record_check(ceiling));
    r
}

/// Decode a ceiling record; `None` if absent, erased, torn or corrupted.
pub fn decode_record(r: &[u8; TX_LEASE_RECORD_LEN]) -> Option<u64> {
    if r[0..4] != TX_LEASE_MAGIC {
        return None;
    }
    let mut c = [0u8; 8];
    c.copy_from_slice(&r[4..12]);
    let ceiling = u64::from_le_bytes(c);
    if r[12..16] != record_check(ceiling) {
        return None;
    }
    Some(ceiling)
}

/// Two alternating checksummed slots: a torn write can only damage the slot
/// being written, so the other slot keeps the previous valid ceiling.
pub struct DualSlotStore<IO: SlotIo> {
    pub io: IO,
}

impl<IO: SlotIo> DualSlotStore<IO> {
    pub fn new(io: IO) -> Self {
        DualSlotStore { io }
    }
    fn slots(&self) -> [Option<u64>; 2] {
        [decode_record(&self.io.read(0)), decode_record(&self.io.read(1))]
    }
}

impl<IO: SlotIo> CeilingStore for DualSlotStore<IO> {
    fn load(&self) -> Option<u64> {
        match self.slots() {
            [Some(a), Some(b)] => Some(a.max(b)),
            [Some(a), None] => Some(a),
            [None, Some(b)] => Some(b),
            [None, None] => None,
        }
    }
    fn store(&mut self, ceiling: u64) -> bool {
        // Overwrite the slot that does NOT hold the current maximum.
        let target = match self.slots() {
            [Some(a), Some(b)] => {
                if a >= b {
                    1
                } else {
                    0
                }
            }
            [Some(_), None] => 1,
            [None, Some(_)] => 0,
            [None, None] => 0,
        };
        if !self.io.write(target, &encode_record(ceiling)) {
            return false;
        }
        // Read back: durable only if the slot now decodes to exactly `ceiling`.
        decode_record(&self.io.read(target)) == Some(ceiling)
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
