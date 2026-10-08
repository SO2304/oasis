//! OASIS LoRa Transport — scaffolding for Gap 1 (real radio transport).
//!
//! This crate defines the **abstraction layer** between an OASIS
//! `MeshRouter` and a LoRa radio (SX126x family: SX1261, SX1262, SX1268),
//! and ships a real (mock-tested, not yet silicon-run) SX1262 driver. It
//! gives you:
//!
//! 1. A minimal `LoRaRadio` trait matching SX126x-style APIs: init,
//!    set modulation, tx_payload, rx_payload, sleep. Blocking; no async.
//! 2. A [`sx126x`] command-encoding layer — datasheet-exact, `no_std`,
//!    pure functions, unit-tested against canonical reference values.
//! 3. A [`sx1262::Sx1262Driver`] implementing `LoRaRadio` over
//!    `embedded-hal 1.0` (SPI + GPIO + delay). The command sequencing is
//!    mock-SPI tested; it has **not** been run on real silicon.
//! 4. A `SimulatedLoRaRadio` that routes bytes through an in-process
//!    channel (host-only, `std` feature). Two instances paired with
//!    crossed tx/rx channels = a simulated radio link, with optional
//!    loss injection.
//! 5. A `LoRaTransport<R: LoRaRadio>` glue that wraps OASIS's existing
//!    `pack_lora_frame` / `parse_lora_frame` around the radio trait,
//!    so a `MeshRouter::origin_wrap()` envelope goes TX → air → RX
//!    → `MeshRouter::process()` byte-for-byte.
//!
//! Wiring a real radio (RP2040 example):
//!
//! ```ignore
//! use oasis_lora_transport::{LoRaTransport, LoRaParams, sx1262::Sx1262Driver};
//! // `spi`, the pins and `delay` come from the board HAL (embedded-hal 1.0).
//! let radio = Sx1262Driver::new(spi, nss, busy, reset, dio1, delay);
//! let transport = LoRaTransport::new(radio, LoRaParams::default())?;
//! ```
//!
//! The rest of the code — the `MeshRouter`, `pack_lora_frame`, v0A
//! signing — doesn't change.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

use crate::airtime::{airtime_us, DutyCycleThrottler};

