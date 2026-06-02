//! Receiver that processes TWO frames and reports the MeshDecision for
//! each. Used with `sender_twice` to demonstrate Bloom-filter dedup:
//! the 1st frame returns Arrived, the 2nd returns Drop("duplicate").

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

    let log_pins = (gpioa.pa9.into_alternate(), gpioa.pa10.into_alternate());
    let mut log = Serial::new(dp.USART1, log_pins,
        Config::default().baudrate(115200.bps()), &clocks).unwrap();

    let bus_pins = (gpioa.pa2.into_alternate(), gpioa.pa3.into_alternate());
    let bus = Serial::new(dp.USART2, bus_pins,
        Config::default().baudrate(115200.bps()), &clocks).unwrap();
    let (_bus_tx, mut bus_rx) = bus.split();

    writeln!(log, "[Node B / Bloom-check] dedup demo").ok();

    let pk_a = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEED_A)).unwrap();
    let mut registry = MeshPubRegistry::new();
    registry.insert(FP_A, pk_a);
    let mut router = MeshRouter::new_ed25519_signed(FP_B, MeshEdSeed(SEED_B), registry);
    writeln!(log, "  router ready, awaiting 2 frames on USART2").ok();

    let mut read_one_frame = |label: &str, log: &mut dyn Write| {
        let mut sync = 0u8;
        loop {
            let b: u8 = nb::block!(bus_rx.read()).unwrap();
            let want = match sync { 0 => 0xFA, 1 => 0xCE, 2 => 0xBE, 3 => 0xEF, _ => 0 };
            if b == want { sync += 1; } else { sync = 0; }
            if sync == 4 { break; }
        }
        let lh: u8 = nb::block!(bus_rx.read()).unwrap();
        let ll: u8 = nb::block!(bus_rx.read()).unwrap();
        let len = ((lh as u16) << 8) | (ll as u16);
        let mut env: Vec<u8> = Vec::with_capacity(len as usize);
        for _ in 0..len { env.push(nb::block!(bus_rx.read()).unwrap()); }

        match router.process(&env) {
            MeshDecision::Arrived { msg_id, .. } => {
                writeln!(log, "  {} -> Arrived (msg_id=0x{:x})", label, msg_id).ok();
            }
            MeshDecision::Drop(reason) => {
                writeln!(log, "  {} -> Drop({:?})", label, reason).ok();
            }
        }
    };

    read_one_frame("frame1", &mut log);
    read_one_frame("frame2", &mut log);

    writeln!(log, "[Node B] BLOOM DEDUP TEST COMPLETE").ok();
    loop { cortex_m::asm::nop(); }
}
