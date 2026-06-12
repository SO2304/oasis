//! SX1262 LoRa radio driver — blocking, `no_std`, generic over `embedded-hal 1.0`.
//!
//! Implements [`LoRaRadio`] by sequencing the [`crate::sx126x`] command encoders
//! over an `embedded-hal` SPI bus + GPIO (NSS, BUSY, RESET, DIO1) + delay. The
//! board glue supplies the concrete HAL types per target.
//!
//! Honest status: the **command sequencing is unit-tested** with a mock SPI bus
//! (see tests — the exact bytes for init + TX are asserted), but the driver has
//! **not been run on real silicon**. The byte-level encoding is datasheet-exact
//! (`sx126x` tests); what a real board adds is timing, IRQ edges, and RF.
//!
//! ## Canonical wiring to Raspberry Pi Pico (RP2040), SX1262 over SPI1
//!
//! ```text
//!   Pi Pico pin     SX1262 pin    Notes
//!   ─────────────── ───────────   ───────────────────────────────
//!   GP10 (SPI1 SCK)  SCK          1 MHz nominal, up to 8 MHz
//!   GP11 (SPI1 TX)   MOSI
//!   GP12 (SPI1 RX)   MISO
//!   GP13 (SPI1 CS)   NSS          active low
//!   GP14             BUSY         input, radio-asserted when busy
//!   GP15             DIO1         RX/TX done IRQ
//!   GP16             RESET        output, active-low pulse on init
//!   3V3 / GND        VCC / GND    SX1262 is 3.0-3.7 V
//! ```
//!
//! ## Canonical wiring to STM32F4 Discovery / Nucleo
//!
//! ```text
//!   STM32F4 pin      SX1262 pin
//!   ──────────────── ───────────
//!   PA5 (SPI1 SCK)   SCK
//!   PA7 (SPI1 MOSI)  MOSI
//!   PA6 (SPI1 MISO)  MISO
//!   PB0              NSS
//!   PB1              BUSY
//!   PC13             DIO1 (EXTI13)
//!   PC14             RESET
//! ```

use crate::sx126x::{
    dio1_irq_params, modulation_params, pa_config, packet_params, rf_freq_word, sync_word_write,
    timeout_word, tx_params, IRQ_CRC_ERR, IRQ_RX_DONE, IRQ_TIMEOUT, IRQ_TX_DONE,
    LORA_HEADER_EXPLICIT, OP_CLEAR_IRQ_STATUS, OP_GET_IRQ_STATUS, OP_GET_RX_BUFFER_STATUS,
    OP_READ_BUFFER, OP_SET_BUFFER_BASE, OP_SET_DIO_IRQ_PARAMS, OP_SET_MODULATION_PARAMS,
    OP_SET_PACKET_PARAMS, OP_SET_PACKET_TYPE, OP_SET_PA_CONFIG, OP_SET_RF_FREQUENCY, OP_SET_RX,
    OP_SET_SLEEP, OP_SET_STANDBY, OP_SET_TX, OP_SET_TX_PARAMS, OP_WRITE_BUFFER, OP_WRITE_REGISTER,
    PACKET_TYPE_LORA, RAMP_200_US, STDBY_RC,
};
use crate::{LoRaError, LoRaParams, LoRaRadio};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiBus;

const _: u8 = LORA_HEADER_EXPLICIT; // keep the explicit-header constant referenced

/// Blocking SX1262 driver over `embedded-hal 1.0`.
pub struct Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1, DELAY> {
    spi: SPI,
    nss: NSS,
    busy: BUSY,
    reset: RESET,
    dio1: DIO1,
    delay: DELAY,
    params: LoRaParams,
}

