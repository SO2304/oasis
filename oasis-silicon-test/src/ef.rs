//! Part C (E/F) on silicon: signed mesh revocation + actuation gate, wired onto
//! the uart_mesh test node. All decisions come from oasis-rt (`mesh_revocation`,
//! `actuation`) and oasis-operator-key — the same code the PC tests exercise.

use alloc::vec::Vec;
use oasis_operator_key::OperatorAuthority;
use crate::jstore::JStore;
use oasis_rt::actuation::{
    command_within_limits, ActCommand, Actuator, Decision, GateInput, OrderClass, StopDecision, StopInput,
    SupervisionBeacon,
};
use oasis_rt::journal::{Journal, LoggedDecision, FLAG_R14_SAFE, FLAG_STOPPED, FLAG_SUPERVISION_LIVE, FLAG_WITHIN_LIMITS};
use oasis_rt::authority::{kind, verify_authority, AuthPolicy};
use oasis_rt::enrollment::{perm, Registry};
use oasis_rt::ownership::{OwnerKeys, OwnerState};
use oasis_rt::hal::PhysicalConstraints;
use oasis_rt::hyper_state::{agent_new, inject_sensory, is_action_safe, Agent};
use oasis_rt::mesh::MeshRouter;
use oasis_rt::mesh_revocation::{
    load_latest_blob, parse_orv1, parse_revocation_body, revocation_transition, signed_message, store_blob, BlobSlots,
    Fp, RevDecision, RevReject, RevState,
};
use rp2040_hal::pac;

/// Ed25519 public key of owner #1, the initial owner (the seed stays on the PC:
/// oasis-operator-key/examples/ef_payloads.rs). After an ownership transfer, revocation
/// lists are verified with the CURRENT owner's keys (Phase 1.2).
pub const OPERATOR_PUB: [u8; 32] = [
    0x0b, 0xee, 0xf5, 0xa9, 0xe6, 0x79, 0xe6, 0xa3, 0xe1, 0x34, 0xfe, 0x27, 0x83, 0x7b, 0xff, 0x32, 0xc7, 0xcb, 0x5f,
    0x5d, 0x44, 0xea, 0x09, 0xbc, 0xb0, 0xe5, 0x42, 0xba, 0xd6, 0xa4, 0xc0, 0xcc,
];
/// The actuator hosted by board C (on-board LED, GP25).
pub const ACTUATOR_ID: u16 = 1;
pub const R14_THRESHOLD: f64 = 0.6;
pub const LED_PIN: u32 = 25;

/// Revocation blob: two alternating 4 KiB sectors below the lease sectors. The slot
/// is the whole sector since Phase 1.1: a hybrid OAU1 revocation is ~2.5 KB (an
/// ORV1 record keeps the same layout; only the read length grew from 512 B).
const REV_SECTORS: [u32; 2] = [0x1F_B000, 0x1F_C000];
const REV_SLOT_CAP: usize = 4096;
pub struct RevFlash;
impl BlobSlots for RevFlash {
    fn cap(&self) -> usize {
        REV_SLOT_CAP
    }
    fn read(&self, slot: usize) -> Vec<u8> {
        let p = (0x1000_0000usize + REV_SECTORS[slot] as usize) as *const u8;
        (0..REV_SLOT_CAP).map(|i| unsafe { core::ptr::read_volatile(p.add(i)) }).collect()
    }
    fn write(&mut self, slot: usize, rec: &[u8]) -> bool {
        let mut buf = [0xFFu8; REV_SLOT_CAP]; // multiple of the 256-byte program page
        buf[..rec.len()].copy_from_slice(rec);
        cortex_m::interrupt::free(|_| unsafe {
            rp2040_flash::flash::flash_range_erase(REV_SECTORS[slot], 4096, true);
            rp2040_flash::flash::flash_range_program(REV_SECTORS[slot], &buf, true);
        });
        true // durability is confirmed by store_blob's read-back
    }
}

/// Test harness: erase both revocation sectors.
pub fn wipe_rev_sectors() {
    cortex_m::interrupt::free(|_| unsafe {
        for s in REV_SECTORS {
            rp2040_flash::flash::flash_range_erase(s, 4096, true);
        }
    });
}

