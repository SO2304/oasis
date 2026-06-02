//! ATECC608B Secure Element driver — **stub with honest design notes**.
//!
//! Replace with a real driver (e.g. `atca-rs`, or a thin wrapper over
//! Microchip's CryptoAuthLib) when hardware lands. Same discipline as
//! the SX1262 stub in `oasis-lora-transport`: refuse to pretend.
//!
//! ## Why ATECC608B is the canonical first target
//!
//! - $1.50 in single-unit quantity, $0.80 at 1k+ volumes.
//! - I2C interface — works on every MCU we target (RP2040, STM32F4,
//!   STM32H7, ESP32, nRF52).
//! - Provides Ed25519 sign/verify in slot 0 (configured at provisioning).
//! - 16 key slots — supports key rotation without replacing the chip.
//! - Tamper detection: external pin (TAMP0) can be wired to a chassis
//!   switch; on assert, slot contents are zero-erased.
//! - Class-3 fault detection inside the chip (countermeasure against
//!   simple voltage / clock glitching).
//!
//! ## Wiring to RP2040 (canonical)
//!
//! ```text
//!   Pi Pico pin   ATECC608B pin   Notes
//!   ──────────────────────────────────────────────────────
//!   GP4 (I2C0 SDA)   SDA          4.7 kΩ pull-up to 3V3
//!   GP5 (I2C0 SCL)   SCL          4.7 kΩ pull-up to 3V3
//!   GND              GND
//!   3V3              VCC          2.0–5.5 V; Pico 3V3 is fine
//!   GP6              TAMP0        chassis-switch input (active-low),
//!                                 pull-down to GND when wire intact
//! ```
//!
//! ## Why a stub now and not a real impl
//!
//! - Without the chip on the bench, any real-driver code is pure
//!   speculation — voltage levels, I2C clock stretching behavior,
//!   slot-N config nuances all need physical validation.
//! - The Microchip CryptoAuthLib is 50k+ LOC of C with platform-
//!   specific HAL hooks — pulling that into a no_std Rust crate is
//!   non-trivial and locks us into a vendor library style we'd have to
//!   maintain.
//! - `atca-rs` (community Rust binding) exists but coverage of
//!   Ed25519 ops is partial as of crate version 0.x.
//! - Decision deferred to: hardware-on-bench round, ~$5 chip + 1
//!   afternoon to validate.

use crate::{SecureElement, SeError, TamperReason};

pub struct Atecc608bSe<I2C, TAMP> {
    pub i2c: I2C,
    pub tamper_pin: TAMP,
    initialized: bool,
}

impl<I2C, TAMP> Atecc608bSe<I2C, TAMP> {
    pub fn new(i2c: I2C, tamper_pin: TAMP) -> Self {
        Self { i2c, tamper_pin, initialized: false }
    }
}

impl<I2C, TAMP> SecureElement for Atecc608bSe<I2C, TAMP> {
    fn provision(&mut self, _seed: &[u8; 32]) -> Result<(), SeError> {
        // TODO: real provisioning sequence per ATECC608B datasheet § 9.1:
        //   1. Wake() + read 0x11 0x33 0x43 0x00 status
        //   2. WriteConfig zone with slot 0 = "Ed25519 sign-only"
        //   3. Lock the config zone (one-way!)
        //   4. PrivWrite slot 0 (encrypted by IO key)
        //   5. Lock the data zone
        //   6. Verify GenKey returns the expected pubkey
        //
        // Until physical validation: refuse rather than pretend.
        self.initialized = false;
        Err(SeError::Driver("Atecc608bSe: provision not yet implemented — \
            pending hardware validation. Use SimSecureElement for tests."))
    }

    fn pubkey(&self) -> Result<[u8; 32], SeError> {
        Err(SeError::Driver("Atecc608bSe: pubkey not yet implemented"))
    }

    fn sign(&self, _msg: &[u8]) -> Result<[u8; 64], SeError> {
        Err(SeError::Driver("Atecc608bSe: sign not yet implemented"))
    }

    fn wipe(&mut self, _reason: TamperReason) -> Result<(), SeError> {
        // Real path: pulse the TAMP0 pin / send a Reset+Lock command
        // sequence. Until then, this is a software-side accounting only,
        // so refuse the call to be honest.
        Err(SeError::Driver("Atecc608bSe: wipe not yet implemented"))
    }

    fn is_wiped(&self) -> bool { false }
}
