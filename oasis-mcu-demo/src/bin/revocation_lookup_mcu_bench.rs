//! MCU bench — validates the deferred prediction P4 from the iterator
//! shadow audit:
//!
//!   On Cortex-M0+ at 125 MHz (RP2040, no FPU, no_std HashSet aliased
//!   to BTreeSet), the access pattern matters:
//!
//!     Pattern A  (per-fleet-fp is_revoked) — predicted ~10 ms at
//!                10k×100 fleet × rev → eats R20's 1 ms budget
//!     Pattern B  (iter-merge to local set) — predicted ~20 µs at
//!                same scale → R20 untouched
//!
//! This binary times both patterns at three scales (100×100, 1k×100,
//! and the largest that fits in RP2040 SRAM headroom) using the
//! RP2040 TIMER (1 µs resolution). UART output captured via Wokwi.
//!
//! Heap budget: OASIS mcu-demo currently runs at 192 KiB heap. A
//! RevocationList with 10k entries is ~160 KiB by itself + index +
//! local Vec, which exceeds budget. We scope the largest tier to fit.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use fugit::RateExtU32;
use panic_halt as _;
use rp_pico::hal::{
    clocks::init_clocks_and_plls,
    pac,
    sio::Sio,
    timer::Timer,
    uart::{DataBits, StopBits, UartConfig, UartPeripheral},
    watchdog::Watchdog,
    Clock,
};
use rp_pico::{hal, XOSC_CRYSTAL_FREQ};

use oasis_rt::spore_crypto::RevocationList;

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const FP_LEN: usize = 8;

fn make_fp(i: u32) -> [u8; FP_LEN] {
    let mut fp = [0u8; FP_LEN];
    fp[0..4].copy_from_slice(&i.to_le_bytes());
    fp
}

#[entry]
fn main() -> ! {
    unsafe {
        HEAP.init(HEAP_MEM.as_mut_ptr() as usize, HEAP_SIZE);
    }

    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();
    let sio = Sio::new(pac.SIO);
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let uart_pins = (
        pins.gpio0.into_function::<hal::gpio::FunctionUart>(),
        pins.gpio1.into_function::<hal::gpio::FunctionUart>(),
    );
    let mut uart = UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    writeln!(uart, "").ok();
    writeln!(
        uart,
        "╔══════════════════════════════════════════════════════════════════╗"
    )
    .ok();
    writeln!(
        uart,
        "║  Revocation lookup MCU bench — Cortex-M0+ @ 125 MHz             ║"
    )
    .ok();
    writeln!(
        uart,
        "║  validates P4 from iterator shadow audit                        ║"
    )
    .ok();
    writeln!(
        uart,
        "╚══════════════════════════════════════════════════════════════════╝"
    )
    .ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  RP2040 TIMER resolution: 1 µs").ok();
    writeln!(uart, "  Heap budget: {} KiB", HEAP_SIZE / 1024).ok();
    writeln!(
        uart,
        "  no_std uses BTreeSet alias for HashSet — O(log M) lookups"
    )
    .ok();
    writeln!(uart, "").ok();

    // Three tiers — increasing rev list size, fixed fleet at 1024.
    // Larger sizes blow the 192 KiB heap; honest about that.
    for &(fleet_size, rev_count) in &[(100u32, 100u32), (1024, 100), (1024, 1000)] {
        run_bench(&mut uart, &timer, fleet_size, rev_count);
    }

    writeln!(uart, "").ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(uart, "  P4 prediction validation:").ok();
    writeln!(uart, "    Predicted (host arithmetic):").ok();
    writeln!(
        uart,
        "      Pattern A at 10k × 100  ≈ 10 ms (eats R20 1ms budget)"
    )
    .ok();
    writeln!(
        uart,
        "      Pattern B at 10k × 100  ≈ 20 µs (R20 untouched)"
    )
    .ok();
    writeln!(
        uart,
        "    Measured: see tier-3 (1024 × 1000) above — extrapolate to 10k"
    )
    .ok();
    writeln!(
        uart,
        "    by ~10× (linear in fleet for A, constant for B's per-env path)"
    )
    .ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  R20 status: see ratio (a_us / b_us) per tier.").ok();
    writeln!(uart, "  bench complete.").ok();

    loop {
        cortex_m::asm::wfi();
    }
}

