//! Variant of sender — emits the SAME v0A envelope twice in succession.
//! Used to prove Bloom-filter dedup on the receiver: 1st = Arrived,
//! 2nd = Drop("duplicate").

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
    let clocks = rcc.cfgr.sysclk(16.MHz()).freeze();
    let gpioa = dp.GPIOA.split();

    let log_pins = (gpioa.pa9.into_alternate(), gpioa.pa10.into_alternate());
    let mut log = Serial::new(
        dp.USART1,
        log_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    )
    .unwrap();

    let bus_pins = (gpioa.pa2.into_alternate(), gpioa.pa3.into_alternate());
    let bus = Serial::new(
        dp.USART2,
        bus_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    )
    .unwrap();
    let (mut bus_tx, _bus_rx) = bus.split();

    writeln!(log, "[Node A / Sender×2] Bloom-dedup demo").ok();

    let pk_b = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEED_B)).unwrap();
    let mut registry = MeshPubRegistry::new();
    registry.insert(FP_B, pk_b);
    let mut router = MeshRouter::new_ed25519_signed(FP_A, MeshEdSeed(SEED_A), registry);

    // Build the envelope ONCE so both transmissions are byte-identical
    // (msg_id and signature are the same → Bloom should catch the second).
    let payload = b"bloom-dedup test payload";
    let envelope = router.origin_wrap(payload);
    writeln!(
        log,
        "  envelope built len={} msg_id_in_hdr_offset_6_to_14",
        envelope.len()
    )
    .ok();

    let n = envelope.len() as u16;
    let hdr = [0xFAu8, 0xCE, 0xBE, 0xEF, (n >> 8) as u8, n as u8];

    // 1st send
    for b in hdr.iter() {
        nb::block!(bus_tx.write(*b)).ok();
    }
    for b in envelope.iter() {
        nb::block!(bus_tx.write(*b)).ok();
    }
    nb::block!(bus_tx.flush()).ok();
    writeln!(log, "  [1st] sent {} bytes", hdr.len() + envelope.len()).ok();

    cortex_m::asm::delay(100_000);

    // 2nd send — IDENTICAL bytes
    for b in hdr.iter() {
        nb::block!(bus_tx.write(*b)).ok();
    }
    for b in envelope.iter() {
        nb::block!(bus_tx.write(*b)).ok();
    }
    nb::block!(bus_tx.flush()).ok();
    writeln!(
        log,
        "  [2nd] sent {} bytes (identical)",
        hdr.len() + envelope.len()
    )
    .ok();

    writeln!(log, "[Node A] BOTH SENDS COMPLETE").ok();

    loop {
        cortex_m::asm::nop();
    }
}
