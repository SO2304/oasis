//! OASIS — `OAC1` order and `OTM1` time-beacon wire format (child module of `actuation`).
//!
//! Split out of `actuation.rs` when part G pushed it past the 400-line rule (R10). The
//! rule itself — the gate, the stop asymmetry, the latch — stays in the parent.
//!
//! Everything here is re-exported by `actuation`, so `actuation::encode_oac1` and the
//! other existing paths are unchanged.

use super::{OAC1_CLASS_OFF, OAC1_LEN, OAC1_MAGIC, OTM1_LEN, OTM1_MAGIC};

/// What an order asks for. Signed as part of the `OAC1` payload (byte 50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderClass {
    /// Act: move, write a register. All 9 conditions apply.
    #[default]
    Act = 0,
    /// Go to the safe direction. Only 3 conditions apply ([`stop_decision`]).
    Stop = 1,
}

impl OrderClass {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(OrderClass::Act),
            1 => Some(OrderClass::Stop),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActCommand {
    pub actuator_id: u16,
    pub cmd_seq: u32,
    pub boot_id: u64,
    pub deadline_ms: u64,
    pub force: f32,
    pub torque: f32,
    pub velocity: f32,
    pub pos: [f32; 3],
}

/// Encode an `Act` order. Byte 50 is left at 0, so the bytes are identical to what
/// this function produced before part G.
pub fn encode_oac1(c: &ActCommand) -> [u8; OAC1_LEN] {
    encode_oac1_with_class(c, OrderClass::Act)
}

pub fn encode_oac1_with_class(c: &ActCommand, class: OrderClass) -> [u8; OAC1_LEN] {
    let mut b = encode_oac1_body(c);
    b[OAC1_CLASS_OFF] = class as u8;
    b
}

fn encode_oac1_body(c: &ActCommand) -> [u8; OAC1_LEN] {
    let mut b = [0u8; OAC1_LEN];
    b[0..4].copy_from_slice(&OAC1_MAGIC);
    b[4..6].copy_from_slice(&c.actuator_id.to_le_bytes());
    b[6..10].copy_from_slice(&c.cmd_seq.to_le_bytes());
    b[10..18].copy_from_slice(&c.boot_id.to_le_bytes());
    b[18..26].copy_from_slice(&c.deadline_ms.to_le_bytes());
    b[26..30].copy_from_slice(&c.force.to_le_bytes());
    b[30..34].copy_from_slice(&c.torque.to_le_bytes());
    b[34..38].copy_from_slice(&c.velocity.to_le_bytes());
    for i in 0..3 {
        b[38 + 4 * i..42 + 4 * i].copy_from_slice(&c.pos[i].to_le_bytes());
    }
    b
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(x)
}

/// Parse an **`Act`** order. Unchanged behaviour from before part G: byte 50 must be 0,
/// so a `Stop` order is refused here and can never be mistaken for an `Act` by a caller
/// that does not know about classes.
pub fn parse_oac1(b: &[u8]) -> Option<ActCommand> {
    match parse_oac1_any(b) {
        Some((c, OrderClass::Act)) => Some(c),
        _ => None,
    }
}

/// Parse an order of any class.
pub fn parse_oac1_any(b: &[u8]) -> Option<(ActCommand, OrderClass)> {
    // Byte 50 is the class; bytes 51..54 stay reserved and must be zero (one encoding
    // per order; the parser used to ignore all four, found by cargo-fuzz 2026-10-07).
    if b.len() != OAC1_LEN || b[0..4] != OAC1_MAGIC || b[OAC1_CLASS_OFF + 1..OAC1_LEN] != [0u8; 3] {
        return None;
    }
    let class = OrderClass::from_byte(b[OAC1_CLASS_OFF])?;
    Some((parse_oac1_fields(b), class))
}

fn parse_oac1_fields(b: &[u8]) -> ActCommand {
    ActCommand {
        actuator_id: u16::from_le_bytes([b[4], b[5]]),
        cmd_seq: u32::from_le_bytes([b[6], b[7], b[8], b[9]]),
        boot_id: u64_at(b, 10),
        deadline_ms: u64_at(b, 18),
        force: f32_at(b, 26),
        torque: f32_at(b, 30),
        velocity: f32_at(b, 34),
        pos: [f32_at(b, 38), f32_at(b, 42), f32_at(b, 46)],
    }
}

pub fn encode_otm1(boot_id: u64, now_ms: u64) -> [u8; OTM1_LEN] {
    let mut b = [0u8; OTM1_LEN];
    b[0..4].copy_from_slice(&OTM1_MAGIC);
    b[4..12].copy_from_slice(&boot_id.to_le_bytes());
    b[12..20].copy_from_slice(&now_ms.to_le_bytes());
    b
}

/// `(boot_id, now_ms)` from an actuator time beacon.
pub fn parse_otm1(b: &[u8]) -> Option<(u64, u64)> {
    if b.len() != OTM1_LEN || b[0..4] != OTM1_MAGIC {
        return None;
    }
    Some((u64_at(b, 4), u64_at(b, 12)))
}
