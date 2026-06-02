//! Per-envelope steady-state revocation check bench (M1 from previous
//! shadow audit).
//!
//! The previous bench measured PER-MERGE cost (rare event when a
//! revocation broadcast arrives). This bench measures the
//! STEADY-STATE PER-ENVELOPE cost (hot path: every received envelope
//! gets a revocation check). The R20 1 ms budget applies to this
//! steady-state path, NOT to the merge.
//!
//! Setup:
//!   - Build a BTreeSet local revocation set ONCE with M entries
//!   - Loop N envelope checks against the local set
//!   - Measure total elapsed / N for per-check median
//!   - K=10 trials for statistical bands
//!
//! Expected (per the M1 prediction):
//!   - O(log M) BTreeSet lookup at M=1000 ≈ 10 µs per check on M0+
//!   - R20 1 ms budget = 100× headroom even at M=1000
//!
//! Validates that R20 is genuinely safe in steady-state operation,
//! correcting the previous round's worry that "both patterns exceed
//! R20 1ms" — true for the merge, NOT true for per-envelope.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use fugit::RateExtU32;
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
use panic_halt as _;

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const FP_LEN: usize = 8;
const N_CHECKS_PER_TRIAL: u32 = 10_000;
const K_TRIALS: u32 = 10;

fn make_fp(i: u32) -> [u8; FP_LEN] {
    let mut fp = [0u8; FP_LEN];
    fp[0..4].copy_from_slice(&i.to_le_bytes());
    fp
}

#[entry]
fn main() -> ! {
    unsafe { HEAP.init(HEAP_MEM.as_mut_ptr() as usize, HEAP_SIZE); }

    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let clocks = init_clocks_and_plls(XOSC_CRYSTAL_FREQ, pac.XOSC, pac.CLOCKS,
        pac.PLL_SYS, pac.PLL_USB, &mut pac.RESETS, &mut watchdog).ok().unwrap();
    let sio = Sio::new(pac.SIO);
    let pins = rp_pico::Pins::new(pac.IO_BANK0, pac.PADS_BANK0,
        sio.gpio_bank0, &mut pac.RESETS);
    let uart_pins = (
        pins.gpio0.into_function::<hal::gpio::FunctionUart>(),
        pins.gpio1.into_function::<hal::gpio::FunctionUart>(),
    );
    let mut uart = UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq()).unwrap();
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    writeln!(uart, "").ok();
    writeln!(uart, "╔══════════════════════════════════════════════════════════════════╗").ok();
    writeln!(uart, "║  Per-envelope steady-state revocation bench — M1 validation     ║").ok();
    writeln!(uart, "║  measures the HOT PATH: revocation check per envelope received  ║").ok();
    writeln!(uart, "╚══════════════════════════════════════════════════════════════════╝").ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  Method: build BTreeSet once with M entries, loop {} checks,",
             N_CHECKS_PER_TRIAL).ok();
    writeln!(uart, "          K={} trials, report min/median/max per-check ns.", K_TRIALS).ok();
    writeln!(uart, "  R20 budget: 1 ms = 1 000 000 ns per envelope.").ok();
    writeln!(uart, "").ok();

    for &m in &[100u32, 1000, 4000] {
        run_steady_state_bench(&mut uart, &timer, m);
    }

    writeln!(uart, "").ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  M1 verdict: see per-check medians above vs R20 budget 1 000 000 ns").ok();
    writeln!(uart, "  bench complete.").ok();

    loop { cortex_m::asm::wfi(); }
}

fn run_steady_state_bench<U: core::fmt::Write>(
    uart: &mut U,
    timer: &Timer,
    rev_count: u32,
) {
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Steady-state: rev_count = {} (local BTreeSet)", rev_count).ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();

    // Build local set ONCE — cost not counted against per-envelope.
    let mut local: BTreeSet<[u8; FP_LEN]> = BTreeSet::new();
    for i in 0..rev_count {
        local.insert(make_fp(i));
    }

    // K trials. Each trial: N_CHECKS_PER_TRIAL contains() calls,
    // alternating between hits (in set) and misses (out of set) to
    // exercise both paths.
    let mut per_check_ns_samples: Vec<u64> = Vec::with_capacity(K_TRIALS as usize);

    for trial in 0..K_TRIALS {
        let t_start = timer.get_counter().ticks();
        let mut hits: u32 = 0;
        for i in 0..N_CHECKS_PER_TRIAL {
            // Mix of hits and misses to model real fleet traffic where
            // most envelopes are NOT from revoked nodes.
            // 1 in 10 is from a revoked fp (i.e., expected to hit).
            let probe_id = if i % 10 == 0 {
                i % rev_count               // hit
            } else {
                rev_count + (i % 1000)      // guaranteed miss
            };
            if local.contains(&make_fp(probe_id)) {
                hits += 1;
            }
        }
        let t_end = timer.get_counter().ticks();
        let elapsed_us = t_end - t_start;
        let elapsed_ns = elapsed_us * 1000;
        let per_check_ns = elapsed_ns / N_CHECKS_PER_TRIAL as u64;
        per_check_ns_samples.push(per_check_ns);
        writeln!(uart, "    trial {}: {} ns/check  ({} hits, {} µs total)",
                 trial, per_check_ns, hits, elapsed_us).ok();
    }

    // Compute min / median / max
    per_check_ns_samples.sort();
    let min = per_check_ns_samples[0];
    let median = per_check_ns_samples[K_TRIALS as usize / 2];
    let max = per_check_ns_samples[K_TRIALS as usize - 1];
    writeln!(uart, "").ok();
    writeln!(uart, "  K=10 bands: min = {} ns/check, median = {} ns/check, max = {} ns/check",
             min, median, max).ok();
    let r20_ns: u64 = 1_000_000;
    let headroom = r20_ns / median.max(1);
    writeln!(uart, "  R20 budget headroom (median): {}× ({} ns measured vs {} ns budget)",
             headroom, median, r20_ns).ok();
    if median <= r20_ns {
        writeln!(uart, "  [R20 OK] per-envelope check fits trivially in steady state").ok();
    } else {
        writeln!(uart, "  [R20 FAIL] per-envelope check EXCEEDS budget — investigate").ok();
    }
    writeln!(uart, "").ok();
}