fn run_bench<U: core::fmt::Write>(uart: &mut U, timer: &Timer, fleet_size: u32, rev_count: u32) {
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  Tier: fleet = {}, rev_count = {}",
        fleet_size, rev_count
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();

    // Build the RevocationList
    let t_build_start = timer.get_counter().ticks();
    let mut rev = RevocationList::new();
    for i in 0..rev_count {
        rev.revoke(make_fp(i), 1746883200 + i as u64);
    }
    let t_build_end = timer.get_counter().ticks();
    writeln!(
        uart,
        "  build RevocationList ({} entries): {} µs",
        rev_count,
        t_build_end - t_build_start
    )
    .ok();

    // Build the fleet fp list
    let mut fleet: Vec<[u8; FP_LEN]> = Vec::with_capacity(fleet_size as usize);
    for i in 0..fleet_size {
        fleet.push(make_fp(i));
    }

    // ── Pattern A — per-fleet-fp is_revoked() lookup ─────────────
    let t_a_start = timer.get_counter().ticks();
    let mut a_hits: u32 = 0;
    for fp in &fleet {
        if rev.is_revoked(fp) {
            a_hits += 1;
        }
    }
    let t_a_end = timer.get_counter().ticks();
    let a_us = t_a_end - t_a_start;
    writeln!(
        uart,
        "  Pattern A (per-fleet is_revoked): {} µs  (hits={})",
        a_us, a_hits
    )
    .ok();

    // ── Pattern B — iter-merge to local set, then intersect ──────
    use alloc::collections::BTreeSet;
    let t_b_start = timer.get_counter().ticks();
    let mut local: BTreeSet<[u8; FP_LEN]> = BTreeSet::new();
    for fp in rev.fingerprints() {
        local.insert(*fp);
    }
    let mut b_hits: u32 = 0;
    for fp in &fleet {
        if local.contains(fp) {
            b_hits += 1;
        }
    }
    let t_b_end = timer.get_counter().ticks();
    let b_us = t_b_end - t_b_start;
    writeln!(
        uart,
        "  Pattern B (iter-merge + lookup):  {} µs  (hits={})",
        b_us, b_hits
    )
    .ok();

    // Sanity: same intersection count
    if a_hits != b_hits {
        writeln!(
            uart,
            "  [WARN] hit-count mismatch: A={} B={}",
            a_hits, b_hits
        )
        .ok();
    } else {
        writeln!(uart, "  [OK]  intersection counts agree ({})", a_hits).ok();
    }

    // Speedup ratio (A µs / B µs); bigger means Pattern B wins more
    if b_us > 0 {
        let ratio_x100 = (a_us * 100) / b_us;
        writeln!(
            uart,
            "  ratio A/B = {}.{:02}× ({} µs / {} µs)",
            ratio_x100 / 100,
            ratio_x100 % 100,
            a_us,
            b_us
        )
        .ok();
    }

    // R20 budget check
    let r20_us: u64 = 1000;
    if a_us > r20_us {
        writeln!(
            uart,
            "  [R20] Pattern A EXCEEDS 1ms budget by {}×",
            a_us / r20_us
        )
        .ok();
    } else {
        writeln!(
            uart,
            "  [R20] Pattern A within budget ({} µs ≤ 1000 µs)",
            a_us
        )
        .ok();
    }
    if b_us > r20_us {
        writeln!(uart, "  [R20] Pattern B EXCEEDS 1ms budget").ok();
    } else {
        writeln!(
            uart,
            "  [R20] Pattern B within budget ({} µs ≤ 1000 µs)",
            b_us
        )
        .ok();
    }
    writeln!(uart, "").ok();
}