/// Errors a LoRa radio driver may return.
#[derive(Debug)]
pub enum LoRaError {
    /// Transmit buffer was larger than the radio's max payload.
    PayloadTooLarge(usize),
    /// RX completed but CRC check failed at the PHY level.
    CrcFail,
    /// No packet within the configured timeout.
    Timeout,
    /// Lower-level driver error (SPI, GPIO, etc.).
    Driver(&'static str),
    /// Frame-layer parse error (bad magic / version / OASIS header).
    Frame(&'static str),
    /// The regional duty cycle has no room for this frame yet. **Nothing was
    /// transmitted.** `needed_us` is the frame's time on air, `available_us` what the
    /// budget holds now, and `retry_in_ms` how long until it would fit.
    ///
    /// This exists because `POSITIONING_GAPS.md` C3 was real: the throttler was
    /// implemented and used by two examples, while `send_envelope` transmitted
    /// unconditionally. A duty cycle a caller may forget is not an enforced one.
    DutyCycleExceeded { needed_us: u64, available_us: u64, retry_in_ms: u64 },
}

/// LoRa modulation parameters — matches SX126x register layout semantics.
/// Actual regulatory / range tuning happens at integration time.
#[derive(Clone, Copy, Debug)]
pub struct LoRaParams {
    /// Carrier frequency in Hz. Typical EU: 868_000_000. US: 915_000_000.
    pub frequency_hz: u32,
    /// Spreading factor (7..12). SF7 = high data rate, SF12 = max range.
    pub sf: u8,
    /// Bandwidth code: 0..9 per SX126x datasheet. 4 = 125 kHz (ISM).
    pub bw_code: u8,
    /// Coding rate: 5 = 4/5, 6 = 4/6, 7 = 4/7, 8 = 4/8.
    pub cr: u8,
    /// TX power in dBm. Regulatory max depends on region.
    pub tx_power_dbm: i8,
    /// Preamble length in symbols (default 8 on SX126x).
    pub preamble_len: u16,
    /// LoRa network sync word (private 0x12, public 0x34).
    pub sync_word: u8,
}

impl Default for LoRaParams {
    fn default() -> Self {
        Self {
            frequency_hz: 868_000_000,
            sf: 7,
            bw_code: 4,       // 125 kHz
            cr: 5,            // 4/5
            tx_power_dbm: 14, // ETSI EU868 max for general ISM use
            preamble_len: 8,
            sync_word: 0x12, // private network default
        }
    }
}

/// Abstract LoRa radio. Production impl wraps an SX1262 driver; tests
/// use a `SimulatedLoRaRadio` that routes bytes through channels.
pub trait LoRaRadio {
    /// One-time init: wake from sleep, configure modulation + sync word,
    /// set frequency, apply TX power. Returns Err on any low-level fail.
    fn init(&mut self, params: &LoRaParams) -> Result<(), LoRaError>;

    /// Transmit a single packet. Blocks until TX done irq fires.
    fn tx_payload(&mut self, payload: &[u8]) -> Result<(), LoRaError>;

    /// Receive one packet into `buf`, returning its length. `timeout_ms`
    /// = 0 means continuous RX (caller polls); > 0 means return
    /// `LoRaError::Timeout` after that many ms without reception.
    fn rx_payload(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, LoRaError>;

    /// Put radio to sleep (minimal current draw). Nice-to-have on MCU.
    fn sleep(&mut self) -> Result<(), LoRaError> {
        Ok(())
    }

    /// Max payload the chip can fit in a single packet. SX126x FIFO
    /// is 256 bytes; useable payload is less due to header / CRC.
    fn max_payload(&self) -> usize {
        255
    }
}

/// High-level transport pairing a LoRa radio with OASIS frame packing.
///
/// Usage from an OASIS mesh node:
/// ```ignore
/// let mut router = MeshRouter::new_ed25519_signed(fp, seed, registry);
/// let mut transport = LoRaTransport::new(radio, LoRaParams::default())?;
///
/// let envelope = router.origin_wrap(b"payload");
/// transport.send_envelope(&envelope)?;        // TX over air
///
/// let mut rx_buf = [0u8; 255];
/// let env_rx = transport.recv_envelope(&mut rx_buf, 5000)?;
/// match router.process(env_rx) {
///     MeshDecision::Arrived { .. } => { /* verified */ },
///     _ => { /* reject */ },
/// }
/// ```
pub struct LoRaTransport<R: LoRaRadio> {
    radio: R,
    params: LoRaParams,
    /// `None` only for a region with no duty limit, chosen through
    /// [`LoRaTransport::new_without_duty_limit`]. Bypassing the budget has to be a
    /// visible decision, never an omission.
    duty: Option<DutyCycleThrottler>,
}

impl<R: LoRaRadio> LoRaTransport<R> {
    /// EU868 at 1 %: the duty cycle is enforced by this transport, not left to the
    /// caller. `start_ms` seeds the budget clock.
    pub fn new(radio: R, params: LoRaParams) -> Result<Self, LoRaError> {
        Self::new_at(radio, params, 0)
    }

    /// Same, with an explicit start time for the duty-cycle clock.
    pub fn new_at(mut radio: R, params: LoRaParams, start_ms: u64) -> Result<Self, LoRaError> {
        radio.init(&params)?;
        Ok(Self { radio, params, duty: Some(DutyCycleThrottler::eu868_1pct(start_ms)) })
    }

    /// No duty-cycle enforcement. For a region or band that imposes none, or for a
    /// bench where the radio is a simulation. Named so that reading the call site is
    /// enough to see that the budget is off.
    pub fn new_without_duty_limit(mut radio: R, params: LoRaParams) -> Result<Self, LoRaError> {
        radio.init(&params)?;
        Ok(Self { radio, params, duty: None })
    }

    /// Wrap `envelope` in an OASIS LoRa frame header (magic + ver + len + crc) and
    /// transmit — **if the duty cycle allows it at `now_ms`**.
    ///
    /// On [`LoRaError::DutyCycleExceeded`] the radio is **not touched**: the budget is
    /// checked before `tx_payload`, which `duty_cycle_refuses_before_the_radio_is_touched`
    /// proves by counting transmissions on a simulated radio.
    ///
    /// ⚠️ The time-on-air formula comes from the SX1276 datasheet's published form, which
    /// is **not verified at the source** (`docs/compliance/PQC.md` §3): the enforcement is
    /// exact with respect to that formula, not with respect to a measured radio. No radio
    /// has ever transmitted here.
    pub fn send_envelope(&mut self, envelope: &[u8], now_ms: u64) -> Result<(), LoRaError> {
        let frame = pack_lora_frame_local(envelope)?;
        if frame.len() > self.radio.max_payload() {
            return Err(LoRaError::PayloadTooLarge(frame.len()));
        }
        let needed_us = airtime_us(&self.params, frame.len());
        if let Some(duty) = self.duty.as_mut() {
            if !duty.try_send(needed_us, now_ms) {
                let available_us = duty.available_us(now_ms);
                // permille µs of airtime are earned per ms of wall clock.
                let short = needed_us.saturating_sub(available_us);
                let retry_in_ms = short.div_ceil(duty.duty_permille().max(1) as u64);
                return Err(LoRaError::DutyCycleExceeded { needed_us, available_us, retry_in_ms });
            }
        }
        self.radio.tx_payload(&frame)
    }

    /// Airtime budget (µs) available at `now_ms`, or `None` when enforcement is off.
    pub fn duty_budget_us(&mut self, now_ms: u64) -> Option<u64> {
        self.duty.as_mut().map(|d| d.available_us(now_ms))
    }

    /// Receive one LoRa packet, strip the OASIS frame header, return the
    /// payload (the v0A envelope). `buf` must be at least 255 B for a
    /// full SX126x FIFO.
    pub fn recv_envelope<'b>(
        &mut self,
        buf: &'b mut [u8],
        timeout_ms: u32,
    ) -> Result<&'b [u8], LoRaError> {
        let n = self.radio.rx_payload(buf, timeout_ms)?;
        parse_lora_frame_local(&buf[..n])
            .map(|(hdr_len, payload_len)| &buf[hdr_len..hdr_len + payload_len])
    }

    pub fn params(&self) -> &LoRaParams {
        &self.params
    }

    /// Time-on-air (µs) to send an `envelope_len`-byte OASIS envelope, including
    /// the 8-byte LoRa frame header. Pair with a
    /// The duty cycle is enforced by [`LoRaTransport::send_envelope`] itself (C3).
    pub fn airtime_us(&self, envelope_len: usize) -> u64 {
        crate::airtime::airtime_us(&self.params, FRAME_HEADER_LEN + envelope_len)
    }
    pub fn radio(&mut self) -> &mut R {
        &mut self.radio
    }
}

// ── OASIS LoRa frame format (mirror of oasis-rt::transport, kept here
//    as a no_std inline copy so this crate can build without std). ───
//
//   0..4   = LORA magic
//   4      = version (0x01)
//   5..7   = payload length u16 little-endian
//   7      = header CRC-8
//   8..    = payload (the OASIS mesh envelope bytes)

const FRAME_MAGIC: &[u8; 4] = b"LORA";
const FRAME_VER: u8 = 0x01;
const FRAME_HEADER_LEN: usize = 8;

fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn pack_lora_frame_local(payload: &[u8]) -> Result<Vec<u8>, LoRaError> {
    if payload.len() > 65535 {
        return Err(LoRaError::PayloadTooLarge(payload.len()));
    }
    let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + payload.len());
    frame.extend_from_slice(FRAME_MAGIC);
    frame.push(FRAME_VER);
    let len = payload.len() as u16;
    frame.extend_from_slice(&len.to_le_bytes());
    let header_crc = crc8(&frame);
    frame.push(header_crc);
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn parse_lora_frame_local(frame: &[u8]) -> Result<(usize, usize), LoRaError> {
    if frame.len() < FRAME_HEADER_LEN {
        return Err(LoRaError::Frame("frame too short"));
    }
    if &frame[..4] != FRAME_MAGIC {
        return Err(LoRaError::Frame("bad magic"));
    }
    if frame[4] != FRAME_VER {
        return Err(LoRaError::Frame("unsupported version"));
    }
    let len = u16::from_le_bytes([frame[5], frame[6]]) as usize;
    let expected_crc = crc8(&frame[..7]);
    if frame[7] != expected_crc {
        return Err(LoRaError::Frame("header crc mismatch"));
    }
    if frame.len() < FRAME_HEADER_LEN + len {
        return Err(LoRaError::Frame("payload truncated"));
    }
    Ok((FRAME_HEADER_LEN, len))
}

// ── Simulated radio — host-only, routes bytes through channels. ────

#[cfg(feature = "std")]
pub mod sim;

// ── SX126x command-encoding layer (datasheet-exact, no_std, tested).

pub mod sx126x;

// ── SX1262 driver — real, embedded-hal 1.0, mock-SPI tested (not silicon-run).

pub mod sx1262;

// ── Airtime + duty-cycle throttle (no_std, pure math — regional compliance).

pub mod airtime;

// ── Gap 2 — FHSS anti-narrowband-jam (host-only sim for now).

#[cfg(feature = "std")]
pub mod fhss;

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip_preserves_payload() {
        let payload = b"OASIS v0A envelope bytes";
        let framed = pack_lora_frame_local(payload).unwrap();
        assert_eq!(&framed[..4], FRAME_MAGIC);
        assert_eq!(framed[4], FRAME_VER);
        let (hdr_len, pl_len) = parse_lora_frame_local(&framed).unwrap();
        assert_eq!(hdr_len, FRAME_HEADER_LEN);
        assert_eq!(pl_len, payload.len());
        assert_eq!(&framed[hdr_len..hdr_len + pl_len], payload);
    }

