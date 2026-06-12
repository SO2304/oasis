//! SX126x command-encoding layer — datasheet-exact, `core`-only, `no_std`.
//!
//! These are the **pure** building blocks of the SX1262 driver: opcodes and the
//! byte payloads of each command, with no hardware and no I/O. They are the part
//! most likely to hide bugs (the frequency word, the SF/BW/CR codes, the PA
//! table), so they are encoded straight from the SX126x datasheet (rev 2.x) and
//! verified against the canonical reference values in the tests below.
//!
//! The driver ([`crate::sx1262`]) sends `OPCODE` followed by these argument bytes
//! over SPI. Keeping encoding separate from transport means the encoding is
//! testable on a host with zero hardware.

use crate::LoRaParams;

// ── Opcodes (SX126x datasheet §11, command table) ──────────────────────────
pub const OP_SET_SLEEP: u8 = 0x84;
pub const OP_SET_STANDBY: u8 = 0x80;
pub const OP_SET_TX: u8 = 0x83;
pub const OP_SET_RX: u8 = 0x82;
pub const OP_SET_PACKET_TYPE: u8 = 0x8A;
pub const OP_SET_RF_FREQUENCY: u8 = 0x86;
pub const OP_SET_PA_CONFIG: u8 = 0x95;
pub const OP_SET_TX_PARAMS: u8 = 0x8E;
pub const OP_SET_BUFFER_BASE: u8 = 0x8F;
pub const OP_SET_MODULATION_PARAMS: u8 = 0x8B;
pub const OP_SET_PACKET_PARAMS: u8 = 0x8C;
pub const OP_SET_DIO_IRQ_PARAMS: u8 = 0x08;
pub const OP_CLEAR_IRQ_STATUS: u8 = 0x02;
pub const OP_GET_IRQ_STATUS: u8 = 0x12;
pub const OP_WRITE_BUFFER: u8 = 0x0E;
pub const OP_READ_BUFFER: u8 = 0x1E;
pub const OP_GET_RX_BUFFER_STATUS: u8 = 0x13;
pub const OP_WRITE_REGISTER: u8 = 0x0D;
pub const OP_CALIBRATE_IMAGE: u8 = 0x98;

// ── Constants ──────────────────────────────────────────────────────────────
pub const STDBY_RC: u8 = 0x00;
pub const PACKET_TYPE_LORA: u8 = 0x01;
pub const LORA_HEADER_EXPLICIT: u8 = 0x00;
pub const RAMP_200_US: u8 = 0x04;

// IRQ bits (datasheet §11.4 / Table 13-29).
pub const IRQ_TX_DONE: u16 = 0x0001;
pub const IRQ_RX_DONE: u16 = 0x0002;
pub const IRQ_CRC_ERR: u16 = 0x0040;
pub const IRQ_TIMEOUT: u16 = 0x0200;

// LoRa sync-word registers.
pub const REG_LORA_SYNC_WORD_MSB: u16 = 0x0740;

/// 32-bit RF frequency word: `freq_hz * 2^25 / F_XTAL`, F_XTAL = 32 MHz.
/// Big-endian, as `SetRfFrequency` expects. (868.0 MHz → 0x36400000.)
pub fn rf_freq_word(freq_hz: u32) -> [u8; 4] {
    let word = (((freq_hz as u64) << 25) / 32_000_000) as u32;
    word.to_be_bytes()
}

/// LoRa low-data-rate optimize flag (symbol time > 16 ms). Mirrors `airtime`.
pub fn ldro(sf: u8, bw_code: u8) -> u8 {
    if (sf >= 11 && bw_code <= 4) || (sf == 12 && bw_code == 5) {
        1
    } else {
        0
    }
}

/// `SetModulationParams` (LoRa): [SF, BW, CR, LDRO].
/// `bw_code` is already the datasheet LoRa BW register value (4 = 125 kHz).
/// `cr` in [`LoRaParams`] is 5..8 (4/5..4/8); the register field is 1..4.
pub fn modulation_params(p: &LoRaParams) -> [u8; 4] {
    let sf = p.sf.clamp(5, 12);
    let cr_reg = p.cr.clamp(5, 8) - 4; // 4/5 -> 1
    [sf, p.bw_code, cr_reg, ldro(sf, p.bw_code)]
}

/// `SetPacketParams` (LoRa): preamble MSB/LSB, header type, payload len,
/// CRC on/off, invert-IQ. Explicit header, standard IQ.
pub fn packet_params(preamble_len: u16, payload_len: u8, crc_on: bool) -> [u8; 6] {
    let [pmsb, plsb] = preamble_len.to_be_bytes();
    [
        pmsb,
        plsb,
        LORA_HEADER_EXPLICIT,
        payload_len,
        crc_on as u8,
        0x00, // standard IQ
    ]
}

/// `SetPaConfig` for the SX1262, chosen from the datasheet optimal-settings
/// table (Table 13-21) by target power. Returns [paDutyCycle, hpMax, devSel, paLut].
pub fn pa_config(tx_power_dbm: i8) -> [u8; 4] {
    let (duty, hp_max) = match tx_power_dbm {
        p if p >= 22 => (0x04, 0x07),
        20 | 21 => (0x03, 0x05),
        17 | 18 | 19 => (0x02, 0x03),
        _ => (0x02, 0x02), // <= +14 dBm
    };
    [duty, hp_max, 0x00 /* SX1262 */, 0x01]
}

