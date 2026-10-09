//! Message budget: how many OASIS messages fit in an hour of a regional duty cycle.
//!
//! This replaces `docs/lora_budget.py`, which was deleted on 2026-10-09. The script was a
//! **second implementation of [`crate::airtime::airtime_us`]** in another language, and
//! nothing compared the two: the documents quoted the script's numbers while the radio
//! path charged the Rust one. Six documents cite those figures, so the two agreeing was
//! load-bearing and unchecked. They do agree, and `tests` is where that is now asserted
//! rather than assumed — including the 2 793.5 ms at SF12 / 64 B that the script
//! self-checked against `docs/compliance/PQC.md`.
//!
//! Pure `core`, integer-exact, no float: the same arithmetic on a Cortex-M0+ and a laptop.
//!
//! ⚠️ The time-on-air formula itself is **not verified at the source**: the Semtech
//! datasheet is behind a commercial portal. What is verified is that this code reproduces
//! the figures published in the documents that cite it. Exactness is with respect to that
//! formula, not to a measured radio — and **no radio has ever transmitted**.

use crate::airtime::airtime_us;
use crate::LoRaParams;

/// ETSI EN 300 220-2 clause 4.4.3.2: `Tobs` = 1 h, and the limit is **per band**, not per
/// channel, so the three default LoRaWAN channels of band M share one budget.
pub const DUTY_US_PER_HOUR_1PCT: u64 = 36_000_000;

/// LoRaWAN EU863-870 **application** payload cap, RP002-1.0.3 tables 12/13.
///
/// This is the cap that makes the point: 51 bytes at SF10–SF12, while a v0B header alone
/// is 99, so **no OASIS message fits at long range on LoRaWAN class A**. Raw LoRa has a
/// 255-byte PHY payload and no such cap, which is why the raw-LoRa column is the useful
/// one.
pub const fn lorawan_cap(sf: u8) -> u16 {
    match sf {
        7 | 8 => 242,
        9 => 115,
        10..=12 => 51,
        _ => 0,
    }
}

/// LoRaWAN frame overhead charged on top of the application payload.
///
/// ⚠️ **Not verified at the source** (TS001 not re-read). 13 bytes is the convention
/// `docs/compliance/PQC.md` §4 used — 51 + 13 = 64, hence its time-on-air column at
/// PL = 64 — and it is kept so the two documents stay comparable.
pub const LORAWAN_OVERHEAD: u16 = 13;

/// The EU868 parameters the budget is quoted for: BW 125 kHz, CR 4/5, 8-symbol preamble.
pub fn eu868(sf: u8) -> LoRaParams {
    LoRaParams {
        sf,
        bw_code: 4,
        ..LoRaParams::default()
    }
}

/// Time on air in µs for `payload_len` bytes at `sf`, EU868 125 kHz.
pub fn toa_us(sf: u8, payload_len: usize) -> u64 {
    airtime_us(&eu868(sf), payload_len)
}

/// How many such messages a 1 % band budget allows per hour. Zero means one does not fit.
pub fn msgs_per_hour(sf: u8, payload_len: usize) -> u64 {
    let t = toa_us(sf, payload_len);
    if t == 0 {
        return 0;
    }
    DUTY_US_PER_HOUR_1PCT / t
}

/// What fragmenting `nbytes` costs at `sf`: `(fragments, airtime µs, hundredths of the
/// hourly budget)`.
///
/// A payload over the application cap must be fragmented (`OFR1`), and every fragment is
/// charged against the same 36 s per hour per band. The third value is in **hundredths of
/// an hour of budget** so the unit stays integer: 1150 means 11.5 hours of the hourly
/// budget, i.e. the message cannot be sent legally in under 11.5 h.
pub fn frag_cost(nbytes: usize, sf: u8) -> (u32, u64, u64) {
    let cap = lorawan_cap(sf) as usize;
    if cap == 0 || nbytes == 0 {
        return (0, 0, 0);
    }
    let frames = nbytes.div_ceil(cap) as u32;
    let air = frames as u64 * toa_us(sf, cap + LORAWAN_OVERHEAD as usize);
    (frames, air, air * 100 / DUTY_US_PER_HOUR_1PCT)
}

#[cfg(test)]
mod tests;
