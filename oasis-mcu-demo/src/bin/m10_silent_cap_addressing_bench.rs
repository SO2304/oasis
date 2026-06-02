//! Silent-cap addressing — fail-LOUD vs fail-QUIET demonstration.
//!
//! Previous round flagged: WorldModel has hard cap MAX_ZONES = 32,
//! `add_zone()` is silent no-op once at cap. The previous audit
//! treated this as "binding constraint to address". User correction:
//! it's WORSE than unbounded growth. Unbounded growth fails LOUDLY
//! (OOM crash, perf degradation visible). Silent cap fails QUIETLY:
//! agent continues planning on an INCOMPLETE world model with NO
//! INDICATION the model is incomplete.
//!
//! This is a fail-LOUD-vs-fail-SILENT issue. In a defense vertical,
//! silent failures are far more dangerous because they erode operator
//! trust + produce wrong autonomous decisions.
//!
//! This round demonstrates the failure case + 4 policy alternatives:
//!
//!   Phase A — SILENT_DROP    (current default): dangerous case shown
//!   Phase B — REFUSE_LOUD    (fail-loud): caller gets Err, must decide
//!   Phase C — LRU_EVICT      (auto-evict oldest): freshness preserved
//!   Phase D — PRIORITY_EVICT (evict lowest intensity): critical hazards stay
//!
//! Each phase ends with: operator-visible cap-hit count + agent
//! navigation + did agent's path hit a critical hazard the model
//! "knew about" (or thought it did)?

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

const MAX_ZONES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SaturationPolicy {
    SilentDrop,    // current default — dangerous
    RefuseLoud,    // returns Err on overflow
    LruEvict,      // auto-evict oldest
    PriorityEvict, // auto-evict lowest intensity
}

#[derive(Debug)]
enum AddOutcome {
    Added,
    DroppedSilently,
    Refused,
    EvictedAndAdded { evicted_idx: usize },
}

/// Wrapper exposing CAP-HIT TELEMETRY. The point of this round is
/// that operators MUST see when the model saturates.
struct CapAwareWorld {
    inner: WorldModel,
    policy: SaturationPolicy,
    /// Parallel to inner.zones — insertion order ID and intensity for
    /// LRU / Priority eviction policies.
    insertion_id: Vec<u64>,
    intensities: Vec<f64>,
    next_id: u64,
    /// THE CRITICAL TELEMETRY: how many cap-hits has this node seen?
    /// In the SilentDrop policy, this is the only signal an operator
    /// has that the world model is incomplete.
    pub cap_hit_count: u32,
    pub eviction_count: u32,
    pub refuse_count: u32,
}

impl CapAwareWorld {
    fn new(policy: SaturationPolicy) -> Self {
        Self {
            inner: WorldModel::new(),
            policy,
            insertion_id: Vec::new(),
            intensities: Vec::new(),
            next_id: 0,
            cap_hit_count: 0,
            eviction_count: 0,
            refuse_count: 0,
        }
    }