/// `SetTxParams`: [power, rampTime]. SX1262 power range -9..+22 dBm.
pub fn tx_params(tx_power_dbm: i8, ramp: u8) -> [u8; 2] {
    let p = tx_power_dbm.clamp(-9, 22) as u8;
    [p, ramp]
}

/// `SetDioIrqParams` routing the given IRQ mask to **DIO1** (DIO2/DIO3 unused).
pub fn dio1_irq_params(irq_mask: u16) -> [u8; 8] {
    let [im, il] = irq_mask.to_be_bytes();
    [im, il, im, il, 0, 0, 0, 0]
}

/// `WriteRegister` for the LoRa sync word: addr (2 B) + 2 data bytes.
/// 0x12 (private) → [0x14, 0x24]; 0x34 (public) → [0x34, 0x44].
pub fn sync_word_write(sync_word: u8) -> ([u8; 2], [u8; 2]) {
    let hi = ((sync_word >> 4) & 0x0F) << 4 | 0x04;
    let lo = (sync_word & 0x0F) << 4 | 0x04;
    (REG_LORA_SYNC_WORD_MSB.to_be_bytes(), [hi, lo])
}

/// 3-byte timeout word for `SetTx`/`SetRx` (step = 15.625 µs). 0 = no timeout
/// (TX) / single (RX). Saturates at the 24-bit max.
pub fn timeout_word(ms: u32) -> [u8; 3] {
    // 15.625 µs/tick = 1/64 ms, so 1 ms = 64 ticks.
    let ticks = ((ms as u64) * 64).min(0x00FF_FFFF) as u32;
    let b = ticks.to_be_bytes();
    [b[1], b[2], b[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(sf: u8, bw_code: u8, cr: u8) -> LoRaParams {
        LoRaParams {
            sf,
            bw_code,
            cr,
            ..LoRaParams::default()
        }
    }

    #[test]
    fn rf_freq_word_matches_canonical_values() {
        // Canonical SX126x reference values (RadioLib / Semtech).
        assert_eq!(rf_freq_word(868_000_000), [0x36, 0x40, 0x00, 0x00]);
        assert_eq!(rf_freq_word(915_000_000), [0x39, 0x30, 0x00, 0x00]);
        // monotonic
        assert!(rf_freq_word(869_000_000) > rf_freq_word(868_000_000));
    }

    #[test]
    fn modulation_params_sf7_bw125_cr45() {
        // SF7, BW 125 kHz (code 4), CR 4/5, no LDRO.
        assert_eq!(
            modulation_params(&params(7, 4, 5)),
            [0x07, 0x04, 0x01, 0x00]
        );
        // SF12, BW125 -> LDRO on; CR 4/8 -> reg 4.
        assert_eq!(
            modulation_params(&params(12, 4, 8)),
            [0x0C, 0x04, 0x04, 0x01]
        );
    }

    #[test]
    fn packet_params_explicit_crc() {
        // preamble 8, 16-byte payload, CRC on.
        assert_eq!(
            packet_params(8, 16, true),
            [0x00, 0x08, 0x00, 0x10, 0x01, 0x00]
        );
        // CRC off
        assert_eq!(packet_params(12, 255, false)[4], 0x00);
        assert_eq!(packet_params(12, 255, false)[3], 0xFF);
    }

    #[test]
    fn pa_config_table() {
        assert_eq!(pa_config(22), [0x04, 0x07, 0x00, 0x01]); // +22 dBm
        assert_eq!(pa_config(14), [0x02, 0x02, 0x00, 0x01]); // +14 dBm
        assert_eq!(pa_config(0), [0x02, 0x02, 0x00, 0x01]); // low power -> +14 row
    }

    #[test]
    fn tx_params_clamp() {
        assert_eq!(tx_params(14, RAMP_200_US), [14, 0x04]);
        assert_eq!(tx_params(99, RAMP_200_US), [22, 0x04]); // clamped to +22
        assert_eq!(tx_params(-50, RAMP_200_US), [(-9i8) as u8, 0x04]); // clamped to -9
    }

    #[test]
    fn dio1_routes_txdone_and_timeout() {
        let m = dio1_irq_params(IRQ_TX_DONE | IRQ_TIMEOUT);
        assert_eq!(&m[0..2], &[0x02, 0x01]); // global mask = 0x0201
        assert_eq!(&m[2..4], &[0x02, 0x01]); // DIO1 mask = same
        assert_eq!(&m[4..8], &[0, 0, 0, 0]); // DIO2/3 unused
    }

    #[test]
    fn sync_word_private_and_public() {
        let (addr, data) = sync_word_write(0x12);
        assert_eq!(addr, [0x07, 0x40]);
        assert_eq!(data, [0x14, 0x24]); // private network
        assert_eq!(sync_word_write(0x34).1, [0x34, 0x44]); // public (LoRaWAN)
    }

    #[test]
    fn timeout_word_units() {
        assert_eq!(timeout_word(0), [0, 0, 0]); // no timeout
                                                // 1000 ms / 15.625 µs ≈ 64000 ticks = 0x00FA00
        assert_eq!(timeout_word(1000), [0x00, 0xFA, 0x00]);
        assert_eq!(timeout_word(u32::MAX), [0xFF, 0xFF, 0xFF]); // saturates 24-bit
    }
}