    #[test]
    fn frame_rejects_tampered_crc() {
        let mut f = pack_lora_frame_local(b"x").unwrap();
        f[7] ^= 0x01;
        assert!(matches!(
            parse_lora_frame_local(&f),
            Err(LoRaError::Frame("header crc mismatch"))
        ));
    }

    #[test]
    fn frame_rejects_bad_magic() {
        let mut f = pack_lora_frame_local(b"x").unwrap();
        f[0] = b'X';
        assert!(matches!(
            parse_lora_frame_local(&f),
            Err(LoRaError::Frame("bad magic"))
        ));
    }
}

#[cfg(test)]
mod duty_tests {
    use super::*;
    use crate::airtime::airtime_us;

    /// A radio that counts, and nothing else. If the duty-cycle check runs before the
    /// radio, a refused frame leaves `tx_calls` untouched.
    struct CountingRadio {
        tx_calls: u32,
        last_len: usize,
    }

    impl LoRaRadio for CountingRadio {
        fn init(&mut self, _p: &LoRaParams) -> Result<(), LoRaError> {
            Ok(())
        }
        fn tx_payload(&mut self, payload: &[u8]) -> Result<(), LoRaError> {
            self.tx_calls += 1;
            self.last_len = payload.len();
            Ok(())
        }
        fn rx_payload(&mut self, _b: &mut [u8], _t: u32) -> Result<usize, LoRaError> {
            Err(LoRaError::Timeout)
        }
        fn max_payload(&self) -> usize {
            255
        }
    }

