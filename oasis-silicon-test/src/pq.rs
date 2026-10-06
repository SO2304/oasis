//! Phase 1.1 on silicon: hybrid authority messages (`OAU1`, Ed25519 + ML-DSA-44)
//! carried as `OFR1` fragments, each in its own v0B envelope, on the uart_mesh node.
//! Every decision comes from oasis-rt (`authority`, `fragment`, `mesh_revocation`),
//! the code covered by the PC tests and the Kani harnesses.
//! Spec: docs/specs/PQ_AUTHORITY_SPEC.md §3 (fragmentation, store-and-forward), §6.

use alloc::vec::Vec;
use oasis_rt::authority::{
    kind, parse_policy_update, verify_authority, AuthPolicy, AuthReject, AuthorityKeys,
    MLDSA44_PK_LEN, POLICY_RECORD_LEN,
};
use oasis_rt::fragment::{FragOutcome, Reassembler};
use oasis_rt::mesh::{MeshRouter, MESH_V0B_HEADER_LEN};
use oasis_rt::mesh_revocation::RevDecision;

/// ML-DSA-44 operator public key. The seed stays on the PC
/// (`oasis-operator-key/examples/pq_payloads.rs`).
pub static OPERATOR_MLDSA_PK: &[u8; MLDSA44_PK_LEN] = include_bytes!("pq/op_mldsa44.pk");

pub fn keys() -> AuthorityKeys<'static> {
    AuthorityKeys {
        ed25519: crate::ef::OPERATOR_PUB,
        mldsa44: OPERATOR_MLDSA_PK,
    }
}

/// Largest `OFR1` fragment whose v0B envelope still fits one UART frame.
pub const FRAG_MAX: usize = crate::MAX_ENV - MESH_V0B_HEADER_LEN;

// ── policy persistence: two 4 KiB sectors below the revocation sectors ─────────
const POLICY_SECTORS: [u32; 2] = [0x1F_9000, 0x1F_A000];

fn read_policy_slot(slot: usize) -> [u8; POLICY_RECORD_LEN] {
    let p = (0x1000_0000usize + POLICY_SECTORS[slot] as usize) as *const u8;
    let mut r = [0u8; POLICY_RECORD_LEN];
    for (i, b) in r.iter_mut().enumerate() {
        *b = unsafe { core::ptr::read_volatile(p.add(i)) };
    }
    r
}

fn write_policy_slot(slot: usize, rec: &[u8; POLICY_RECORD_LEN]) {
    let mut page = [0xFFu8; 256];
    page[..POLICY_RECORD_LEN].copy_from_slice(rec);
    cortex_m::interrupt::free(|_| unsafe {
        rp2040_flash::flash::flash_range_erase(POLICY_SECTORS[slot], 4096, true);
        rp2040_flash::flash::flash_range_program(POLICY_SECTORS[slot], &page, true);
    });
}

/// Policy at boot: the default raised by every valid slot (`AuthPolicy::from_slots`).
pub fn load_policy() -> AuthPolicy {
    AuthPolicy::from_slots(&read_policy_slot(0), &read_policy_slot(1))
}

/// Write the record to slot 0 then slot 1, each read back. A power cut tears at
/// most one slot; the other still holds the previous (never weaker) policy.
fn persist_policy(p: &AuthPolicy) -> bool {
    let r = p.to_bytes();
    for slot in 0..2 {
        write_policy_slot(slot, &r);
        if read_policy_slot(slot) != r {
            return false;
        }
    }
    true
}

/// Test harness: erase both policy sectors.
pub fn wipe_policy_sectors() {
    cortex_m::interrupt::free(|_| unsafe {
        for s in POLICY_SECTORS {
            rp2040_flash::flash::flash_range_erase(s, 4096, true);
        }
    });
}

// ── measurement: the same stack painting as pq_bench ─────────────────────────
const PAINT: u32 = 0xA5C3_5A3C;
extern "C" {
    static _stack_end: u32;
}

