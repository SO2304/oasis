//! L2 — Bloom filter alternative for the local revocation set.
//!
//! Compare BTreeSet (current default) vs a 16384-bit Bloom filter
//! (2 KiB, k=8 hashes via oasis-rt's SplitMix64-based bloom_bit_index)
//! across:
//!   - Per-check cost (MCU µs/op)
//!   - Memory footprint
//!   - Empirical false-positive rate (Bloom only)
//!
//! Predictions from previous audit (L2):
//!   - Bloom ~3 µs/check (no logarithm; fixed cost)
//!   - BTreeSet 8 µs/check (already measured M1 round)
//!   - FP rate ≈ 0.05% at M=1000 in 16384 bits with k=8
//!
//! Bonus L3: decompose hit-only vs miss-only paths for both
//! structures to surface the asymmetry (predicted ~5%).
//!
//! Critical security property under test:
//!   Bloom NEVER produces false negatives (revoked → always detected).
//!   Only false positives (legit fp wrongly flagged as revoked).
//!   For revocation use case, false-positive is the SAFE failure mode
//!   (over-block, never under-block).

#![no_std]
#![no_main]

extern crate alloc;

use alloc::collections::BTreeSet;
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

use oasis_rt::mesh::bloom_bit_index;

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const FP_LEN: usize = 8;
const N_CHECKS: u32 = 10_000;
const K_TRIALS: u32 = 5;

// Bloom parameters: 16384 bits = 2 KiB, k=8 hash functions.
// At M=1000: theoretical FP rate = (1 - e^(-8000/16384))^8 ≈ 0.05%
const BLOOM_BYTES: usize = 2048;
const BLOOM_BITS: u64 = (BLOOM_BYTES * 8) as u64;
const BLOOM_K: u64 = 8;

struct BloomLocal {
    bits: [u8; BLOOM_BYTES],
}

impl BloomLocal {
    fn new() -> Self {
        Self {
            bits: [0; BLOOM_BYTES],
        }
    }

    fn fp_to_u64(fp: &[u8; FP_LEN]) -> u64 {
        u64::from_le_bytes(*fp)
    }

    fn insert(&mut self, fp: &[u8; FP_LEN]) {
        let key = Self::fp_to_u64(fp);
        for k in 0..BLOOM_K {
            let bit = bloom_bit_index(key, k, BLOOM_BITS);
            let byte = (bit / 8) as usize;
            let bit_in_byte = (bit % 8) as u8;
            self.bits[byte] |= 1 << bit_in_byte;
        }
    }

    fn contains(&self, fp: &[u8; FP_LEN]) -> bool {
        let key = Self::fp_to_u64(fp);
        for k in 0..BLOOM_K {
            let bit = bloom_bit_index(key, k, BLOOM_BITS);
            let byte = (bit / 8) as usize;
            let bit_in_byte = (bit % 8) as u8;
            if self.bits[byte] & (1 << bit_in_byte) == 0 {
                return false;
            }
        }
        true
    }
}

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
        "║  L2 — Bloom vs BTreeSet local set, per-check cost + FP rate     ║"
    )
    .ok();
    writeln!(
        uart,
        "╚══════════════════════════════════════════════════════════════════╝"
    )
    .ok();
    writeln!(uart, "").ok();
    writeln!(
        uart,
        "  Bloom: {} bits ({} KiB), k={} hashes via oasis-rt SplitMix64",
        BLOOM_BITS,
        BLOOM_BYTES / 1024,
        BLOOM_K
    )
    .ok();
    writeln!(uart, "  BTreeSet: dynamic alloc on heap, O(log M) lookup").ok();
    writeln!(
        uart,
        "  Method: build once, loop {} contains() calls × K={} trials",
        N_CHECKS, K_TRIALS
    )
    .ok();
    writeln!(uart, "").ok();

    for &m in &[100u32, 1000, 4000] {
        bench_tier(&mut uart, &timer, m);
    }

    // Hit/miss decomposition (L3) at M=1000 — using BTreeSet as reference
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  L3 — hit-only vs miss-only path asymmetry (BTreeSet, M=1000)"
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    bench_hit_miss_decomposition(&mut uart, &timer, 1000);

    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(uart, "  bench complete.").ok();
    loop {
        cortex_m::asm::wfi();
    }
}