    fn transport() -> LoRaTransport<CountingRadio> {
        LoRaTransport::new_at(CountingRadio { tx_calls: 0, last_len: 0 }, LoRaParams::default(), 0)
            .unwrap()
    }

    /// C3: the first frames fit the EU868 1 % burst, then the budget refuses — and the
    /// refusal carries what a caller needs to pace itself.
    #[test]
    fn duty_cycle_refuses_once_the_budget_is_spent() {
        let mut t = transport();
        let env = [0x5Au8; 60];
        let air = airtime_us(&LoRaParams::default(), 60 + FRAME_HEADER_LEN);

        let mut sent = 0u32;
        let mut refused: Option<(u64, u64, u64)> = None;
        for _ in 0..200 {
            match t.send_envelope(&env, 0) {
                Ok(()) => sent += 1,
                Err(LoRaError::DutyCycleExceeded { needed_us, available_us, retry_in_ms }) => {
                    refused = Some((needed_us, available_us, retry_in_ms));
                    break;
                }
                Err(e) => panic!("unexpected error: {e:?}"),
            }
        }
        assert!(sent > 0, "the burst capacity must allow at least one frame");
        let (needed, available, retry) = refused.expect("the budget must run out");
        assert!(needed > available, "refused because {needed} us > {available} us");
        assert!(retry > 0, "a refusal must say when to try again");
        // The frame's own airtime is what was charged each time.
        assert!(needed >= air - 1 && needed <= air + 1, "needed {needed} vs airtime {air}");
        assert_eq!(t.radio().tx_calls, sent, "exactly the accepted frames reached the radio");
    }

