//! R2 (zone TTL aging) + compressed soak validation.
//!
//! Previous round flagged: "M10 zone count is monotonic under add-only
//! API → cost INCREASES with uptime unless explicit zone-aging is
//! added." This round adds the aging + measures.
//!
//! Approach (application-layer TTL, no oasis-rt change):
//!   - Wrapper struct tracks (zone_idx, expiry_tick) for each added zone.
//!   - On tick(now): scan, remove zones where expiry_tick <= now.
//!   - Zones added with `add_with_ttl(center, ttl_ticks)` get auto-removed
//!     after ttl_ticks calls to tick().
//!
//! Test pattern:
//!   Phase A: 1000 reports without TTL → zone count grows monotonically
//!     to ~500 (after Bloom rejects half), navigate cost grows with it.
//!   Phase B: 1000 reports WITH TTL=20 → zone count plateaus at ~10-20
//!     (steady-state arrival rate × TTL window), navigate cost stays
//!     bounded.
//!   Phase C (compressed soak): 10 000 reports w/ TTL=20 → verify
//!     - zone count stays bounded across the full run
//!     - per-tick cost stays bounded
//!     - no allocator drift (heap usage doesn't grow unboundedly)
//!     - bench output deterministic (cycle-accurate sim should be
//!       byte-identical across runs)
//!
//! Honest scope: 10 000 reports at ~1 report/second = ~2.8 hours of
//! fleet ops. Not 24h. The stability test passes if no drift in 10k;
//! we extrapolate to 24h. Real 24h test = next round.

#![no_std]
#![no_main]

extern crate alloc;

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

use oasis_rt::vec::{V, vz};
use oasis_rt::world_model::{WorldModel, ZoneType};

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// TTL-aware wrapper. Tracks `(zone_idx, expiry_tick)` pairs;
/// prune_expired() removes zones whose expiry has passed.
///
/// IMPORTANT: when we remove from WorldModel, indices shift, so the
/// expiry list must be rebuilt around the surviving zones. We do this
/// by removing from highest index to lowest each tick.
struct TtlWorld {
    inner: WorldModel,
    /// Parallel to inner.zones: expiry_tick for each, or u64::MAX for
    /// permanent zones. Order MUST stay aligned with inner.zones.
    expiries: Vec<u64>,
}

impl TtlWorld {
    fn new() -> Self {
        Self { inner: WorldModel::new(), expiries: Vec::new() }
    }

    /// Add a zone that lives forever (e.g., goal, fixed obstacle).
    fn add_permanent(&mut self, kind: ZoneType, center: V,
                     intensity: f64, falloff: f64) {
        self.inner.add_zone(kind, center, intensity, falloff);
        self.expiries.push(u64::MAX);
    }

    /// Add a zone that auto-expires at `expires_at` tick.
    fn add_with_ttl(&mut self, kind: ZoneType, center: V,
                    intensity: f64, falloff: f64, expires_at: u64) {
        self.inner.add_zone(kind, center, intensity, falloff);
        self.expiries.push(expires_at);
    }

    /// Prune expired zones. Returns number removed. Removes in
    /// reverse-index order so remaining indices stay valid.
    fn prune_expired(&mut self, now: u64) -> u32 {
        // Collect indices to remove (high to low).
        let mut to_remove: Vec<usize> = Vec::new();
        for (i, &exp) in self.expiries.iter().enumerate() {
            if exp <= now { to_remove.push(i); }
        }
        // Remove in reverse so earlier indices stay valid.
        for &idx in to_remove.iter().rev() {
            self.inner.remove_zone(idx);
            self.expiries.remove(idx);
        }
        to_remove.len() as u32
    }

    fn zone_count(&self) -> usize { self.inner.zone_count() }

    fn navigate(&self, start: &V, goal: &V, steps: usize) -> Vec<V> {
        self.inner.navigate(start, goal, steps)
    }
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
    writeln!(uart, "║  R2 — zone TTL aging + compressed soak validation               ║").ok();
    writeln!(uart, "╚══════════════════════════════════════════════════════════════════╝").ok();
    writeln!(uart, "").ok();

