//! LoRa time-on-air + duty-cycle throttling — pure `core`, integer-exact, `no_std`.
//!
//! Two things a real IoT LoRa node cannot ship without:
//!   1. [`airtime_us`] — the Semtech SX126x time-on-air for a payload, so a node
//!      knows the cost of every transmission.
//!   2. [`DutyCycleThrottler`] — a token bucket (in µs of airtime) that keeps a
//!      node within a regional duty cycle (e.g. EU868 1%). Transmitting outside
//!      this is illegal in EU/most regions; this is the software half of
//!      compliance (the radio driver enforces it by calling `try_send` first).
//!
//! No hardware, no float, no allocation: every node (host or MCU) can use it.
//! The caller supplies a monotonic `now_ms` (the MCU/host clock), so the bucket
//! works the same on a Cortex-M0+ as on a laptop.

use crate::LoRaParams;

/// Bandwidth in Hz for an SX126x LoRa bandwidth code. Delegates to
/// [`crate::sx126x::bw_hz`] so the full (non-Hz-ordered) register map and the
/// LDRO decision share one source of truth.
pub fn bw_hz(bw_code: u8) -> u32 {
    crate::sx126x::bw_hz(bw_code)
}

/// Time-on-air, in **microseconds**, for `payload_len` bytes under `params`.
///
/// Uses the SX126x reference formula with the common LoRaWAN defaults that the
/// SimulatedLoRaRadio and a real driver share: coding rate 4/5, explicit header,
/// CRC on, 8-symbol preamble, low-data-rate-optimize auto-enabled when the symbol
/// time is ≥ 16 ms (via [`crate::sx126x::ldro`]). Integer-exact for those defaults
/// (verified against the published airtime tables — see tests).
pub fn airtime_us(params: &LoRaParams, payload_len: usize) -> u64 {
    let sf = (params.sf.clamp(6, 12)) as u64;
    let bw = bw_hz(params.bw_code) as u64;
    let pl = payload_len as u64;
    let cr = 1u64; // 4/5  -> (CR+4) = 5 symbols per block
    let crc = 1u64; // CRC on
    let ih = 0u64; // explicit header
    let n_pre = 8u64; // preamble symbols
                      // Low-data-rate optimize — shared rule (symbol time ≥ 16 ms), keyed off the
                      // real bandwidth, not the raw register code. Single source: `sx126x::ldro`.
    let de = crate::sx126x::ldro(sf as u8, params.bw_code) as u64;

    // Symbol time (µs): 2^SF / BW seconds.
    let t_sym = ((1u64 << sf) * 1_000_000) / bw;

    // payloadSymbNb = 8 + max(ceil((8*PL - 4*SF + 28 + 16*CRC - 20*IH) / (4*(SF - 2*DE))) * (CR+4), 0)
    let num = (8 * pl + 28 + 16 * crc).saturating_sub(4 * sf + 20 * ih);
    let den = 4 * (sf - 2 * de);
    let payload_symb = 8 + num.div_ceil(den) * (cr + 4);

    // Preamble = (n_pre + 4.25) * t_sym = ((n_pre*4 + 17) * t_sym) / 4  (exact in µs)
    let t_preamble = ((n_pre * 4 + 17) * t_sym) / 4;
    let t_payload = payload_symb * t_sym;
    t_preamble + t_payload
}

/// Token-bucket duty-cycle throttle. Tokens are µs of *airtime*; the bucket
/// refills at `duty_permille/1000` µs of airtime per ms of wall-clock — i.e. a
/// `duty_permille` of 10 == a 1.0% duty cycle, 1 == 0.1%.
///
/// `no_std`: the caller passes a monotonic `now_ms`; nothing here reads a clock.
#[derive(Debug, Clone)]
pub struct DutyCycleThrottler {
    duty_permille: u32,
    tokens_us: u64,
    capacity_us: u64,
    last_ms: u64,
}

impl DutyCycleThrottler {
    /// `duty_permille`: parts-per-thousand duty (10 = 1%). `capacity_us`: max
    /// burst airtime that can accumulate (must be ≥ one packet's airtime to ever
    /// send). `start_ms`: the node's current monotonic time.
    pub fn new(duty_permille: u32, capacity_us: u64, start_ms: u64) -> Self {
        Self {
            duty_permille,
            tokens_us: capacity_us,
            capacity_us,
            last_ms: start_ms,
        }
    }

