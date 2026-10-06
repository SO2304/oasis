//! Phase 1.1/1.2 on silicon: authority messages (`OAU1`, Ed25519 + ML-DSA-44) carried
//! as `OFR1` fragments, each in its own v0B envelope, on the uart_mesh node: revocation
//! and policy (1.1), enrollment and ownership transfer (1.2). Every decision comes
//! from oasis-rt (`authority`, `fragment`, `mesh_revocation`, `enrollment`,
//! `ownership`), the code covered by the PC tests and the Kani harnesses.
//! Specs: docs/specs/PQ_AUTHORITY_SPEC.md, docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md.

use alloc::vec::Vec;
use oasis_rt::authority::{
    kind, parse_oau1, parse_policy_update, verify_authority, AuthPolicy, AuthReject,
    MLDSA44_PK_LEN, POLICY_RECORD_LEN,
};
use oasis_rt::enrollment::{
    enrollment_transition, parse_attestation, EnrollDecision, EnrollReject, Registry,
};
use oasis_rt::fragment::{FragOutcome, Reassembler};
use oasis_rt::mesh::{MeshEdPub, MeshRouter, MESH_V0B_HEADER_LEN};
use oasis_rt::mesh_revocation::RevDecision;
use oasis_rt::ownership::{
    accept_decision, apply_transfer, content_sub, offer_decision, offer_digest, parse_accept,
    parse_offer, OwnDecision, OwnReject, OwnerKeys, OwnerState, Pending, SUB_ACCEPT, SUB_OFFER,
};

/// ML-DSA-44 public key of owner #1 (the seed stays on the PC:
/// `oasis-operator-key/examples/pq_payloads.rs`, `oasis_enroll.rs`).
pub static OPERATOR_MLDSA_PK: &[u8; MLDSA44_PK_LEN] = include_bytes!("pq/op_mldsa44.pk");

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

/// Which owner signed the persisted revocation list (spec §4, `NeedsResign`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevSigner {
    None,
    Current,
    Previous,
}

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
    Enroll(EnrollDecision),
    Own(OwnDecision),
    /// Verified, but this kind is not handled yet on the node (Phase 1.3).
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
                | Done::Enroll(EnrollDecision::Enrolled | EnrollDecision::Updated)
                | Done::Own(OwnDecision::OfferPending | OwnDecision::Transferred)
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
    /// Message uploaded by the PC (`@Q<hex>`), sent by `@F` / `@T`, loaded by `@L`.
    pub stage: Vec<u8>,
    pub owner: OwnerState,
    /// An offer verified with the current owner's keys, waiting for its acceptance
    /// (RAM only: after a reboot the offer is simply sent again).
    pub pending: Option<Pending>,
    pub registry: Registry,
    pub rev_signer: RevSigner,
}

impl Pq {
    pub fn boot() -> Self {
        Pq {
            policy: load_policy(),
            reasm: Reassembler::new(),
            stage: Vec::new(),
            owner: crate::enroll::load_owner(),
            pending: None,
            registry: crate::enroll::load_registry(),
            rev_signer: RevSigner::None,
        }
    }

    /// The v0B registry is filled only from enrollment (no compiled keys).
    pub fn install_registry(&self, router: &mut MeshRouter) {
        for e in &self.registry.entries {
            router.ed_registry_insert(e.fp, MeshEdPub(e.pk));
        }
    }

    /// Feed one `OFR1` fragment from v0B origin `origin`.
    pub fn push_fragment(&mut self, origin: [u8; 8], frag: &[u8]) -> FragOutcome {
        self.reasm.push(origin, crate::ef::now_ms64(), frag)
    }

