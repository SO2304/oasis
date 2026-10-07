//! OASIS — signed fragmentation `OFR1` for messages larger than one frame.
//!
//! Spec: `docs/specs/PQ_AUTHORITY_SPEC.md` §3. A hybrid authority message is
//! ~2.6 KB; a frame is <= 300 B (UART harness) or <= 255 B (LoRa). Each fragment
//! travels as the payload of its own v0B envelope, so every fragment is origin-
//! authenticated, fresh and network-bound at every hop. This module only
//! reassembles; the caller verifies the complete message before forwarding it.
//!
//! `"OFR1" | msg_id[8] | total_len u16 | idx u8 | count u8 | chunk`
//!
//! `msg_id` = first 8 bytes of SHA-256 over the whole message. Layout is fixed by
//! the header: stride = ceil(total_len / count); fragment `idx` carries bytes
//! `[idx*stride, min((idx+1)*stride, total_len))`, so overlaps are impossible.

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};
use sha2::{Digest, Sha256};

pub const OFR1_MAGIC: [u8; 4] = *b"OFR1";
pub const FRAG_HEADER_LEN: usize = 4 + 8 + 2 + 1 + 1;
/// Largest message that can be reassembled.
pub const MAX_ASSEMBLED: usize = 4096;
/// Most fragments per message (fits the u32 received-bitmap).
pub const MAX_FRAGMENTS: usize = 32;
/// Concurrent reassemblies (one per origin at most).
pub const REASSEMBLY_SLOTS: usize = 2;
/// An unfinished reassembly is dropped after this long.
pub const REASSEMBLY_TIMEOUT_MS: u64 = 30_000;

pub type Fp = [u8; 8];

/// First 8 bytes of SHA-256(message).
pub fn msg_id_of(msg: &[u8]) -> [u8; 8] {
    let d = Sha256::digest(msg);
    let mut id = [0u8; 8];
    id.copy_from_slice(&d[..8]);
    id
}

fn stride(total_len: usize, count: usize) -> usize {
    total_len.div_ceil(count)
}

