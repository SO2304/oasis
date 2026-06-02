//! Node B — relay node. Receives a v0A envelope on USART2, calls
//! `MeshRouter::process()` (which verifies the Ed25519 sig, decrements
//! TTL, increments hops), and if `forward = true` re-emits the
//! envelope on USART3 to the next hop.
//!
//! This proves on real MCUs the same property as the host test
//! `v10_signed_roundtrip_origin_to_hop`: the origin's signature
//! survives a TTL/hops mutation by an intermediate node.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use panic_halt as _;
use stm32f4xx_hal::{pac, prelude::*, serial::{Config, Serial}};

use oasis_rt::mesh::{MeshDecision, MeshEdSeed, MeshRouter, MeshPubRegistry,
                     mesh_v10_pubkey_from_seed};

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
    unsafe { HEAP.init(HEAP_MEM.as_mut_ptr() as usize, HEAP_SIZE); }

    let dp = pac::Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(16.MHz()).freeze();

    let gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();

    // USART1 = host log
    let log_pins = (gpioa.pa9.into_alternate(), gpioa.pa10.into_alternate());
    let mut log = Serial::new(
        dp.USART1, log_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    ).unwrap();

    // USART2 = inbound from previous hop
    let in_pins = (gpioa.pa2.into_alternate(), gpioa.pa3.into_alternate());
    let bus_in = Serial::new(
        dp.USART2, in_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    ).unwrap();
    let (_in_tx, mut in_rx) = bus_in.split();

    // USART3 = outbound to next hop (PB10 TX, PB11 RX)
    let out_pins = (gpiob.pb10.into_alternate(), gpiob.pb11.into_alternate());
    let bus_out = Serial::new(
        dp.USART3, out_pins,
        Config::default().baudrate(115200.bps()),
        &clocks,
    ).unwrap();
    let (mut out_tx, _out_rx) = bus_out.split();

    writeln!(log, "").ok();
    writeln!(log, "[Node B / Forwarder] OASIS M11 multi-hop relay").ok();
    writeln!(log, "  USART2 = inbound from A; USART3 = outbound to C").ok();

    // Build router. Registry contains A's pubkey so we can verify A's sigs.
    let pk_a = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEED_A)).unwrap();
    let mut registry = MeshPubRegistry::new();
    registry.insert(FP_A, pk_a);
    let mut router = MeshRouter::new_ed25519_signed(FP_B, MeshEdSeed(SEED_B), registry);
    writeln!(log, "  router built, awaiting frame on USART2").ok();

    // Sync on FA CE BE EF magic
    let mut sync = 0u8;
    loop {
        let b: u8 = nb::block!(in_rx.read()).unwrap();
        let want = match sync { 0 => 0xFA, 1 => 0xCE, 2 => 0xBE, 3 => 0xEF, _ => 0 };
        if b == want { sync += 1; } else { sync = 0; }
        if sync == 4 { break; }
    }
    let lh: u8 = nb::block!(in_rx.read()).unwrap();
    let ll: u8 = nb::block!(in_rx.read()).unwrap();
    let len = ((lh as u16) << 8) | (ll as u16);
    let mut env: Vec<u8> = Vec::with_capacity(len as usize);
    for _ in 0..len { env.push(nb::block!(in_rx.read()).unwrap()); }

    writeln!(log, "  RX {} bytes", env.len()).ok();
    let ttl_in = env[22];
    let hops_in = env[23];
    writeln!(log, "  inbound TTL={} hops={}", ttl_in, hops_in).ok();

    match router.process(&env) {
        MeshDecision::Arrived { envelope, msg_id, hops_seen, forward, .. } => {
            let ttl_out = envelope[22];
            let hops_out = envelope[23];
            writeln!(log, "  [VERIFIED] msg_id=0x{:x} hops_seen={} forward={}",
                     msg_id, hops_seen, forward).ok();
            writeln!(log, "  outbound TTL={} hops={}", ttl_out, hops_out).ok();
            if forward {
                let n = envelope.len() as u16;
                let hdr = [0xFAu8, 0xCE, 0xBE, 0xEF, (n >> 8) as u8, n as u8];
                for b in hdr.iter() { nb::block!(out_tx.write(*b)).ok(); }
                for b in envelope.iter() { nb::block!(out_tx.write(*b)).ok(); }
                nb::block!(out_tx.flush()).ok();
                writeln!(log, "[Node B] FORWARDED to USART3").ok();
            } else {
                writeln!(log, "[Node B] TTL=0 — terminal hop, no forward").ok();
            }
        }
        MeshDecision::Drop(reason) => {
            writeln!(log, "[Node B] DROP: {}", reason).ok();
        }
    }

    loop { cortex_m::asm::nop(); }
}