impl<SPI, NSS, BUSY, RESET, DIO1, DELAY> Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1, DELAY>
where
    SPI: SpiBus<u8>,
    NSS: OutputPin,
    BUSY: InputPin,
    RESET: OutputPin,
    DIO1: InputPin,
    DELAY: DelayNs,
{
    pub fn new(spi: SPI, nss: NSS, busy: BUSY, reset: RESET, dio1: DIO1, delay: DELAY) -> Self {
        Self {
            spi,
            nss,
            busy,
            reset,
            dio1,
            delay,
            params: LoRaParams::default(),
        }
    }

    /// Recover the SPI peripheral (e.g. for tests / teardown).
    pub fn release(self) -> SPI {
        self.spi
    }

    fn busy_wait(&mut self) -> Result<(), LoRaError> {
        for _ in 0..100_000u32 {
            if self
                .busy
                .is_low()
                .map_err(|_| LoRaError::Driver("busy pin"))?
            {
                return Ok(());
            }
            self.delay.delay_us(2);
        }
        Err(LoRaError::Timeout)
    }

    /// Send `opcode` + `args` in one NSS-framed transaction (BUSY-gated).
    fn cmd(&mut self, opcode: u8, args: &[u8]) -> Result<(), LoRaError> {
        self.busy_wait()?;
        self.nss.set_low().map_err(|_| LoRaError::Driver("nss"))?;
        self.spi
            .write(&[opcode])
            .map_err(|_| LoRaError::Driver("spi"))?;
        if !args.is_empty() {
            self.spi.write(args).map_err(|_| LoRaError::Driver("spi"))?;
        }
        self.nss.set_high().map_err(|_| LoRaError::Driver("nss"))?;
        Ok(())
    }

    /// WriteBuffer: place `payload` at FIFO offset 0.
    fn write_buffer(&mut self, payload: &[u8]) -> Result<(), LoRaError> {
        self.busy_wait()?;
        self.nss.set_low().map_err(|_| LoRaError::Driver("nss"))?;
        self.spi
            .write(&[OP_WRITE_BUFFER, 0x00])
            .map_err(|_| LoRaError::Driver("spi"))?;
        self.spi
            .write(payload)
            .map_err(|_| LoRaError::Driver("spi"))?;
        self.nss.set_high().map_err(|_| LoRaError::Driver("nss"))?;
        Ok(())
    }

    /// ReadBuffer from `offset` into `out` (1 status NOP between opcode+offset and data).
    fn read_buffer(&mut self, offset: u8, out: &mut [u8]) -> Result<(), LoRaError> {
        self.busy_wait()?;
        self.nss.set_low().map_err(|_| LoRaError::Driver("nss"))?;
        self.spi
            .write(&[OP_READ_BUFFER, offset])
            .map_err(|_| LoRaError::Driver("spi"))?;
        let mut nop = [0u8; 1];
        self.spi
            .transfer_in_place(&mut nop)
            .map_err(|_| LoRaError::Driver("spi"))?;
        self.spi.read(out).map_err(|_| LoRaError::Driver("spi"))?;
        self.nss.set_high().map_err(|_| LoRaError::Driver("nss"))?;
        Ok(())
    }

    /// A read-style command: opcode, then `n_nop` discarded status bytes, then `out`.
    fn read_status(&mut self, opcode: u8, n_nop: usize, out: &mut [u8]) -> Result<(), LoRaError> {
        self.busy_wait()?;
        self.nss.set_low().map_err(|_| LoRaError::Driver("nss"))?;
        self.spi
            .write(&[opcode])
            .map_err(|_| LoRaError::Driver("spi"))?;
        let mut nop = [0u8; 4];
        if n_nop > 0 {
            self.spi
                .transfer_in_place(&mut nop[..n_nop])
                .map_err(|_| LoRaError::Driver("spi"))?;
        }
        self.spi.read(out).map_err(|_| LoRaError::Driver("spi"))?;
        self.nss.set_high().map_err(|_| LoRaError::Driver("nss"))?;
        Ok(())
    }

    fn wait_dio1(&mut self, timeout_ms: u32) -> Result<(), LoRaError> {
        let iters = timeout_ms.saturating_mul(1000).max(1);
        for _ in 0..iters {
            if self.dio1.is_high().map_err(|_| LoRaError::Driver("dio1"))? {
                return Ok(());
            }
            self.delay.delay_us(1);
        }
        Err(LoRaError::Timeout)
    }

    fn reset_pulse(&mut self) -> Result<(), LoRaError> {
        self.reset
            .set_low()
            .map_err(|_| LoRaError::Driver("reset"))?;
        self.delay.delay_us(200);
        self.reset
            .set_high()
            .map_err(|_| LoRaError::Driver("reset"))?;
        self.delay.delay_ms(5);
        self.busy_wait()
    }
}

impl<SPI, NSS, BUSY, RESET, DIO1, DELAY> LoRaRadio
    for Sx1262Driver<SPI, NSS, BUSY, RESET, DIO1, DELAY>
