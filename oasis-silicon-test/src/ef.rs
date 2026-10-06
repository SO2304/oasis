//! Part C (E/F) on silicon: signed mesh revocation + actuation gate, wired onto
//! the uart_mesh test node. All decisions come from oasis-rt (`mesh_revocation`,
//! `actuation`) and oasis-operator-key — the same code the PC tests exercise.

use alloc::vec::Vec;
use oasis_operator_key::OperatorAuthority;
use oasis_rt::actuation::{command_within_limits, ActCommand, Actuator, Decision, GateInput};
use oasis_rt::hal::PhysicalConstraints;
use oasis_rt::hyper_state::{agent_new, inject_sensory, is_action_safe, Agent};
use oasis_rt::mesh::MeshRouter;
use oasis_rt::mesh_revocation::{
    load_latest_blob, parse_orv1, revocation_transition, signed_message, store_blob, BlobSlots, Fp, RevDecision,
    RevReject, RevState,
};
use rp2040_hal::pac;

/// Operator public key (the seed stays on the PC: oasis-operator-key/examples/ef_payloads.rs).
pub const OPERATOR_PUB: [u8; 32] = [
    0x0b, 0xee, 0xf5, 0xa9, 0xe6, 0x79, 0xe6, 0xa3, 0xe1, 0x34, 0xfe, 0x27, 0x83, 0x7b, 0xff, 0x32, 0xc7, 0xcb, 0x5f,
    0x5d, 0x44, 0xea, 0x09, 0xbc, 0xb0, 0xe5, 0x42, 0xba, 0xd6, 0xa4, 0xc0, 0xcc,
];
/// The actuator hosted by board C (on-board LED, GP25).
pub const ACTUATOR_ID: u16 = 1;
/// Only board A may command actuator 1 (provisioned authority table, spec F.3).
pub const ACTUATOR_AUTHORITY: Fp = [0xAA; 8];
pub const R14_THRESHOLD: f64 = 0.6;
pub const LED_PIN: u32 = 25;

/// Revocation blob: two alternating 4 KiB sectors below the lease sectors.
const REV_SECTORS: [u32; 2] = [0x1F_B000, 0x1F_C000];
const REV_SLOT_CAP: usize = 512;
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
        _ => "other",
    }
}

pub struct Ef {
    pub rev: RevState,
    auth: OperatorAuthority,
    pub act: Actuator,
    agent: Agent,
    pub sensor_lost: bool,
    pub boot_id: u64,
    pub last_entropy: f64,
}

impl Ef {
    pub fn new(boot_id: u64) -> Self {
        Ef {
            rev: RevState::default(),
            auth: OperatorAuthority::Single { pub_key: OPERATOR_PUB },
            act: Actuator::new(),
            agent: agent_new(3),
            sensor_lost: false,
            boot_id,
            last_entropy: 0.0,
        }
    }

    fn verified(&self, blob: &[u8]) -> Result<(RevDecision, Option<RevState>), RevReject> {
        let p = parse_orv1(blob)?;
        let sig_ok = self.auth.verify_authorization(&signed_message(&p), &p.sigs).is_ok();
        Ok(revocation_transition(&self.rev, &crate::NETWORK_ID, &p, sig_ok))
    }

    fn apply(&mut self, router: &mut MeshRouter, new: RevState) {
        for fp in &new.revoked {
            router.revoke(*fp);
        }
        self.rev = new;
    }

    /// Boot: restore the persisted list, re-verifying the operator signature,
    /// BEFORE any envelope is processed. Ok(None) = nothing persisted.
    pub fn restore(&mut self, router: &mut MeshRouter) -> Result<Option<u64>, RevReject> {
        let blob = match load_latest_blob(&RevFlash) {
            Some(b) => b,
            None => return Ok(None),
        };
        match self.verified(&blob)? {
            (RevDecision::Applied, Some(n)) => {
                let e = n.epoch;
                self.apply(router, n);
                Ok(Some(e))
            }
            (RevDecision::Reject(r), _) => Err(r),
            _ => Ok(None),
        }
    }

    /// Relay rule (spec E.3): verify, persist BEFORE applying, apply, and tell the
    /// caller whether to forward (only `Applied` is forwarded, once per epoch).
    pub fn ingest_orv1(&mut self, router: &mut MeshRouter, blob: &[u8]) -> RevDecision {
        match self.verified(blob) {
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

    /// Actuation gate (spec F.3). Drives the LED only on `Act`.
    pub fn decide(&mut self, router: &MeshRouter, origin: &Fp, v0b_ok: bool, cmd: &ActCommand) -> Decision {
        inject_sensory(&mut self.agent, if self.sensor_lost { 1.0 } else { 0.0 });
        self.last_entropy = self.agent.entropy;
        let input = GateInput {
            v0b_ok,
            authorized: cmd.actuator_id == ACTUATOR_ID && *origin == ACTUATOR_AUTHORITY,
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