    /// EU868 default sub-band: 1% duty, ~1 s airtime burst capacity.
    /// Parts-per-thousand duty (10 = 1 %). Needed by the transport to tell a caller
    /// how long until a refused frame would fit (C3).
    pub fn duty_permille(&self) -> u32 {
        self.duty_permille
    }

    pub fn eu868_1pct(start_ms: u64) -> Self {
        Self::new(10, 1_000_000, start_ms)
    }

    fn refill(&mut self, now_ms: u64) {
        if now_ms > self.last_ms {
            // µs of airtime earned = elapsed_ms * (duty_permille/1000) * 1000 = elapsed_ms * duty_permille
            let earned_us = (now_ms - self.last_ms) * self.duty_permille as u64;
            self.tokens_us = (self.tokens_us + earned_us).min(self.capacity_us);
            self.last_ms = now_ms;
        }
    }

    /// Account for a transmission of `airtime_us`. Returns `true` (and consumes
    /// the budget) if it fits the duty cycle now, `false` if the node must wait.
    pub fn try_send(&mut self, airtime_us: u64, now_ms: u64) -> bool {
        self.refill(now_ms);
        if self.tokens_us >= airtime_us {
            self.tokens_us -= airtime_us;
            true
        } else {
            false
        }
    }

    /// Airtime budget (µs) available right now, after refilling to `now_ms`.
    pub fn available_us(&mut self, now_ms: u64) -> u64 {
        self.refill(now_ms);
        self.tokens_us
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(sf: u8, bw_code: u8) -> LoRaParams {
        LoRaParams {
            sf,
            bw_code,
            ..LoRaParams::default()
        }
    }

    #[test]
    fn airtime_matches_published_reference_sf7_125k_16b() {
        // SF7, BW125, CR4/5, explicit header, CRC on, 8-sym preamble, 16 B payload.
        // Published time-on-air ≈ 51.456 ms (Semtech / online LoRa calculators).
        assert_eq!(airtime_us(&params(7, 4), 16), 51_456);
    }

    #[test]
    fn airtime_grows_with_sf_and_payload() {
        let a_sf7 = airtime_us(&params(7, 4), 16);
        let a_sf12 = airtime_us(&params(12, 4), 16);
        assert!(a_sf12 > a_sf7 * 10, "SF12 must be far slower than SF7");
        let a_small = airtime_us(&params(9, 4), 8);
        let a_big = airtime_us(&params(9, 4), 200);
        assert!(a_big > a_small, "more payload => more airtime");
    }

    #[test]
    fn wider_bandwidth_is_faster() {
        assert!(airtime_us(&params(9, 6), 32) < airtime_us(&params(9, 4), 32)); // 500 kHz < 125 kHz airtime
    }

    #[test]
    fn duty_cycle_throttles_then_recovers() {
        let air = airtime_us(&params(7, 4), 16); // 51_456 µs
        let mut t = DutyCycleThrottler::new(10, 1_000_000, 0); // 1%, 1 s burst

        // Drain the burst: 1_000_000 / 51_456 ≈ 19 packets, then deny.
        let mut sent = 0;
        while t.try_send(air, 0) {
            sent += 1;
        }
        assert!(
            (18..=20).contains(&sent),
            "burst allows ~19 packets, got {sent}"
        );
        assert!(!t.try_send(air, 0), "throttled once burst is spent");

        // After the burst ~22 ms of budget remained; one more SF7 packet (51.456 ms)
        // needs ~29 ms more airtime, i.e. ~2.9 s at 1%.
        assert!(
            !t.try_send(air, 2_000),
            "2 s after burst: still short of a full packet"
        );
        assert!(
            t.try_send(air, 3_000),
            "3 s after burst: one packet's budget is back"
        );
    }

    #[test]
    fn never_exceeds_capacity_on_long_idle() {
        let mut t = DutyCycleThrottler::new(10, 1_000_000, 0);
        let _ = t.try_send(500_000, 0); // spend half
                                        // idle for an hour — tokens cap at capacity, not unbounded.
        assert_eq!(t.available_us(3_600_000), 1_000_000);
    }
}