/// 64-bit monotonic microseconds since boot (RP2040 1 MHz TIMER).
pub fn now_us64() -> u64 {
    let t = unsafe { &*pac::TIMER::ptr() };
    loop {
        let hi = t.timerawh().read().bits();
        let lo = t.timerawl().read().bits();
        if t.timerawh().read().bits() == hi {
            return ((hi as u64) << 32) | lo as u64;
        }
    }
}

/// 64-bit monotonic milliseconds since boot (RP2040 1 MHz TIMER).
pub fn now_ms64() -> u64 {
    let t = unsafe { &*pac::TIMER::ptr() };
    loop {
        let hi = t.timerawh().read().bits();
        let lo = t.timerawl().read().bits();
        if t.timerawh().read().bits() == hi {
            return (((hi as u64) << 32) | lo as u64) / 1000;
        }
    }
}

pub fn led_set(on: bool) {
    let sio = unsafe { &*pac::SIO::ptr() };
    if on {
        sio.gpio_out_set().write(|w| unsafe { w.bits(1 << LED_PIN) });
    } else {
        sio.gpio_out_clr().write(|w| unsafe { w.bits(1 << LED_PIN) });
    }
}

/// Level actually present on the pad (input register), not the commanded value.
pub fn led_pin_level() -> u32 {
    let sio = unsafe { &*pac::SIO::ptr() };
    (sio.gpio_in().read().bits() >> LED_PIN) & 1
}

/// Decode lowercase/uppercase hex into a buffer; None on odd length / bad digit.
pub fn hex_decode(s: &[u8], out: &mut [u8]) -> Option<usize> {
    if s.len() % 2 != 0 || s.len() / 2 > out.len() {
        return None;
    }
    let d = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    for i in 0..s.len() / 2 {
        out[i] = d(s[2 * i])? << 4 | d(s[2 * i + 1])?;
    }
    Some(s.len() / 2)
}

pub fn content_kind(payload: &[u8]) -> &'static str {
    match payload.get(0..4) {
        Some(b"ORV1") => "ORV1",
        Some(b"OAC1") => "OAC1",
        Some(b"OEP1") => "OEP1",
        Some(b"OFR1") => "OFR1",
        Some(b"OAU1") => "OAU1",
        Some(b"OMB1") => "OMB1",
        _ => "other",
    }
}

pub struct Ef {
    pub rev: RevState,
    pub act: Actuator,
    agent: Agent,
    pub sensor_lost: bool,
    pub boot_id: u64,
    pub last_entropy: f64,
    /// Part I: the hash chain. `None` until [`Ef::open_journal`] has read flash back.
    pub journal: Option<Journal>,
    pub jstore: Option<JStore>,
    /// Entries written but whose head commit has not been attempted (power-cut window).
    pub j_pending: u32,
}

impl Ef {
    pub fn new(boot_id: u64) -> Self {
        Ef {
            rev: RevState::default(),
            // Part H: this actuator is a FIXED one (an LED, and the Modbus device on A),
            // so supervision is NOT required — `Actuator::new()`, not `new_supervised()`.
            // `@A1` switches it on at run time for the part H tests. Annex III part 3
            // applies to autonomous mobile machinery, which this board is not pretending
            // to be.
            act: Actuator::new(),
            agent: agent_new(3),
            sensor_lost: false,
            boot_id,
            last_entropy: 0.0,
            journal: None,
            jstore: None,
            j_pending: 0,
        }
    }

    /// Read the journal head back from flash. A head from an **earlier boot** starts a
    /// fresh chain for this boot (the chain is boot-bound by `genesis`), but the stored
    /// entries are left in place so the previous boot's journal can still be read out.
    pub fn open_journal(&mut self) -> (bool, Option<u64>) {
        let (store, head) = JStore::open();
        let prev_boot = head.as_ref().map(|h| h.boot_id);
        let restored = match head {
            Some(h) if h.boot_id == self.boot_id => {
                self.journal = Some(Journal { head: h });
                true
            }
            _ => {
                let j = Journal::new(self.boot_id);
                self.journal = Some(j);
                false
            }
        };
        self.jstore = Some(store);
        (restored, prev_boot)
    }

