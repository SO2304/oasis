//! SX1262 driver hook point — **stub with an honest design sketch**, to
//! be replaced by a real driver (e.g., `lora-phy` or `sx126x-rs`) when
//! hardware lands. Compiles to nothing useful on its own; kept here as
//! the contract that a future PR will implement.
//!
//! Why a stub rather than vendoring a real driver now:
//!
//! - `lora-phy` (embassy-rs) is async and pulls a large dependency tree;
//!   committing to it without flying hardware risks lock-in on an API
//!   that may shift before we have a real board to validate against.
//! - `sx126x-rs` (blocking, stable) is a reasonable alternative but
//!   requires `embedded-hal 1.0` SPI + InputPin + OutputPin wiring
//!   that's MCU-specific (different HAL per target).
//!
//! The integration is cheap when hardware arrives: implement `LoRaRadio`
//! for a `Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1>` type and the rest
//! of the crate keeps working unchanged.
//!
//! ## Canonical wiring to Raspberry Pi Pico (RP2040)
//!
//! Tested commodity boards that expose an SX1262 over SPI1:
//! - Waveshare Pico-LoRa-SX1262 (915 MHz or 868 MHz variants)
//! - SEEED Wio-WM1110 dev board
//! - Adafruit Feather RP2040 with RFM95W (SX1276 — needs sx127x driver
//!   instead; wire format on-air identical at LoRa layer)
//!
//! ```text
//!   Pi Pico pin     SX1262 pin        Notes
//!   ─────────────── ─────────────     ───────────────────────────────
//!   GP10 (SPI1 SCK)  SCK              1 MHz nominal, up to 8 MHz
//!   GP11 (SPI1 TX)   MOSI
//!   GP12 (SPI1 RX)   MISO
//!   GP13 (SPI1 CS)   NSS              active low
//!   GP14             BUSY             input, radio-asserted when busy
//!   GP15             DIO1             RX/TX done IRQ
//!   GP16             RESET            output, active low pulse on init
//!   3V3 / GND        VCC / GND        SX1262 is 3.0-3.7 V
//! ```
//!
//! ## Canonical wiring to STM32F4 Discovery / Nucleo
//!
//! ```text
//!   STM32F4 pin      SX1262 pin
//!   ──────────────── ─────────────
//!   PA5 (SPI1 SCK)   SCK
//!   PA7 (SPI1 MOSI)  MOSI
//!   PA6 (SPI1 MISO)  MISO
//!   PB0              NSS
//!   PB1              BUSY
//!   PC13             DIO1 (EXTI13)
//!   PC14             RESET
//! ```

use crate::{LoRaError, LoRaParams, LoRaRadio};

/// Placeholder SX1262 driver. A future PR replaces this with the real
/// implementation using embedded-hal SPI + GPIO traits. The constructor
/// signature already anticipates that shape.
pub struct Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1> {
    pub spi: SPI,
    pub nss: NSS,
    pub busy: BUSY,
    pub reset: RESET,
    pub dio1: DIO1,
    initialized: bool,
}

impl<SPI, NSS, BUSY, RESET, DIO1> Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1> {
    pub fn new(spi: SPI, nss: NSS, busy: BUSY, reset: RESET, dio1: DIO1) -> Self {
        Self {
            spi,
            nss,
            busy,
            reset,
            dio1,
            initialized: false,
        }
    }
}

impl<SPI, NSS, BUSY, RESET, DIO1> LoRaRadio for Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1> {
    fn init(&mut self, _params: &LoRaParams) -> Result<(), LoRaError> {
        // TODO: real init sequence per SX126x datasheet rev 2.1 §15:
        //   1. Pull RESET low for 100 us then release
        //   2. Wait for BUSY=0
        //   3. set_standby(STDBY_RC)
        //   4. set_packet_type(PACKET_TYPE_LORA)
        //   5. set_rf_frequency(params.frequency_hz)
        //   6. set_pa_config(pa_duty_cycle, hp_max, device_sel, pa_lut)
        //   7. set_tx_params(params.tx_power_dbm, ramp_time)
        //   8. set_buffer_base_address(0x00, 0x00)
        //   9. set_modulation_params_lora(sf, bw, cr, ldro)
        //  10. set_packet_params_lora(preamble, header_type, payload_len,
        //                             crc_on, invert_iq)
        //  11. set_sync_word(params.sync_word)
        //  12. calibrate_image(freq)
        // Until the driver is real, refuse to pretend:
        self.initialized = false;
        Err(LoRaError::Driver(
            "Sx1262Driver: init not yet implemented — \
            pending hardware validation. Use SimulatedLoRaRadio for tests.",
        ))
    }

    fn tx_payload(&mut self, _payload: &[u8]) -> Result<(), LoRaError> {
        Err(LoRaError::Driver("Sx1262Driver: tx_payload stub"))
    }

    fn rx_payload(&mut self, _buf: &mut [u8], _timeout_ms: u32) -> Result<usize, LoRaError> {
        Err(LoRaError::Driver("Sx1262Driver: rx_payload stub"))
    }

    fn max_payload(&self) -> usize {
        255
    }
}