/// Split `msg` into `OFR1` fragments of at most `max_fragment_len` bytes each
/// (header included). `None` if the message is too large or the frame too small.
pub fn fragment(msg: &[u8], max_fragment_len: usize) -> Option<Vec<Vec<u8>>> {
    if msg.is_empty() || msg.len() > MAX_ASSEMBLED || max_fragment_len <= FRAG_HEADER_LEN {
        return None;
    }
    let max_chunk = max_fragment_len - FRAG_HEADER_LEN;
    let count = msg.len().div_ceil(max_chunk);
    if count > MAX_FRAGMENTS {
        return None;
    }
    let s = stride(msg.len(), count);
    let id = msg_id_of(msg);
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let start = idx * s;
        let end = (start + s).min(msg.len());
        let mut f = Vec::with_capacity(FRAG_HEADER_LEN + end - start);
        f.extend_from_slice(&OFR1_MAGIC);
        f.extend_from_slice(&id);
        f.extend_from_slice(&(msg.len() as u16).to_le_bytes());
        f.push(idx as u8);
        f.push(count as u8);
        f.extend_from_slice(&msg[start..end]);
        out.push(f);
    }
    Some(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FragHeader {
    pub msg_id: [u8; 8],
    pub total_len: u16,
    pub idx: u8,
    pub count: u8,
}

/// Parse and self-check one fragment. Never panics.
pub fn parse_ofr1(b: &[u8]) -> Option<(FragHeader, &[u8])> {
    if b.len() <= FRAG_HEADER_LEN || b[0..4] != OFR1_MAGIC {
        return None;
    }
    let mut msg_id = [0u8; 8];
    msg_id.copy_from_slice(&b[4..12]);
    let total_len = u16::from_le_bytes([b[12], b[13]]);
    let (idx, count) = (b[14], b[15]);
    let (t, c, i) = (total_len as usize, count as usize, idx as usize);
    if c == 0 || c > MAX_FRAGMENTS || i >= c || t == 0 || t > MAX_ASSEMBLED {
        return None;
    }
    let s = stride(t, c);
    let start = i * s;
    if start >= t {
        return None; // this layout would give an empty fragment
    }
    let chunk = &b[FRAG_HEADER_LEN..];
    if chunk.len() != (start + s).min(t) - start {
        return None;
    }
    Some((FragHeader { msg_id, total_len, idx, count }, chunk))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragReject {
    /// Bad magic, inconsistent header or wrong chunk length.
    Malformed,
    /// No free reassembly slot.
    NoSlot,
    /// The assembled bytes do not hash to `msg_id`.
    HashMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragOutcome {
    /// Stored; more fragments needed.
    Incomplete,
    /// Already had this fragment.
    Duplicate,
    /// Message complete and its hash matches `msg_id`.
    Complete(Vec<u8>),
    Rejected(FragReject),
}

struct Slot {
    origin: Fp,
    hdr: FragHeader,
    received: u32,
    buf: Vec<u8>,
    started_ms: u64,
}

/// Bounded reassembler: `REASSEMBLY_SLOTS` messages at once, one per origin.
pub struct Reassembler {
    slots: [Option<Slot>; REASSEMBLY_SLOTS],
}

impl Default for Reassembler {
    fn default() -> Self {
        Reassembler { slots: [None, None] }
    }
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Slots currently in use.
    pub fn in_use(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Feed one fragment received from `origin` (the v0B origin of its envelope).
    pub fn push(&mut self, origin: Fp, now_ms: u64, frag: &[u8]) -> FragOutcome {
        for s in self.slots.iter_mut() {
            if s.as_ref().map_or(false, |x| now_ms.saturating_sub(x.started_ms) > REASSEMBLY_TIMEOUT_MS) {
                *s = None;
            }
        }
        let (hdr, chunk) = match parse_ofr1(frag) {
            Some(x) => x,
            None => return FragOutcome::Rejected(FragReject::Malformed),
        };
        // One message per origin: a different message from the same origin
        // replaces the unfinished one.
        let slot_i = match self.slots.iter().position(|s| s.as_ref().map_or(false, |x| x.origin == origin)) {
            Some(i) => {
                let same = self.slots[i].as_ref().map_or(false, |x| x.hdr.msg_id == hdr.msg_id);
                let consistent = self.slots[i].as_ref().map_or(false, |x| x.hdr.total_len == hdr.total_len && x.hdr.count == hdr.count);
                if same && !consistent {
                    self.slots[i] = None; // incoherent fragment cancels the slot
                    return FragOutcome::Rejected(FragReject::Malformed);
                }
                if !same {
                    self.slots[i] = None;
                }
                i
            }
            None => match self.slots.iter().position(|s| s.is_none()) {
                Some(i) => i,
                None => return FragOutcome::Rejected(FragReject::NoSlot),
            },
        };
        if self.slots[slot_i].is_none() {
            self.slots[slot_i] = Some(Slot { origin, hdr, received: 0, buf: vec![0u8; hdr.total_len as usize], started_ms: now_ms });
        }
        let slot = self.slots[slot_i].as_mut().unwrap();
        let bit = 1u32 << hdr.idx;
        if slot.received & bit != 0 {
            return FragOutcome::Duplicate;
        }
        let s = stride(hdr.total_len as usize, hdr.count as usize);
        let start = hdr.idx as usize * s;
        slot.buf[start..start + chunk.len()].copy_from_slice(chunk);
        slot.received |= bit;
        let all = if hdr.count as usize == 32 { u32::MAX } else { (1u32 << hdr.count) - 1 };
        if slot.received != all {
            return FragOutcome::Incomplete;
        }
        let done = self.slots[slot_i].take().unwrap();
        if msg_id_of(&done.buf) != done.hdr.msg_id {
            return FragOutcome::Rejected(FragReject::HashMismatch);
        }
        FragOutcome::Complete(done.buf)
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