    /// Append one decision and persist it: entry page first, then the head. A power cut
    /// between the two leaves exactly one unconfirmed entry, which `verify` reports.
    pub fn log_decision(
        &mut self,
        origin: &Fp,
        cmd_seq: u32,
        class: OrderClass,
        decision: LoggedDecision,
        flags: u8,
    ) -> Option<u32> {
        let j = self.journal.as_mut()?;
        let store = self.jstore.as_mut()?;
        let bytes = j.append(*origin, cmd_seq, class, decision, flags);
        let seq = j.head.seq;
        let discarded = store.append_entry(&bytes);
        j.head.overwritten = j.head.overwritten.saturating_add(discarded);
        self.j_pending += 1;
        store.commit_head(&j.head);
        self.j_pending -= 1;
        seq
    }

    /// Flags recorded with a decision, so a reader can see the context the gate saw.
    pub fn flags_now(&self, r14_safe: bool, within_limits: bool, now_ms: u64) -> u8 {
        let ctx = self.act.context(now_ms);
        let mut f = 0;
        if r14_safe {
            f |= FLAG_R14_SAFE;
        }
        if within_limits {
            f |= FLAG_WITHIN_LIMITS;
        }
        if !ctx.supervision_expired {
            f |= FLAG_SUPERVISION_LIVE;
        }
        if ctx.stopped {
            f |= FLAG_STOPPED;
        }
        f
    }

    /// Part G: decide on a **stop** order. Three conditions only — see
    /// `oasis_rt::actuation::stop_decision`. On `Stop` the LED is driven to its safe
    /// state (off) and the latch is set.
    pub fn decide_stop(&mut self, router: &MeshRouter, registry: &Registry, origin: &Fp, v0b_ok: bool) -> StopDecision {
        let d = self.act.decide_stop(&StopInput {
            v0b_ok,
            // `STOP` is a permission of its own. No board has it until an owner-signed
            // attestation grants it, exactly like `ACTUATE`.
            stop_authorized: registry.allows(origin, perm::STOP),
            revoked: router.is_revoked(origin),
        });
        if d == StopDecision::Stop {
            led_set(false);
        }
        d
    }

    /// Part H: apply a supervision beacon the caller has already authenticated.
    pub fn apply_beacon(&mut self, b: &SupervisionBeacon, now_ms: u64) -> bool {
        if b.actuator_boot_id != self.boot_id {
            return false;
        }
        self.act.apply_beacon(b, now_ms)
    }

    /// Verify a revocation blob (ORV1 or OAU1) against one owner's keys.
    fn verified(&self, blob: &[u8], owner: &OwnerKeys) -> Result<(RevDecision, Option<RevState>), RevReject> {
        if blob.get(0..4) == Some(&b"OAU1"[..]) {
            // Boot restore of a persisted OAU1 revocation: signatures re-verified under
            // the DEFAULT policy. It was accepted under the policy in force at the time;
            // re-applying a policy raised since would drop a valid list at every boot.
            let a = verify_authority(&AuthPolicy::default(), &crate::NETWORK_ID, &owner.keys(), blob)
                .map_err(|_| RevReject::BadOperatorSig)?;
            if a.kind != kind::REVOCATION {
                return Err(RevReject::Malformed);
            }
            let p = parse_revocation_body(a.network_id, a.content)?;
            return Ok(revocation_transition(&self.rev, &crate::NETWORK_ID, &p, true));
        }
        let p = parse_orv1(blob)?;
        let auth = OperatorAuthority::Single { pub_key: owner.ed25519 };
        let sig_ok = auth.verify_authorization(&signed_message(&p), &p.sigs).is_ok();
        Ok(revocation_transition(&self.rev, &crate::NETWORK_ID, &p, sig_ok))
    }

    fn apply(&mut self, router: &mut MeshRouter, new: RevState) {
        for fp in &new.revoked {
            router.revoke(*fp);
        }
        self.rev = new;
    }