    /// The property that makes the enforcement real: on a refusal the radio is **not
    /// touched**. Checked by counting transmissions, not by reading the error.
    #[test]
    fn duty_cycle_refuses_before_the_radio_is_touched() {
        let mut t = transport();
        let env = [0x11u8; 60];
        while t.send_envelope(&env, 0).is_ok() {}
        let after_burst = t.radio().tx_calls;

        for _ in 0..50 {
            match t.send_envelope(&env, 0) {
                Err(LoRaError::DutyCycleExceeded { .. }) => {}
                other => panic!("expected a duty-cycle refusal, got {other:?}"),
            }
        }
        assert_eq!(
            t.radio().tx_calls, after_burst,
            "50 refused frames must not have reached the radio"
        );
    }

    /// And the budget comes back with time, at the rate the regulation allows: 1 % means
    /// 10 us of airtime earned per ms of wall clock.
    #[test]
    fn duty_cycle_recovers_at_one_percent() {
        let mut t = transport();
        let env = [0x22u8; 60];
        while t.send_envelope(&env, 0).is_ok() {}
        assert!(t.send_envelope(&env, 0).is_err());

        let air = airtime_us(&LoRaParams::default(), 60 + FRAME_HEADER_LEN);
        let wait_ms = air / 10 + 1; // 1 % -> 10 us of airtime per ms
        assert!(
            t.send_envelope(&env, wait_ms).is_ok(),
            "after {wait_ms} ms the budget must hold one more frame"
        );
        assert!(
            t.send_envelope(&env, wait_ms).is_err(),
            "but only one: the budget is not a free pass"
        );
    }

    /// Opting out is possible, and has to be visible at the call site.
    #[test]
    fn opting_out_is_explicit() {
        let mut t = LoRaTransport::new_without_duty_limit(
            CountingRadio { tx_calls: 0, last_len: 0 },
            LoRaParams::default(),
        )
        .unwrap();
        let env = [0x33u8; 60];
        for _ in 0..500 {
            t.send_envelope(&env, 0).expect("no limit means no refusal");
        }
        assert_eq!(t.radio().tx_calls, 500);
        assert_eq!(t.duty_budget_us(0), None, "no budget to report when enforcement is off");
    }
}