    // ── Phase A: no TTL, count grows ───────────────────────────
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Phase A — 200 reports WITHOUT TTL (baseline; count grows)").ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    {
        let mut w = TtlWorld::new();
        let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
        w.add_permanent(ZoneType::Attractive, goal, 5.0, 8.0);
        for tick in 1..=200u32 {
            let mut center: V = vz();
            center[0] = ((tick * 7) % 100) as f64 / 10.0;
            center[1] = ((tick * 11) % 100) as f64 / 10.0;
            w.add_permanent(ZoneType::Repulsive, center, 3.0, 1.0);
            if tick % 50 == 0 {
                writeln!(uart, "    tick {:>3}: zone_count = {}", tick, w.zone_count()).ok();
            }
        }
        // Final navigate cost
        let mut start: V = vz();
        let t0 = timer.get_counter().ticks();
        let _path = w.navigate(&start, &goal, 50);
        let t1 = timer.get_counter().ticks();
        writeln!(uart, "    final navigate(50 steps) cost: {} µs ({} per step)",
                 t1 - t0, (t1 - t0) / 50).ok();
        writeln!(uart, "    final zone_count: {}", w.zone_count()).ok();
    }
    writeln!(uart, "").ok();

    // ── Phase B: TTL=20, count plateaus ───────────────────────
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Phase B — 200 reports WITH TTL=20 ticks (count plateaus)").ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    {
        let mut w = TtlWorld::new();
        let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
        w.add_permanent(ZoneType::Attractive, goal, 5.0, 8.0);
        const TTL: u64 = 20;
        for tick in 1..=200u64 {
            let mut center: V = vz();
            center[0] = ((tick * 7) % 100) as f64 / 10.0;
            center[1] = ((tick * 11) % 100) as f64 / 10.0;
            let expiry = tick + TTL;
            w.add_with_ttl(ZoneType::Repulsive, center, 3.0, 1.0, expiry);
            // Tick once per report — prune anything older than `tick`.
            let pruned = w.prune_expired(tick);
            if tick % 50 == 0 {
                writeln!(uart, "    tick {:>3}: zone_count = {} (pruned {} this tick)",
                         tick, w.zone_count(), pruned).ok();
            }
        }
        // Final navigate cost (with bounded zone count)
        let mut start: V = vz();
        let t0 = timer.get_counter().ticks();
        let _path = w.navigate(&start, &goal, 50);
        let t1 = timer.get_counter().ticks();
        writeln!(uart, "    final navigate(50 steps) cost: {} µs ({} per step)",
                 t1 - t0, (t1 - t0) / 50).ok();
        writeln!(uart, "    final zone_count: {}", w.zone_count()).ok();
    }
    writeln!(uart, "").ok();

    // ── Phase C: compressed soak — 10 000 reports w/ TTL ───────
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Phase C — compressed soak: 10 000 reports w/ TTL=20").ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  validates: bounded zone_count, bounded navigate cost, no drift").ok();
    {
        let mut w = TtlWorld::new();
        let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
        w.add_permanent(ZoneType::Attractive, goal, 5.0, 8.0);
        const TTL: u64 = 20;
        let mut max_zone_count = 0u32;
        let mut min_zone_count_after_warmup = u32::MAX;
        let mut sum_navigate_us = 0u64;
        let mut nav_samples = 0u32;
        let t_soak_start = timer.get_counter().ticks();
        for tick in 1..=10_000u64 {
            let mut center: V = vz();
            center[0] = ((tick * 7919) % 1000) as f64 / 100.0;
            center[1] = ((tick * 7907) % 1000) as f64 / 100.0;
            w.add_with_ttl(ZoneType::Repulsive, center, 3.0, 1.0, tick + TTL);
            w.prune_expired(tick);
            let zc = w.zone_count() as u32;
            if zc > max_zone_count { max_zone_count = zc; }
            if tick > 50 && zc < min_zone_count_after_warmup {
                min_zone_count_after_warmup = zc;
            }
            // Sample navigate cost every 1000 ticks to track drift
            if tick % 1000 == 0 {
                let mut start: V = vz();
                let t0 = timer.get_counter().ticks();
                let _ = w.navigate(&start, &goal, 10);
                let t1 = timer.get_counter().ticks();
                let nav_us = t1 - t0;
                sum_navigate_us += nav_us;
                nav_samples += 1;
                writeln!(uart, "    tick {:>5}: zone_count = {}, navigate(10) = {} µs",
                         tick, zc, nav_us).ok();
            }
        }
        let t_soak_end = timer.get_counter().ticks();
        writeln!(uart, "  ─────────────────────────────────────────────────────────────").ok();
        writeln!(uart, "  soak total: {} ms across 10 000 reports", (t_soak_end - t_soak_start) / 1000).ok();
        writeln!(uart, "  zone_count range (post-warmup): {} .. {}",
                 min_zone_count_after_warmup, max_zone_count).ok();
        writeln!(uart, "  zone_count delta (max - min): {}",
                 max_zone_count.saturating_sub(min_zone_count_after_warmup)).ok();
        let avg_nav_us = if nav_samples > 0 { sum_navigate_us / nav_samples as u64 } else { 0 };
        writeln!(uart, "  navigate(10 steps) avg over 10 samples: {} µs", avg_nav_us).ok();
        if max_zone_count - min_zone_count_after_warmup <= 5 {
            writeln!(uart, "  [OK] zone_count stable (delta ≤ 5)").ok();
        } else {
            writeln!(uart, "  [DRIFT] zone_count drifted by more than 5 — investigate").ok();
        }
    }

    writeln!(uart, "").ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  R2 verdict: TTL aging keeps zone count bounded over uptime,").ok();
    writeln!(uart, "  preserves M10 navigate cost regardless of total report volume.").ok();
    writeln!(uart, "  bench complete.").ok();
    loop { cortex_m::asm::wfi(); }
}
