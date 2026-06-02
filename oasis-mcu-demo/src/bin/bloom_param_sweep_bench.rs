//! O2 + O3 + O5 — Bloom parameter sweep + hit/miss decomposition.
//!
//! Validates 3 predictions from previous audit at once:
//!   O2: doubling m reduces FP at saturated M
//!   O3: optimal k for (m=16384, n=1000) is k=12 (theoretical)
//!   O5: Bloom hit/miss asymmetry REVERSES vs BTreeSet
//!       (k bits all checked on hit, can early-exit on miss)
//!
//! Method: a single 8 KiB byte buffer used as the underlying storage;
//! the bench operates on a configurable PREFIX of that buffer to
//! simulate Bloom filters of different sizes (4096, 16384, 65536 bits).
//! k is also configurable per-call. Same SplitMix64 hash from oasis-rt.
//!
//! Operators get a 3D capacity-planning matrix:
//!   (m, k, M) → (per-check ns, FP rate)

#![no_std]
#![no_main]

extern crate alloc;

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
const N_CHECKS: u32 = 5_000;
const K_TRIALS: u32 = 5;

// Max Bloom = 65536 bits = 8 KiB. We size the buffer for the largest
// case, then operate on a prefix for smaller sizes.
const MAX_BLOOM_BYTES: usize = 8 * 1024;

struct BloomParam {
    bits: [u8; MAX_BLOOM_BYTES],
    m_bits: u64,
    k: u64,
}

impl BloomParam {
    fn new() -> Self {
        Self {
            bits: [0; MAX_BLOOM_BYTES],
            m_bits: 0,
            k: 0,
        }
    }

    fn reset(&mut self, m_bits: u64, k: u64) {
        self.m_bits = m_bits;
        self.k = k;
        for b in &mut self.bits[..] {
            *b = 0;
        }
    }

    fn fp_to_u64(fp: &[u8; FP_LEN]) -> u64 {
        u64::from_le_bytes(*fp)
    }

    fn insert(&mut self, fp: &[u8; FP_LEN]) {
        let key = Self::fp_to_u64(fp);
        for k in 0..self.k {
            let bit = bloom_bit_index(key, k, self.m_bits);
            let byte = (bit / 8) as usize;
            let bit_in_byte = (bit % 8) as u8;
            self.bits[byte] |= 1 << bit_in_byte;
        }
    }

    fn contains(&self, fp: &[u8; FP_LEN]) -> bool {
        let key = Self::fp_to_u64(fp);
        for k in 0..self.k {
            let bit = bloom_bit_index(key, k, self.m_bits);
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
        "║  Bloom parameter sweep — O2 + O3 + O5 validation                ║"
    )
    .ok();
    writeln!(
        uart,
        "║  vary m (4096/16384/65536 bits), k (4/8/12), measure FP + cost  ║"
    )
    .ok();
    writeln!(
        uart,
        "╚══════════════════════════════════════════════════════════════════╝"
    )
    .ok();
    writeln!(uart, "").ok();

    let mut bloom = BloomParam::new();

    // ── O2: size sweep at saturated M=4000 ──────────────────────
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  O2 — Bloom size sweep at M=4000 (was 28.86% FP at m=16384)"
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    for &m_bits in &[16384u64, 32768, 65536] {
        sweep_one(&mut uart, &timer, &mut bloom, m_bits, 8, 4000);
    }

    // ── O3: k sweep at M=1000, m=16384 ──────────────────────────
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  O3 — k sweep at M=1000, m=16384 (predicted k=12 optimal FP)"
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    for &k in &[4u64, 8, 12] {
        sweep_one(&mut uart, &timer, &mut bloom, 16384, k, 1000);
    }

    // ── O5: hit/miss decomposition on Bloom (at M=1000, m=16384, k=8) ─
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  O5 — hit/miss decomposition on Bloom (m=16384, k=8, M=1000)"
    )
    .ok();
    writeln!(
        uart,
        "  predicted: hit traverses all k bits; miss can early-exit"
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    bloom.reset(16384, 8);
    for i in 0..1000 {
        bloom.insert(&make_fp(i));
    }

    // Hit-only loop
    let t0 = timer.get_counter().ticks();
    for i in 0..N_CHECKS {
        let _ = bloom.contains(&make_fp(i % 1000));
    }
    let t1 = timer.get_counter().ticks();
    let hit_ns = ((t1 - t0) * 1000) / N_CHECKS as u64;

    // Miss-only loop
    let t0 = timer.get_counter().ticks();
    for i in 0..N_CHECKS {
        let _ = bloom.contains(&make_fp(1000 + 100_000 + (i % 5000)));
    }
    let t1 = timer.get_counter().ticks();
    let miss_ns = ((t1 - t0) * 1000) / N_CHECKS as u64;

    writeln!(uart, "  hit-only:   {} ns/check", hit_ns).ok();
    writeln!(uart, "  miss-only:  {} ns/check", miss_ns).ok();
    let direction = if hit_ns > miss_ns {
        "miss path FASTER (early-exit on first 0-bit)"
    } else {
        "hit path FASTER"
    };
    let asym_x100 = if hit_ns > miss_ns {
        ((hit_ns - miss_ns) * 100) / hit_ns
    } else {
        ((miss_ns - hit_ns) * 100) / miss_ns.max(1)
    };
    writeln!(uart, "  asymmetry:  {}% — {}", asym_x100, direction).ok();
    writeln!(uart, "").ok();

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

fn sweep_one<U: core::fmt::Write>(
    uart: &mut U,
    timer: &Timer,
    bloom: &mut BloomParam,
    m_bits: u64,
    k: u64,
    m_load: u32,
) {
    bloom.reset(m_bits, k);
    for i in 0..m_load {
        bloom.insert(&make_fp(i));
    }

    // Per-check cost K=5 trials
    let mut samples = [0u64; 5];
    for trial in 0..K_TRIALS as usize {
        let t0 = timer.get_counter().ticks();
        let mut hits = 0u32;
        for i in 0..N_CHECKS {
            // 10% hits, 90% misses
            let probe_id = if i % 10 == 0 {
                i % m_load
            } else {
                m_load + (i % 1000)
            };
            if bloom.contains(&make_fp(probe_id)) {
                hits += 1;
            }
        }
        let t1 = timer.get_counter().ticks();
        samples[trial] = ((t1 - t0) * 1000) / N_CHECKS as u64;
        let _ = hits;
    }
    samples.sort();
    let med = samples[2];

    // Empirical FP rate
    let mut fp = 0u32;
    let probe_start = m_load + 200_000;
    for i in 0..N_CHECKS {
        if bloom.contains(&make_fp(probe_start + i)) {
            fp += 1;
        }
    }
    let fp_per_million = (fp as u64 * 1_000_000) / N_CHECKS as u64;
    let fp_pct_x10000 = (fp as u64 * 10_000_000) / N_CHECKS as u64;

    writeln!(
        uart,
        "  m={:>5} bits ({:>2} KiB), k={:>2}, M={}: med = {} ns/check, FP = {}/{} = {}.{:03}%",
        m_bits,
        m_bits as usize / 8 / 1024,
        k,
        m_load,
        med,
        fp,
        N_CHECKS,
        fp_pct_x10000 / 10000,
        (fp_pct_x10000 % 10000) / 10
    )
    .ok();
    let _ = fp_per_million;
}
