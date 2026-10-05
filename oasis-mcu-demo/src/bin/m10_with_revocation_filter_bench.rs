//! M10 demonstration + cross-layer integration with Bloom-revocation
//! filter. Shows the anti-tamper layer (Bloom) protecting the
//! autonomy layer (M10 pressure-field navigation) in vivo.
//!
//! Setup: 1 edge agent, 1 charging-station goal, initial world with 2
//! known hazard zones. A simulated sensor stream announces 30 new
//! hazard reports, each tagged with a sender fingerprint.
//!
//!   - Some senders are LEGITIMATE (their fp is NOT in revoked set)
//!     → reading accepted, added to M10 as Repulsive zone, agent
//!     re-navigates around the new hazard.
//!   - Some senders are REVOKED (compromised, captured, decommissioned)
//!     → Bloom check rejects in ~5 µs, M10 untouched, agent path
//!     unaffected.
//!
//! Without the Bloom filter: the agent would absorb all sensor
//! reports including those from compromised nodes, producing wildly
//! distorted navigation (attacker can arbitrarily manipulate
//! trajectory by injecting fake "hazards" at chosen positions).
//!
//! With the Bloom filter: only verified-legitimate senders affect the
//! world model. Demonstrates layered defense in vivo.
//!
//! Measured: per-step total cost (Bloom check + M10 navigate),
//! trajectory progression, accepted vs rejected count, R20 budget
//! verification.

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

use oasis_rt::mesh::bloom_bit_index;
use oasis_rt::vec::{vz, V};
use oasis_rt::world_model::{WorldModel, ZoneType};

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const FP_LEN: usize = 8;
const BLOOM_BYTES: usize = 2048;
const BLOOM_BITS: u64 = (BLOOM_BYTES * 8) as u64;
const BLOOM_K: u64 = 8;

/// Bloom local revocation set (16 384 bits, k=8 — sweet spot from
/// previous round at M=1000 entries).
struct BloomRev {
    bits: [u8; BLOOM_BYTES],
}