    /// Boot: restore the persisted list, re-verifying its signature with the current
    /// owner, else the previous one (one level, spec 1.2 §4), BEFORE any envelope is
    /// processed. Ok(None) = nothing persisted; Ok(Some((epoch, by_current))).
    pub fn restore(&mut self, router: &mut MeshRouter, owner: &OwnerState) -> Result<Option<(u64, bool)>, RevReject> {
        let blob = match load_latest_blob(&RevFlash) {
            Some(b) => b,
            None => return Ok(None),
        };
        let mut tries = [(Some(&owner.current), true), (owner.previous.as_ref(), false)];
        let mut last = RevReject::BadOperatorSig;
        for (keys, by_current) in tries.iter_mut() {
            let k = match keys {
                Some(k) => *k,
                None => continue,
            };
            match self.verified(&blob, k) {
                Ok((RevDecision::Applied, Some(n))) => {
                    let e = n.epoch;
                    self.apply(router, n);
                    return Ok(Some((e, *by_current)));
                }
                Ok((RevDecision::Reject(r), _)) | Err(r) => last = r,
                _ => return Ok(None),
            }
        }
        Err(last)
    }

    /// Relay rule (spec E.3): verify, persist BEFORE applying, apply, and tell the
    /// caller whether to forward (only `Applied` is forwarded, once per epoch).
    pub fn ingest_orv1(&mut self, router: &mut MeshRouter, blob: &[u8], owner: &OwnerKeys) -> RevDecision {
        match self.verified(blob, owner) {
            Err(r) => RevDecision::Reject(r),
            Ok((RevDecision::Applied, Some(new))) => {
                if !store_blob(&mut RevFlash, blob) {
                    return RevDecision::Reject(RevReject::PersistFailed);
                }
                self.apply(router, new);
                RevDecision::Applied
            }
            Ok((d, _)) => d,
        }
    }

    /// OAU1 revocation whose signatures `verify_authority` already accepted under the
    /// live policy: same rule as ORV1 (persist BEFORE applying; only `Applied` is
    /// forwarded, by re-origination).
    pub fn ingest_oau1_revocation(&mut self, router: &mut MeshRouter, blob: &[u8], content: &[u8]) -> RevDecision {
        let p = match parse_revocation_body(crate::NETWORK_ID, content) {
            Ok(p) => p,
            Err(r) => return RevDecision::Reject(r),
        };
        match revocation_transition(&self.rev, &crate::NETWORK_ID, &p, true) {
            (RevDecision::Applied, Some(new)) => {
                if !store_blob(&mut RevFlash, blob) {
                    return RevDecision::Reject(RevReject::PersistFailed);
                }
                self.apply(router, new);
                RevDecision::Applied
            }
            (d, _) => d,
        }
    }

    /// Actuation gate (spec F.3). Drives the LED only on `Act`.
    /// "Authorized" = the origin is enrolled with the `ACTUATE` permission (Phase 1.2;
    /// it was board A's compiled fingerprint before).
    /// R14 now, evaluated exactly as for an `OAC1` command (Phase 1.4 gateway).
    pub fn r14_safe_now(&mut self) -> bool {
        inject_sensory(&mut self.agent, if self.sensor_lost { 1.0 } else { 0.0 });
        self.last_entropy = self.agent.entropy;
        is_action_safe(&self.agent, R14_THRESHOLD)
    }

    pub fn decide(&mut self, router: &MeshRouter, registry: &Registry, origin: &Fp, v0b_ok: bool, cmd: &ActCommand) -> Decision {
        inject_sensory(&mut self.agent, if self.sensor_lost { 1.0 } else { 0.0 });
        self.last_entropy = self.agent.entropy;
        let input = GateInput {
            v0b_ok,
            authorized: cmd.actuator_id == ACTUATOR_ID && registry.allows(origin, perm::ACTUATE),
            revoked: router.is_revoked(origin),
            cmd_boot_id: cmd.boot_id,
            deadline_ms: cmd.deadline_ms,
            actuator_boot_id: self.boot_id,
            now_ms: now_ms64(),
            r14_safe: is_action_safe(&self.agent, R14_THRESHOLD),
            within_limits: command_within_limits(cmd, &PhysicalConstraints::default_robot()),
            cmd_seq: cmd.cmd_seq,
            last_executed_seq: None, // overridden by Actuator state
        };
        let d = self.act.decide(input);
        if d == Decision::Act {
            led_set(true);
        }
        d
    }
}