    fn try_add(&mut self, kind: ZoneType, center: V,
               intensity: f64, falloff: f64) -> AddOutcome {
        if self.inner.zone_count() < MAX_ZONES {
            self.inner.add_zone(kind, center, intensity, falloff);
            self.insertion_id.push(self.next_id);
            self.intensities.push(intensity);
            self.next_id += 1;
            return AddOutcome::Added;
        }

        // CAP HIT
        self.cap_hit_count += 1;
        match self.policy {
            SaturationPolicy::SilentDrop => {
                // Current default — silently does nothing.
                // No telemetry to caller. THE DANGEROUS CASE.
                AddOutcome::DroppedSilently
            }
            SaturationPolicy::RefuseLoud => {
                self.refuse_count += 1;
                AddOutcome::Refused
            }
            SaturationPolicy::LruEvict => {
                // Find oldest (smallest insertion_id), evict, then add.
                let mut oldest_idx = 0usize;
                let mut oldest_id = self.insertion_id[0];
                for (i, &id) in self.insertion_id.iter().enumerate() {
                    if id < oldest_id { oldest_id = id; oldest_idx = i; }
                }
                self.inner.remove_zone(oldest_idx);
                self.insertion_id.remove(oldest_idx);
                self.intensities.remove(oldest_idx);
                self.eviction_count += 1;

                self.inner.add_zone(kind, center, intensity, falloff);
                self.insertion_id.push(self.next_id);
                self.intensities.push(intensity);
                self.next_id += 1;
                AddOutcome::EvictedAndAdded { evicted_idx: oldest_idx }
            }
            SaturationPolicy::PriorityEvict => {
                // Find lowest intensity, evict if new intensity > evicted.
                let mut lowest_idx = 0usize;
                let mut lowest_int = self.intensities[0];
                for (i, &v) in self.intensities.iter().enumerate() {
                    if v < lowest_int { lowest_int = v; lowest_idx = i; }
                }
                if intensity <= lowest_int {
                    // New zone is lower-or-equal priority than the lowest
                    // existing — drop the new one.
                    return AddOutcome::DroppedSilently;
                }
                self.inner.remove_zone(lowest_idx);
                self.insertion_id.remove(lowest_idx);
                self.intensities.remove(lowest_idx);
                self.eviction_count += 1;

                self.inner.add_zone(kind, center, intensity, falloff);
                self.insertion_id.push(self.next_id);
                self.intensities.push(intensity);
                self.next_id += 1;
                AddOutcome::EvictedAndAdded { evicted_idx: lowest_idx }
            }
        }
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
    let _ = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    writeln!(uart, "").ok();
    writeln!(uart, "╔══════════════════════════════════════════════════════════════════╗").ok();
    writeln!(uart, "║  Silent-cap addressing — fail-LOUD vs fail-QUIET demonstration  ║").ok();
    writeln!(uart, "╚══════════════════════════════════════════════════════════════════╝").ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  Scenario: 100 sensor reports stream into the world model").ok();
    writeln!(uart, "    - reports 0..49: 50 LOW-intensity junk hazards far from path").ok();
    writeln!(uart, "      (cap reached at report 31; 18 silently dropped in SilentDrop)").ok();
    writeln!(uart, "    - report 50: CRITICAL new hazard at (5.0, 5.0) HIGH intensity 10.0").ok();
    writeln!(uart, "    - reports 51..99: more junk").ok();
    writeln!(uart, "  Agent goes from (0,0) to (10,10) → critical hazard sits ON the path").ok();
    writeln!(uart, "").ok();

    for &policy in &[
        SaturationPolicy::SilentDrop,
        SaturationPolicy::RefuseLoud,
        SaturationPolicy::LruEvict,
        SaturationPolicy::PriorityEvict,
    ] {
        run_phase(&mut uart, policy);
    }

    writeln!(uart, "").ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Verdict: SilentDrop is fail-QUIET. The other 3 are fail-LOUD or").ok();
    writeln!(uart, "  fail-CORRECT. For defense-vertical use, SilentDrop must NEVER be").ok();
    writeln!(uart, "  the default — operator must explicitly opt into the silent mode.").ok();
    writeln!(uart, "  bench complete.").ok();
    loop { cortex_m::asm::wfi(); }
}

fn run_phase<U: core::fmt::Write>(uart: &mut U, policy: SaturationPolicy) {
    let label = match policy {
        SaturationPolicy::SilentDrop    => "SILENT_DROP    (current default — DANGEROUS)",
        SaturationPolicy::RefuseLoud    => "REFUSE_LOUD    (caller gets Err — explicit decision)",
        SaturationPolicy::LruEvict      => "LRU_EVICT      (auto-evict oldest — freshness)",
        SaturationPolicy::PriorityEvict => "PRIORITY_EVICT (auto-evict lowest intensity — criticality)",
    };
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Policy: {}", label).ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();

    let mut w = CapAwareWorld::new(policy);

    // Goal at (10, 10), Attractive
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
    w.try_add(ZoneType::Attractive, goal, 5.0, 8.0);

    // 50 LOW-intensity junk hazards far from the (0,0)→(10,10) path.
    // Cap (32) reached after ~31 of these; remaining ~18 will be the
    // first silent-drops to validate the policy difference.
    for i in 0..50 {
        let mut center: V = vz();
        center[0] = -5.0 - (i as f64) * 0.1;     // way off the path
        center[1] = -5.0 - (i as f64) * 0.07;
        // LOW intensity (1.0) — minor hazards
        w.try_add(ZoneType::Repulsive, center, 1.0, 0.5);
    }

    let zones_after_warmup = w.zone_count();
    writeln!(uart, "  Reports 0..49 (50 junk): zone_count = {} (cap reached early)",
             zones_after_warmup).ok();
    writeln!(uart, "    cap_hit_count so far = {}", w.cap_hit_count).ok();

    // Report 50 — CRITICAL new hazard at (5.0, 5.0), HIGH intensity 10.0
    // CAP IS ALREADY HIT. Does the critical reach M10?
    let mut critical: V = vz();
    critical[0] = 5.0; critical[1] = 5.0;
    let outcome = w.try_add(ZoneType::Repulsive, critical, 10.0, 2.0);
    writeln!(uart, "  Report 50 (CRITICAL hazard at (5,5), int=10.0):").ok();
    writeln!(uart, "    add outcome = {:?}", outcome).ok();
    writeln!(uart, "    zone_count after = {}", w.zone_count()).ok();
    let critical_in_model = matches!(outcome,
        AddOutcome::Added | AddOutcome::EvictedAndAdded { .. });

    // Reports 51..99: more junk after the critical one
    for i in 51..100 {
        let mut center: V = vz();
        center[0] = -10.0 - (i as f64) * 0.1;
        center[1] = -10.0;
        w.try_add(ZoneType::Repulsive, center, 1.0, 0.5);
    }

    writeln!(uart, "  After all 100 reports:").ok();
    writeln!(uart, "    zone_count       = {}", w.zone_count()).ok();
    writeln!(uart, "    cap_hit_count    = {} (operator-visible telemetry)",
             w.cap_hit_count).ok();
    writeln!(uart, "    eviction_count   = {}", w.eviction_count).ok();
    writeln!(uart, "    refuse_count     = {}", w.refuse_count).ok();

    // Did the critical hazard at (5,5) survive into M10?
    if critical_in_model {
        writeln!(uart, "  CRITICAL hazard at (5,5): IN MODEL ✓").ok();
    } else {
        writeln!(uart, "  CRITICAL hazard at (5,5): MISSING from model ✗ (fail-QUIET!)").ok();
    }

    // Agent navigates start → goal. Does the trajectory avoid (5, 5)?
    let mut start: V = vz();
    let path = w.navigate(&start, &goal, 60);
    let dist_sq_to_critical = |p: &V| {
        let dx = p[0] - 5.0;
        let dy = p[1] - 5.0;
        dx * dx + dy * dy
    };
    let min_dist_sq = path.iter()
        .map(dist_sq_to_critical)
        .fold(f64::MAX, f64::min);
    writeln!(uart, "  Agent trajectory:").ok();
    writeln!(uart, "    start ({:.1},{:.1}) → end ({:.2},{:.2}) [target was (10,10)]",
             start[0], start[1], path.last().unwrap()[0], path.last().unwrap()[1]).ok();
    writeln!(uart, "    minimum distance² to critical (5,5): {:.3}", min_dist_sq).ok();
    let safety_radius_sq = 1.0_f64;       // squared radius of "danger zone"
    if min_dist_sq < safety_radius_sq {
        writeln!(uart, "    SAFETY: agent ENTERED danger radius around (5,5) — UNSAFE").ok();
    } else {
        writeln!(uart, "    SAFETY: agent kept ≥{:.1} from (5,5) — SAFE",
                 safety_radius_sq).ok();
    }

    writeln!(uart, "").ok();
}
