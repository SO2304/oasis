//! OASIS LoRa Transport — scaffolding for Gap 1 (real radio transport).
//!
//! This crate defines the **abstraction layer** between an OASIS
//! `MeshRouter` and a LoRa radio (SX126x family: SX1261, SX1262, SX1268).
//! It does NOT contain a real SX126x SPI driver yet — that's the final
//! piece when hardware lands. It DOES give you:
//!
//! 1. A minimal `LoRaRadio` trait matching SX126x-style APIs: init,
//!    set modulation, tx_payload, rx_payload, sleep. Blocking; no async.
//! 2. A `SimulatedLoRaRadio` that routes bytes through an in-process
//!    channel (host-only, `std` feature). Two instances paired with
//!    crossed tx/rx channels = a simulated radio link, with optional
//!    loss injection.
//! 3. A `LoRaTransport<R: LoRaRadio>` glue that wraps OASIS's existing
//!    `pack_lora_frame` / `parse_lora_frame` around the radio trait,
//!    so a `MeshRouter::origin_wrap()` envelope goes TX → air → RX
//!    → `MeshRouter::process()` byte-for-byte.
//!
//! When real hardware lands:
//!
//! ```ignore
//! use lora_transport::LoRaTransport;
//! // Replace this stub:
//! // let radio = SimulatedLoRaRadio::new(...);
//! // With a real driver, e.g.:
//! let radio = Sx1262Driver::new(spi, nss, reset, busy, dio1);
//! let transport = LoRaTransport::new(radio, LoRaParams::default());
//! ```
//!
//! The rest of the code — the `MeshRouter`, `pack_lora_frame`, v0A
//! signing — doesn't change.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

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
}

impl<R: LoRaRadio> LoRaTransport<R> {
    pub fn new(mut radio: R, params: LoRaParams) -> Result<Self, LoRaError> {
        radio.init(&params)?;
        Ok(Self { radio, params })
    }

    /// Wrap `envelope` in an OASIS LoRa frame header (magic + ver + len + crc)
    /// and transmit. The frame format matches `oasis-rt::transport::pack_lora_frame`.
    pub fn send_envelope(&mut self, envelope: &[u8]) -> Result<(), LoRaError> {
        let frame = pack_lora_frame_local(envelope)?;
        if frame.len() > self.radio.max_payload() {
            return Err(LoRaError::PayloadTooLarge(frame.len()));
        }
        self.radio.tx_payload(&frame)
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

// ── SX1262 driver hook point — stub, to be replaced with real driver.

pub mod sx1262;

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