    /// A complete message (reassembled, or loaded over USB with `@L`): full authority
    /// check under the live policy (timed and stack-measured), then dispatch by kind.
    /// An ownership ACCEPTANCE is verified with the keys of the pending offer; every
    /// other message with the current owner's keys.
    pub fn complete(
        &mut self,
        efs: &mut crate::ef::Ef,
        router: &mut MeshRouter,
        msg: &[u8],
    ) -> (Done, Gate) {
        let (k, s) = (
            msg.get(4).copied().unwrap_or(0),
            msg.get(5).copied().unwrap_or(0),
        );
        let is_accept = k == kind::OWNERSHIP_TRANSFER
            && parse_oau1(msg).ok().and_then(|p| content_sub(p.content)) == Some(SUB_ACCEPT);
        let signer: Option<&OwnerKeys> = if is_accept {
            self.pending.as_ref().map(|p| &p.new_owner)
        } else {
            Some(&self.owner.current)
        };
        let signer = match signer {
            Some(o) => o.keys(),
            None => {
                let g = Gate {
                    kind: k,
                    suite: s,
                    verify_us: 0,
                    stack: 0,
                };
                return (Done::Own(OwnDecision::Reject(OwnReject::NoPendingOffer)), g);
            }
        };
        let t0 = crate::now_us();
        let (res, stack) =
            with_stack_peak(|| verify_authority(&self.policy, &crate::NETWORK_ID, &signer, msg));
        let gate = Gate {
            kind: k,
            suite: s,
            verify_us: crate::now_us().wrapping_sub(t0),
            stack,
        };
        let p = match res {
            Ok(p) => p,
            Err(r) => return (Done::Rejected(r), gate),
        };
        let done = match p.kind {
            kind::REVOCATION => {
                let d = efs.ingest_oau1_revocation(router, msg, p.content);
                if d == RevDecision::Applied {
                    self.rev_signer = RevSigner::Current;
                }
                Done::Revocation(d)
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
            kind::ENROLLMENT => Done::Enroll(self.enroll(router, p.content)),
            kind::OWNERSHIP_TRANSFER => Done::Own(self.transfer(msg, p.content)),
            other => Done::Unsupported(other),
        };
        (done, gate)
    }

    /// Verified attestation: same rules as the PC tests; persist BEFORE applying.
    fn enroll(&mut self, router: &mut MeshRouter, content: &[u8]) -> EnrollDecision {
        let a = match parse_attestation(content) {
            Ok(a) => a,
            Err(r) => return EnrollDecision::Reject(r),
        };
        let fp = a.fp();
        let (d, new) = enrollment_transition(&self.registry, &fp, &a, router.is_revoked(&fp));
        if let Some(n) = new {
            if !crate::enroll::persist_registry(&n) {
                return EnrollDecision::Reject(EnrollReject::PersistFailed);
            }
            router.ed_registry_insert(fp, MeshEdPub(a.node_pk));
            self.registry = n;
        }
        d
    }

    /// Verified kind-4 message (offer: current owner's keys; acceptance: offered keys).
    fn transfer(&mut self, msg: &[u8], content: &[u8]) -> OwnDecision {
        match content_sub(content) {
            Some(SUB_OFFER) => {
                let offer = match parse_offer(content) {
                    Some(o) => o,
                    None => return OwnDecision::Reject(OwnReject::Malformed),
                };
                let digest = offer_digest(msg);
                let d = offer_decision(
                    self.owner.seq,
                    self.pending.as_ref().map(|p| &p.digest),
                    offer.seq,
                    &digest,
                    self.rev_signer != RevSigner::Previous,
                );
                if d == OwnDecision::OfferPending {
                    self.pending = Some(Pending {
                        seq: offer.seq,
                        new_owner: offer.new_owner,
                        digest,
                    });
                }
                d
            }
            Some(SUB_ACCEPT) => {
                let a = match parse_accept(content) {
                    Some(a) => a,
                    None => return OwnDecision::Reject(OwnReject::Malformed),
                };
                // Reaching here means verify_authority passed with the pending keys.
                let d = accept_decision(
                    self.owner.seq,
                    self.pending.as_ref().map(|p| (p.seq, &p.digest)),
                    &a,
                    true,
                );
                if d != OwnDecision::Transferred {
                    return d;
                }
                let new = apply_transfer(&self.owner, self.pending.as_ref().unwrap());
                if !crate::enroll::persist_owner(&new) {
                    return OwnDecision::Reject(OwnReject::PersistFailed);
                }
                self.owner = new;
                self.pending = None;
                if self.rev_signer == RevSigner::Current {
                    self.rev_signer = RevSigner::Previous;
                }
                OwnDecision::Transferred
            }
            _ => OwnDecision::Reject(OwnReject::Malformed),
        }
    }
}
