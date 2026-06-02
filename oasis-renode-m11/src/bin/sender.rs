//! Node A — signs a SPORE v0A envelope and transmits it on USART2.
//!
//! Log output on USART1 (Renode captures for shadow audit).
//! Mesh bytes on USART2 (wired to Node B's USART2 RX in the .resc script).

#![no_std]
#![no_main]

extern crate alloc;

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use panic_halt as _;
use stm32f4xx_hal::{
    pac,
    prelude::*,
    serial::{Config, Serial},
};

use oasis_rt::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed, MeshPubRegistry, MeshRouter};

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 80 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

// Fixed identity seeds — determinism for the shadow audit. On real
// deployment these would come from a hardware RNG at provisioning.
const SEED_A: [u8; 32] = [0x0Au8; 32];
const SEED_B: [u8; 32] = [0x0Bu8; 32];
const FP_A: [u8; 8] = [0xAA, 0, 0, 0, 0, 0, 0, 0];
const FP_B: [u8; 8] = [0xBB, 0, 0, 0, 0, 0, 0, 0];

#[entry]
fn main() -> ! {
    unsafe {
        HEAP.init(HEAP_MEM.as_mut_ptr() as usize, HEAP_SIZE);
    }

    let dp = pac::Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    // Use HSI (16 MHz internal) — Renode's STM32F4 platform may not
    // simulate HSE startup reliably. Firmware correctness is what we
    // demonstrate, not peak clock performance.
    let clocks = rcc.cfgr.sysclk(16.MHz()).freeze();

    let gpioa = dp.GPIOA.split();

    // USART1 = host log (Renode analyzer captures this)
    let log_pins = (gpioa.pa9.into_alternate(), gpioa.pa10.into_alternate());
    let mut log = Serial::new(
        dp.USART1,
        log_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    )
    .unwrap();

    // USART2 = mesh bus, wired to Node B's USART2 RX by Renode .resc
    let bus_pins = (gpioa.pa2.into_alternate(), gpioa.pa3.into_alternate());
    let bus = Serial::new(
        dp.USART2,
        bus_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    )
    .unwrap();
    let (mut bus_tx, _bus_rx) = bus.split();

    writeln!(log, "").ok();
    writeln!(log, "[Node A / Sender] OASIS M11 Federated Resonance demo").ok();
    writeln!(
        log,
        "  target : thumbv7em-none-eabihf (Cortex-M4F, STM32F407)"
    )
    .ok();
    writeln!(log, "  sim    : Renode (multi-node, UART bridged)").ok();
    writeln!(log, "  [dbg] before pubkey derivation").ok();

    let seed_a = MeshEdSeed(SEED_A);
    let pk_b = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEED_B)).unwrap();
    writeln!(log, "  [dbg] pk_b derived").ok();

    let mut registry = MeshPubRegistry::new();
    registry.insert(FP_B, pk_b);
    let mut router = MeshRouter::new_ed25519_signed(FP_A, seed_a, registry);
    writeln!(log, "  [dbg] router built").ok();

    let payload = b"M11 federated-resonance demo payload";
    let envelope = router.origin_wrap(payload);
    writeln!(log, "  [dbg] envelope signed, len={}", envelope.len()).ok();

    writeln!(
        log,
        "  envelope: magic=SPORE\\x0A len={} payload='{}'",
        envelope.len(),
        core::str::from_utf8(payload).unwrap_or("<bytes>")
    )
    .ok();
    writeln!(
        log,
        "  sig_head: [{:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}]",
        envelope[25],
        envelope[26],
        envelope[27],
        envelope[28],
        envelope[29],
        envelope[30],
        envelope[31],
        envelope[32]
    )
    .ok();

    // Transmit a SYNC preamble (4 bytes) + 2-byte BE length + envelope,
    // so the receiver can frame the stream reliably.
    let len = envelope.len() as u16;
    let frame_hdr = [0xFAu8, 0xCE, 0xBE, 0xEF, (len >> 8) as u8, len as u8];
    for b in frame_hdr.iter() {
        nb::block!(bus_tx.write(*b)).ok();
    }
    for b in envelope.iter() {
        nb::block!(bus_tx.write(*b)).ok();
    }
    nb::block!(bus_tx.flush()).ok();

    writeln!(
        log,
        "  SENT {} bytes on USART2 (6-byte frame hdr + {}-byte envelope)",
        frame_hdr.len() + envelope.len(),
        envelope.len()
    )
    .ok();
    writeln!(log, "[Node A] M11 SEND COMPLETE").ok();

    loop {
        cortex_m::asm::nop();
    }
}