/// Run `f`, returning its result and the peak stack it used below the caller's
/// stack pointer (painted free stack, lowest overwritten word). USB is polled, so
/// no interrupt shares the stack. `usize::MAX` = the paint floor was reached.
#[inline(never)]
fn with_stack_peak<R, F: FnOnce() -> R>(f: F) -> (R, usize) {
    let floor = (core::ptr::addr_of!(_stack_end) as usize + 64) & !3;
    let sp = cortex_m::register::msp::read() as usize;
    let top = (sp - 128) & !3;
    let mut a = floor;
    while a < top {
        unsafe { core::ptr::write_volatile(a as *mut u32, PAINT) };
        a += 4;
    }
    let r = f();
    let mut a = floor;
    while a < top && unsafe { core::ptr::read_volatile(a as *const u32) } == PAINT {
        a += 4;
    }
    (r, if a == floor { usize::MAX } else { sp - a })
}

// ── node state ──────────────────────────────────────────────────────────────

/// What a complete authority message led to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    /// A revocation went through the existing ORV1 rules (persist before apply).
    Revocation(RevDecision),
    /// A policy raise; `changed` only if it strengthened the policy.
    Policy {
        changed: bool,
        persisted: bool,
    },
    /// Verified, but this kind is not handled yet on the node (Phase 1.2/1.3).
    Unsupported(u8),
    Rejected(AuthReject),
}

impl Done {
    /// Store-and-forward rule: re-originate only what changed this node's state.
    pub fn forward(&self) -> bool {
        matches!(
            self,
            Done::Revocation(RevDecision::Applied)
                | Done::Policy {
                    changed: true,
                    persisted: true
                }
        )
    }
}

pub struct Gate {
    pub kind: u8,
    pub suite: u8,
    pub verify_us: u32,
    pub stack: usize,
}

pub struct Pq {
    pub policy: AuthPolicy,
    pub reasm: Reassembler,
    /// Message uploaded by the PC (`@Q<hex>`), fragmented and sent by `@F` / `@T`.
    pub stage: Vec<u8>,
}

impl Pq {
    pub fn boot() -> Self {
        Pq {
            policy: load_policy(),
            reasm: Reassembler::new(),
            stage: Vec::new(),
        }
    }

    /// Feed one `OFR1` fragment from v0B origin `origin`.
    pub fn push_fragment(&mut self, origin: [u8; 8], frag: &[u8]) -> FragOutcome {
        self.reasm.push(origin, crate::ef::now_ms64(), frag)
    }

    /// A reassembled message: full authority check under the live policy (timed and
    /// stack-measured), then dispatch by kind.
    pub fn complete(
        &mut self,
        efs: &mut crate::ef::Ef,
        router: &mut MeshRouter,
        msg: &[u8],
    ) -> (Done, Gate) {
        let t0 = crate::now_us();
        let (res, stack) =
            with_stack_peak(|| verify_authority(&self.policy, &crate::NETWORK_ID, &keys(), msg));
        let verify_us = crate::now_us().wrapping_sub(t0);
        let (k, s) = (
            msg.get(4).copied().unwrap_or(0),
            msg.get(5).copied().unwrap_or(0),
        );
        let gate = Gate {
            kind: k,
            suite: s,
            verify_us,
            stack,
        };
        let p = match res {
            Ok(p) => p,
            Err(r) => return (Done::Rejected(r), gate),
        };
        let done = match p.kind {
            kind::REVOCATION => {
                Done::Revocation(efs.ingest_oau1_revocation(router, msg, p.content))
            }
            kind::POLICY => match parse_policy_update(p.content) {
                Some((target, suite)) => {
                    let mut next = self.policy;
                    if next.raise(target, suite) {
                        // Persist before applying, like a revocation.
                        let persisted = persist_policy(&next);
                        if persisted {
                            self.policy = next;
                        }
                        Done::Policy {
                            changed: true,
                            persisted,
                        }
                    } else {
                        Done::Policy {
                            changed: false,
                            persisted: true,
                        }
                    }
                }
                None => Done::Rejected(AuthReject::Malformed),
            },
            other => Done::Unsupported(other),
        };
        (done, gate)
    }
}
