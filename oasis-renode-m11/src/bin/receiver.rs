//! Node B — receives a v0A envelope on USART2, verifies the Ed25519
//! signature, and prints the MeshDecision on USART1.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use panic_halt as _;
use stm32f4xx_hal::{
    pac,
    prelude::*,
    serial::{Config, Serial},
};

use oasis_rt::mesh::{
    mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
};

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
    let (_bus_tx, mut bus_rx) = bus.split();

    writeln!(log, "").ok();
    writeln!(
        log,
        "[Node B / Receiver] OASIS M11 Federated Resonance demo"
    )
    .ok();
    writeln!(
        log,
        "  target : thumbv7em-none-eabihf (Cortex-M4F, STM32F407)"
    )
    .ok();
    writeln!(log, "  sim    : Renode (multi-node, UART bridged)").ok();
    writeln!(
        log,
        "  waiting for frame header FA CE BE EF + u16 length..."
    )
    .ok();

    // Build receiver router with seed B, registry containing A's pubkey.
    let seed_b = MeshEdSeed(SEED_B);
    let pk_a = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEED_A)).unwrap();
    let mut registry = MeshPubRegistry::new();
    registry.insert(FP_A, pk_a);
    let mut router = MeshRouter::new_ed25519_signed(FP_B, seed_b, registry);

    // Blocking byte reader on USART2.
    let mut sync_state = 0u8;
    loop {
        let b: u8 = nb::block!(bus_rx.read()).unwrap();
        let expect = match sync_state {
            0 => 0xFA,
            1 => 0xCE,
            2 => 0xBE,
            3 => 0xEF,
            _ => 0,
        };
        if b == expect {
            sync_state += 1;
        } else {
            sync_state = 0;
        }
        if sync_state == 4 {
            break;
        }
    }
    writeln!(log, "  sync acquired").ok();

    let len_hi: u8 = nb::block!(bus_rx.read()).unwrap();
    let len_lo: u8 = nb::block!(bus_rx.read()).unwrap();
    let len = (u16::from(len_hi) << 8) | u16::from(len_lo);
    writeln!(log, "  frame length = {}", len).ok();

    let mut envelope: Vec<u8> = Vec::with_capacity(len as usize);
    for _ in 0..len {
        let b: u8 = nb::block!(bus_rx.read()).unwrap();
        envelope.push(b);
    }
    writeln!(log, "  RECEIVED {} bytes", envelope.len()).ok();
    writeln!(
        log,
        "  magic    = {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
        envelope[0], envelope[1], envelope[2], envelope[3], envelope[4], envelope[5]
    )
    .ok();
    writeln!(
        log,
        "  sig_head = [{:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}]",
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

    // Feed to the mesh router. v0A verification uses SEED_A's derived
    // pubkey from our registry. Returns MeshDecision::Arrived on success.
    match router.process(&envelope) {
        MeshDecision::Arrived { envelope: _, .. } => {
            writeln!(
                log,
                "  [M11] ED25519 SIGNATURE VERIFIED — federation attested"
            )
            .ok();
            writeln!(log, "[Node B] M11 RECV COMPLETE — VERIFIED").ok();
        }
        MeshDecision::Drop(reason) => {
            writeln!(log, "  [M11] DROP: {}", reason).ok();
            writeln!(log, "[Node B] M11 RECV FAILED").ok();
        }
    }

    loop {
        cortex_m::asm::nop();
    }
}