fn bench_tier<U: core::fmt::Write>(uart: &mut U, timer: &Timer, m: u32) {
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(uart, "  Tier: M = {}", m).ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();

    // ── Build BTreeSet ────────────────────────────────────────
    let mut bset: BTreeSet<[u8; FP_LEN]> = BTreeSet::new();
    for i in 0..m {
        bset.insert(make_fp(i));
    }
    let bset_bytes_est = (m as usize) * core::mem::size_of::<([u8; FP_LEN], usize, usize, usize)>();

    // ── Build Bloom ───────────────────────────────────────────
    let mut bloom = BloomLocal::new();
    for i in 0..m {
        bloom.insert(&make_fp(i));
    }

    writeln!(
        uart,
        "  BTreeSet memory ≈ {} bytes ({} per entry)",
        bset_bytes_est,
        bset_bytes_est / m.max(1) as usize
    )
    .ok();
    writeln!(uart, "  Bloom memory     = {} bytes (fixed)", BLOOM_BYTES).ok();
    let mem_ratio = if BLOOM_BYTES > 0 {
        bset_bytes_est / BLOOM_BYTES
    } else {
        0
    };
    writeln!(
        uart,
        "  memory ratio BTreeSet/Bloom = {}× at M={}",
        mem_ratio, m
    )
    .ok();

    // ── Per-check cost: BTreeSet ─────────────────────────────
    let mut bset_samples = [0u64; 5];
    for trial in 0..K_TRIALS as usize {
        let t0 = timer.get_counter().ticks();
        let mut hits = 0u32;
        for i in 0..N_CHECKS {
            let probe_id = if i % 10 == 0 { i % m } else { m + (i % 1000) };
            if bset.contains(&make_fp(probe_id)) {
                hits += 1;
            }
        }
        let t1 = timer.get_counter().ticks();
        bset_samples[trial] = ((t1 - t0) * 1000) / N_CHECKS as u64;
        let _ = hits;
    }
    bset_samples.sort();
    let bset_med = bset_samples[2];
    writeln!(
        uart,
        "  BTreeSet:  K={} median = {} ns/check",
        K_TRIALS, bset_med
    )
    .ok();

    // ── Per-check cost: Bloom ────────────────────────────────
    let mut bloom_samples = [0u64; 5];
    for trial in 0..K_TRIALS as usize {
        let t0 = timer.get_counter().ticks();
        let mut hits = 0u32;
        for i in 0..N_CHECKS {
            let probe_id = if i % 10 == 0 { i % m } else { m + (i % 1000) };
            if bloom.contains(&make_fp(probe_id)) {
                hits += 1;
            }
        }
        let t1 = timer.get_counter().ticks();
        bloom_samples[trial] = ((t1 - t0) * 1000) / N_CHECKS as u64;
        let _ = hits;
    }
    bloom_samples.sort();
    let bloom_med = bloom_samples[2];
    writeln!(
        uart,
        "  Bloom:     K={} median = {} ns/check",
        K_TRIALS, bloom_med
    )
    .ok();

    if bloom_med > 0 {
        let speedup_x100 = (bset_med * 100) / bloom_med;
        writeln!(
            uart,
            "  ratio BTreeSet/Bloom = {}.{:02}× ({} ns / {} ns)",
            speedup_x100 / 100,
            speedup_x100 % 100,
            bset_med,
            bloom_med
        )
        .ok();
    }

    // ── False-positive rate measurement (Bloom only) ─────────
    // Probe N_CHECKS fps that were NOT inserted; count how many Bloom
    // says "yes" to. That's the empirical FP rate.
    let mut fp_count = 0u32;
    let probe_start = m + 100_000; // far outside inserted range
    for i in 0..N_CHECKS {
        if bloom.contains(&make_fp(probe_start + i)) {
            fp_count += 1;
        }
    }
    let fp_per_million = (fp_count as u64 * 1_000_000) / N_CHECKS as u64;
    writeln!(
        uart,
        "  Bloom FP measured: {}/{} = {} per million ({}.{:03}%)",
        fp_count,
        N_CHECKS,
        fp_per_million,
        fp_per_million / 10000,
        (fp_per_million % 10000) / 10
    )
    .ok();

    // R20 verdict
    let r20_ns: u64 = 1_000_000;
    if bloom_med <= r20_ns && bset_med <= r20_ns {
        writeln!(
            uart,
            "  [R20 OK] both fit (BTreeSet {} ns, Bloom {} ns ≤ {} ns)",
            bset_med, bloom_med, r20_ns
        )
        .ok();
    } else {
        writeln!(uart, "  [R20 FAIL] one or both exceed budget").ok();
    }
    writeln!(uart, "").ok();
}

fn bench_hit_miss_decomposition<U: core::fmt::Write>(uart: &mut U, timer: &Timer, m: u32) {
    let mut bset: BTreeSet<[u8; FP_LEN]> = BTreeSet::new();
    for i in 0..m {
        bset.insert(make_fp(i));
    }

    // Hit-only loop
    let t0 = timer.get_counter().ticks();
    for i in 0..N_CHECKS {
        let _ = bset.contains(&make_fp(i % m));
    }
    let t1 = timer.get_counter().ticks();
    let hit_ns = ((t1 - t0) * 1000) / N_CHECKS as u64;

    // Miss-only loop
    let t0 = timer.get_counter().ticks();
    for i in 0..N_CHECKS {
        let _ = bset.contains(&make_fp(m + (i % 1000)));
    }
    let t1 = timer.get_counter().ticks();
    let miss_ns = ((t1 - t0) * 1000) / N_CHECKS as u64;

    writeln!(uart, "  hit-only:   {} ns/check", hit_ns).ok();
    writeln!(uart, "  miss-only:  {} ns/check", miss_ns).ok();
    let asym_x100 = if miss_ns < hit_ns {
        ((hit_ns - miss_ns) * 100) / hit_ns
    } else {
        ((miss_ns - hit_ns) * 100) / miss_ns.max(1)
    };
    let direction = if hit_ns > miss_ns {
        "miss path faster"
    } else {
        "hit path faster"
    };
    writeln!(
        uart,
        "  asymmetry:  {}.{:02}% ({})",
        asym_x100,
        ((((hit_ns as i64 - miss_ns as i64).unsigned_abs() * 10000) / hit_ns.max(miss_ns)) % 100),
        direction
    )
    .ok();
    writeln!(uart, "").ok();
}