impl BloomRev {
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
            self.bits[(bit / 8) as usize] |= 1 << (bit % 8) as u8;
        }
    }

    fn contains(&self, fp: &[u8; FP_LEN]) -> bool {
        let key = Self::fp_to_u64(fp);
        for k in 0..BLOOM_K {
            let bit = bloom_bit_index(key, k, BLOOM_BITS);
            if self.bits[(bit / 8) as usize] & (1 << (bit % 8) as u8) == 0 {
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

/// Simulated sensor reports: (sender_fp_id, hazard_x, hazard_y).
/// IDs 0-100 are "trusted senders". IDs 1000-1029 are "revoked"
/// (compromised). Mix of both to exercise the filter.
fn sensor_reports() -> [(u32, f64, f64); 30] {
    [
        // Phase 1: legitimate reports
        (10, 2.0, 2.0),
        (11, 3.0, 1.5),
        (12, 1.0, 3.0),
        // Phase 2: REVOKED node tries to inject fake hazards along
        // the agent's path (between start (0,0) and goal (10,10))
        (1000, 5.0, 5.0),
        (1001, 6.0, 6.0),
        (1002, 7.0, 7.0),
        (1003, 4.0, 4.0),
        // Phase 3: more legitimate
        (13, 8.0, 2.0),
        (14, 2.0, 8.0),
        // Phase 4: revoked again
        (1004, 9.0, 9.0),
        (1005, 1.0, 9.0),
        // Phase 5: legit
        (15, 7.5, 1.0),
        (16, 1.0, 7.5),
        // Phase 6: many revoked attempts
        (1006, 5.5, 5.5),
        (1007, 5.0, 6.0),
        (1008, 6.0, 5.0),
        (1009, 4.5, 5.5),
        (1010, 5.5, 4.5),
        // Phase 7: legit final
        (17, 8.5, 8.5),
        (18, 9.0, 1.0),
        (19, 1.5, 8.0),
        // More attempted contamination
        (1011, 5.2, 5.3),
        (1012, 5.3, 5.2),
        (1013, 5.1, 5.4),
        (1014, 5.4, 5.1),
        // Final clean batch
        (20, 6.5, 0.5),
        (21, 0.5, 6.5),
        (22, 7.0, 0.5),
        (23, 0.5, 7.0),
        (24, 8.5, 0.5),
    ]
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
        "║  M10 navigation + Bloom-revocation filter (cross-layer demo)    ║"
    )
    .ok();
    writeln!(
        uart,
        "╚══════════════════════════════════════════════════════════════════╝"
    )
    .ok();
    writeln!(uart, "").ok();

    // ── Setup ──────────────────────────────────────────────────
    let mut world = WorldModel::new();
    // Initial hazards
    let mut h1: V = vz();
    h1[0] = 3.0;
    h1[1] = 5.0;
    let mut h2: V = vz();
    h2[0] = 7.0;
    h2[1] = 3.0;
    world.add_zone(ZoneType::Repulsive, h1, 4.0, 1.5);
    world.add_zone(ZoneType::Repulsive, h2, 4.0, 1.5);
    // Goal: charging station at (10, 10)
    let mut goal: V = vz();
    goal[0] = 10.0;
    goal[1] = 10.0;
    world.add_zone(ZoneType::Attractive, goal, 5.0, 8.0);

    // Local revocation set: insert 30 "compromised" fps (1000..1030)
    let mut bloom = BloomRev::new();
    for i in 1000..1030 {
        bloom.insert(&make_fp(i));
    }
    writeln!(uart, "  Setup:").ok();
    writeln!(uart, "    Initial M10 zones: 2 hazards + 1 goal").ok();
    writeln!(
        uart,
        "    Bloom revocation set: 30 compromised fps (1000..1029)"
    )
    .ok();
    writeln!(uart, "    Agent start: (0, 0)   goal: (10, 10)").ok();
    writeln!(uart, "").ok();

    let reports = sensor_reports();

    // Trajectory tracking
    let mut accepted: u32 = 0;
    let mut rejected: u32 = 0;
    let mut total_bloom_us: u64 = 0;
    let mut total_world_update_us: u64 = 0;
    let mut total_navigate_us: u64 = 0;

    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  Sensor stream — 30 reports, mix of legit and revoked senders"
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();

    for (idx, (sender_id, hx, hy)) in reports.iter().enumerate() {
        let fp = make_fp(*sender_id);

        // 1. Bloom revocation check (Gap 4 anti-tamper layer)
        let t0 = timer.get_counter().ticks();
        let revoked = bloom.contains(&fp);
        let t1 = timer.get_counter().ticks();
        total_bloom_us += t1 - t0;

        if revoked {
            rejected += 1;
            writeln!(uart, "  [{:>2}] sender fp_{:>4} → REJECTED via Bloom (would have added hazard at {}.{}, {}.{})",
                idx, sender_id,
                *hx as i32, ((*hx * 10.0) as i32) % 10,
                *hy as i32, ((*hy * 10.0) as i32) % 10).ok();
            continue;
        }

        // 2. Accept reading: add as Repulsive zone in M10 (autonomy layer)
        let t0 = timer.get_counter().ticks();
        let mut center: V = vz();
        center[0] = *hx;
        center[1] = *hy;
        world.add_zone(ZoneType::Repulsive, center, 3.0, 1.0);
        let t1 = timer.get_counter().ticks();
        total_world_update_us += t1 - t0;
        accepted += 1;
        writeln!(
            uart,
            "  [{:>2}] sender fp_{:>4} → ACCEPTED, hazard added at ({}.{}, {}.{}); zones now = {}",
            idx,
            sender_id,
            *hx as i32,
            ((*hx * 10.0) as i32) % 10,
            *hy as i32,
            ((*hy * 10.0) as i32) % 10,
            world.zone_count()
        )
        .ok();
    }

    writeln!(uart, "").ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(
        uart,
        "  Final navigate from (0, 0) to (10, 10) with full M10 model"
    )
    .ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();

    let mut start: V = vz();
    let t0 = timer.get_counter().ticks();
    let path = world.navigate(&start, &goal, 50);
    let t1 = timer.get_counter().ticks();
    total_navigate_us += t1 - t0;

    // Use squared distances — preserves ordering for our comparison,
    // avoids needing libm sqrt on no_std MCU.
    let dist_sq = |a: &V, b: &V| {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        dx * dx + dy * dy
    };
    let d_start_goal = dist_sq(&start, &goal);
    let d_end_goal = dist_sq(path.last().unwrap(), &goal);

    writeln!(uart, "  start = ({:.1}, {:.1})", start[0], start[1]).ok();
    writeln!(
        uart,
        "  end   = ({:.2}, {:.2})",
        path.last().unwrap()[0],
        path.last().unwrap()[1]
    )
    .ok();
    writeln!(uart, "  goal  = ({:.1}, {:.1})", goal[0], goal[1]).ok();
    writeln!(
        uart,
        "  start→goal² initial: {:.2}    end→goal² final: {:.2}",
        d_start_goal, d_end_goal
    )
    .ok();
    let progressed = d_end_goal < d_start_goal;
    writeln!(
        uart,
        "  agent {} (end is {} from goal than start)",
        if progressed {
            "PROGRESSED ✓"
        } else {
            "DIVERGED ✗"
        },
        if progressed { "closer" } else { "farther" }
    )
    .ok();

    // Sample a few intermediate positions
    writeln!(uart, "  trajectory waypoints (every 10 steps):").ok();
    for (i, p) in path.iter().enumerate() {
        if i % 10 == 0 || i == path.len() - 1 {
            writeln!(uart, "    step {:>2}: ({:.2}, {:.2})", i, p[0], p[1]).ok();
        }
    }

    // ── Summary ─────────────────────────────────────────────────
    writeln!(uart, "").ok();
    writeln!(
        uart,
        "══════════════════════════════════════════════════════════════════"
    )
    .ok();
    writeln!(uart, "  Summary").ok();
    writeln!(
        uart,
        "──────────────────────────────────────────────────────────────────"
    )
    .ok();
    writeln!(uart, "  Sensor reports processed:  {}", reports.len()).ok();
    writeln!(uart, "    accepted (legit):        {}", accepted).ok();
    writeln!(uart, "    rejected (Bloom-revoked): {}", rejected).ok();
    writeln!(
        uart,
        "  Bloom check total:         {} µs ({} avg per report)",
        total_bloom_us,
        total_bloom_us / reports.len() as u64
    )
    .ok();
    writeln!(
        uart,
        "  M10 zone-add total:        {} µs ({} avg per accepted)",
        total_world_update_us,
        if accepted > 0 {
            total_world_update_us / accepted as u64
        } else {
            0
        }
    )
    .ok();
    writeln!(
        uart,
        "  M10 navigate (50 steps):   {} µs ({} per step)",
        total_navigate_us,
        total_navigate_us / 50
    )
    .ok();
    writeln!(uart, "").ok();

    let r20_us: u64 = 1000;
    let bloom_avg = total_bloom_us / reports.len() as u64;
    let nav_per_step = total_navigate_us / 50;
    writeln!(uart, "  R20 1 ms budget verification:").ok();
    writeln!(
        uart,
        "    Bloom per check:    {} µs vs {} µs budget   {}",
        bloom_avg,
        r20_us,
        if bloom_avg <= r20_us {
            "[OK]"
        } else {
            "[FAIL]"
        }
    )
    .ok();
    writeln!(
        uart,
        "    M10 per nav step:   {} µs vs {} µs budget   {}",
        nav_per_step,
        r20_us,
        if nav_per_step <= r20_us {
            "[OK]"
        } else {
            "[FAIL]"
        }
    )
    .ok();
    writeln!(uart, "").ok();
    writeln!(
        uart,
        "  Without the Bloom filter, the agent would have absorbed"
    )
    .ok();
    writeln!(
        uart,
        "  {} attacker-injected hazards near the agent's path,",
        rejected
    )
    .ok();
    writeln!(
        uart,
        "  arbitrarily distorting M10's trajectory. With Bloom +"
    )
    .ok();
    writeln!(
        uart,
        "  revocation cascade, the autonomy layer is protected."
    )
    .ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  bench complete.").ok();

    loop {
        cortex_m::asm::wfi();
    }
}