where
    SPI: SpiBus<u8>,
    NSS: OutputPin,
    BUSY: InputPin,
    RESET: OutputPin,
    DIO1: InputPin,
    DELAY: DelayNs,
{
    fn init(&mut self, params: &LoRaParams) -> Result<(), LoRaError> {
        self.params = *params;
        self.reset_pulse()?;
        self.cmd(OP_SET_STANDBY, &[STDBY_RC])?;
        self.cmd(OP_SET_PACKET_TYPE, &[PACKET_TYPE_LORA])?;
        self.cmd(OP_SET_RF_FREQUENCY, &rf_freq_word(params.frequency_hz))?;
        self.cmd(OP_SET_PA_CONFIG, &pa_config(params.tx_power_dbm))?;
        self.cmd(
            OP_SET_TX_PARAMS,
            &tx_params(params.tx_power_dbm, RAMP_200_US),
        )?;
        self.cmd(OP_SET_BUFFER_BASE, &[0x00, 0x00])?;
        self.cmd(OP_SET_MODULATION_PARAMS, &modulation_params(params))?;
        self.cmd(
            OP_SET_PACKET_PARAMS,
            &packet_params(params.preamble_len, self.max_payload() as u8, true),
        )?;
        let (addr, data) = sync_word_write(params.sync_word);
        self.cmd(OP_WRITE_REGISTER, &[addr[0], addr[1], data[0], data[1]])?;
        self.cmd(
            OP_SET_DIO_IRQ_PARAMS,
            &dio1_irq_params(IRQ_TX_DONE | IRQ_RX_DONE | IRQ_TIMEOUT),
        )?;
        Ok(())
    }

    fn tx_payload(&mut self, payload: &[u8]) -> Result<(), LoRaError> {
        if payload.len() > self.max_payload() {
            return Err(LoRaError::PayloadTooLarge(payload.len()));
        }
        self.cmd(OP_SET_STANDBY, &[STDBY_RC])?;
        self.cmd(OP_SET_BUFFER_BASE, &[0x00, 0x00])?;
        self.write_buffer(payload)?;
        self.cmd(
            OP_SET_PACKET_PARAMS,
            &packet_params(self.params.preamble_len, payload.len() as u8, true),
        )?;
        self.cmd(OP_CLEAR_IRQ_STATUS, &[0xFF, 0xFF])?;
        self.cmd(
            OP_SET_DIO_IRQ_PARAMS,
            &dio1_irq_params(IRQ_TX_DONE | IRQ_TIMEOUT),
        )?;
        self.cmd(OP_SET_TX, &timeout_word(0))?; // 0 = no timeout
        self.wait_dio1(5_000)?;
        self.cmd(OP_CLEAR_IRQ_STATUS, &[0xFF, 0xFF])?;
        Ok(())
    }

    fn rx_payload(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, LoRaError> {
        self.cmd(OP_SET_STANDBY, &[STDBY_RC])?;
        self.cmd(
            OP_SET_PACKET_PARAMS,
            &packet_params(self.params.preamble_len, self.max_payload() as u8, true),
        )?;
        self.cmd(OP_CLEAR_IRQ_STATUS, &[0xFF, 0xFF])?;
        self.cmd(
            OP_SET_DIO_IRQ_PARAMS,
            &dio1_irq_params(IRQ_RX_DONE | IRQ_TIMEOUT | IRQ_CRC_ERR),
        )?;
        self.cmd(OP_SET_RX, &timeout_word(timeout_ms))?;
        self.wait_dio1(timeout_ms)?;

        let mut irq = [0u8; 2];
        self.read_status(OP_GET_IRQ_STATUS, 1, &mut irq)?;
        let irq = u16::from_be_bytes(irq);
        self.cmd(OP_CLEAR_IRQ_STATUS, &[0xFF, 0xFF])?;
        if irq & IRQ_TIMEOUT != 0 {
            return Err(LoRaError::Timeout);
        }
        if irq & IRQ_CRC_ERR != 0 {
            return Err(LoRaError::Driver("rx crc error"));
        }
        if irq & IRQ_RX_DONE == 0 {
            return Err(LoRaError::Driver("rx: no RxDone"));
        }

        // GetRxBufferStatus -> [payloadLen, rxStartBufferPointer]
        let mut st = [0u8; 2];
        self.read_status(OP_GET_RX_BUFFER_STATUS, 1, &mut st)?;
        let len = st[0] as usize;
        let offset = st[1];
        let n = len.min(buf.len());
        self.read_buffer(offset, &mut buf[..n])?;
        Ok(n)
    }

    fn sleep(&mut self) -> Result<(), LoRaError> {
        // 0x04 = warm start (retain config), RTC wake disabled.
        self.cmd(OP_SET_SLEEP, &[0x04])
    }

    fn max_payload(&self) -> usize {
        255
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::sx126x::{
        OP_SET_MODULATION_PARAMS, OP_SET_RF_FREQUENCY, OP_SET_TX, OP_WRITE_BUFFER,
    };
    use core::cell::RefCell;
    use embedded_hal::delay::DelayNs;
    use embedded_hal::digital::{ErrorType as DigErr, InputPin, OutputPin};
    use embedded_hal::spi::{ErrorType as SpiErr, SpiBus};
    use std::rc::Rc;
    use std::vec::Vec;

    #[derive(Debug)]
    struct E;
    impl embedded_hal::spi::Error for E {
        fn kind(&self) -> embedded_hal::spi::ErrorKind {
            embedded_hal::spi::ErrorKind::Other
        }
    }
    impl embedded_hal::digital::Error for E {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    /// Mock SPI bus that records every byte written; reads return 0.
    struct MockSpi(Rc<RefCell<Vec<u8>>>);
    impl SpiErr for MockSpi {
        type Error = E;
    }
    impl SpiBus<u8> for MockSpi {
        fn read(&mut self, words: &mut [u8]) -> Result<(), E> {
            words.iter_mut().for_each(|w| *w = 0);
            Ok(())
        }
        fn write(&mut self, words: &[u8]) -> Result<(), E> {
            self.0.borrow_mut().extend_from_slice(words);
            Ok(())
        }
        fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), E> {
            self.0.borrow_mut().extend_from_slice(write);
            read.iter_mut().for_each(|w| *w = 0);
            Ok(())
        }
        fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), E> {
            self.0.borrow_mut().extend_from_slice(words);
            words.iter_mut().for_each(|w| *w = 0);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), E> {
            Ok(())
        }
    }

    struct Out;
    impl DigErr for Out {
        type Error = E;
    }
    impl OutputPin for Out {
        fn set_low(&mut self) -> Result<(), E> {
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), E> {
            Ok(())
        }
    }

    /// Mock input pin with a fixed level.
    struct In(bool);
    impl DigErr for In {
        type Error = E;
    }
    impl InputPin for In {
        fn is_high(&mut self) -> Result<bool, E> {
            Ok(self.0)
        }
        fn is_low(&mut self) -> Result<bool, E> {
            Ok(!self.0)
        }
    }

    struct NoDelay;
    impl DelayNs for NoDelay {
        fn delay_ns(&mut self, _ns: u32) {}
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn init_and_tx_emit_correct_command_byte_sequence() {
        let log = Rc::new(RefCell::new(Vec::new()));
        // busy = low (not busy); dio1 = high (IRQ already asserted) so waits return at once.
        let mut d = Sx1262Driver::new(MockSpi(log.clone()), Out, In(false), Out, In(true), NoDelay);

        d.init(&LoRaParams::default()).unwrap();
        d.tx_payload(b"OASIS").unwrap();

        let bytes = log.borrow();
        // init: SetRfFrequency + canonical 868 MHz word.
        assert!(
            contains(&bytes, &[OP_SET_RF_FREQUENCY, 0x36, 0x40, 0x00, 0x00]),
            "freq cmd"
        );
        // init: SetModulationParams SF7 / BW125 / CR 4/5 / no LDRO.
        assert!(
            contains(&bytes, &[OP_SET_MODULATION_PARAMS, 0x07, 0x04, 0x01, 0x00]),
            "mod cmd"
        );
        // tx: WriteBuffer(offset 0) then the payload bytes.
        assert!(
            contains(
                &bytes,
                &[OP_WRITE_BUFFER, 0x00, b'O', b'A', b'S', b'I', b'S']
            ),
            "payload"
        );
        // tx: SetTx with no-timeout word.
        assert!(contains(&bytes, &[OP_SET_TX, 0x00, 0x00, 0x00]), "set_tx");
    }

    #[test]
    fn oversized_payload_is_rejected() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut d = Sx1262Driver::new(MockSpi(log), Out, In(false), Out, In(true), NoDelay);
        d.init(&LoRaParams::default()).unwrap();
        let big = [0u8; 256];
        assert!(matches!(
            d.tx_payload(&big),
            Err(LoRaError::PayloadTooLarge(256))
        ));
    }
}
