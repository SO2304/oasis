//! Node C — terminal receiver in the 3-hop chain. Receives the envelope
//! that Node B forwarded, calls `MeshRouter::process()`, asserts the
//! signature still verifies even though TTL was decremented and hops
//! incremented by Node B mid-route. Proves origin-sig integrity across
//! a real intermediate-MCU forward over UART hardware.

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
const SEED_C: [u8; 32] = [0x0Cu8; 32];
const FP_A: [u8; 8] = [0xAA, 0, 0, 0, 0, 0, 0, 0];
const FP_C: [u8; 8] = [0xCC, 0, 0, 0, 0, 0, 0, 0];

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

    let in_pins = (gpioa.pa2.into_alternate(), gpioa.pa3.into_alternate());
    let bus_in = Serial::new(
        dp.USART2,
        in_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    )
    .unwrap();
    let (_in_tx, mut in_rx) = bus_in.split();

    writeln!(log, "").ok();
    writeln!(log, "[Node C / Terminal] OASIS M11 multi-hop receiver").ok();
    writeln!(log, "  USART2 = inbound from B").ok();

    // Trust A's pubkey directly. C does NOT have B's pubkey — proves
    // we're verifying the ORIGIN's sig, not the relay's.
    let pk_a = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEED_A)).unwrap();
    let mut registry = MeshPubRegistry::new();
    registry.insert(FP_A, pk_a);
    let mut router = MeshRouter::new_ed25519_signed(FP_C, MeshEdSeed(SEED_C), registry);
    writeln!(log, "  router built, awaiting forwarded frame").ok();

    let mut sync = 0u8;
    loop {
        let b: u8 = nb::block!(in_rx.read()).unwrap();
        let want = match sync {
            0 => 0xFA,
            1 => 0xCE,
            2 => 0xBE,
            3 => 0xEF,
            _ => 0,
        };
        if b == want {
            sync += 1;
        } else {
            sync = 0;
        }
        if sync == 4 {
            break;
        }
    }
    let lh: u8 = nb::block!(in_rx.read()).unwrap();
    let ll: u8 = nb::block!(in_rx.read()).unwrap();
    let len = ((lh as u16) << 8) | (ll as u16);
    let mut env: Vec<u8> = Vec::with_capacity(len as usize);
    for _ in 0..len {
        env.push(nb::block!(in_rx.read()).unwrap());
    }

    let ttl_in = env[22];
    let hops_in = env[23];
    writeln!(
        log,
        "  RX {} bytes  TTL={} hops={}",
        env.len(),
        ttl_in,
        hops_in
    )
    .ok();
    writeln!(
        log,
        "  origin_fp = {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        env[14], env[15], env[16], env[17], env[18], env[19], env[20], env[21]
    )
    .ok();

    match router.process(&env) {
        MeshDecision::Arrived {
            msg_id,
            hops_seen,
            forward,
            ..
        } => {
            writeln!(
                log,
                "  [VERIFIED] msg_id=0x{:x} hops_seen={} forward={}",
                msg_id, hops_seen, forward
            )
            .ok();
            writeln!(
                log,
                "  ORIGIN-SIG SURVIVED THE RELAY — multi-hop attestation OK"
            )
            .ok();
            writeln!(log, "[Node C] M11 MULTI-HOP RECV COMPLETE — VERIFIED").ok();
        }
        MeshDecision::Drop(reason) => {
            writeln!(log, "[Node C] DROP: {}", reason).ok();
        }
    }

    loop {
        cortex_m::asm::nop();
    }
}
