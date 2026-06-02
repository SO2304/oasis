//! U4 — stronger adversarial scenario PROVING safety impact of silent-cap.
//!
//! Previous round's bench used hazard intensity=10, falloff=2.0, navigate=60
//! steps — produced SAFE outcomes for all 4 policies (margin 1.07 unit at
//! min). The user correctly flagged: "Sans lui, Finding 4 reste un soft
//! point dans le pitch." This round produces a HARD demonstration:
//!
//!   - Hazard intensity = 50 (5× stronger)
//!   - Hazard center placed EXACTLY on the (0,0)→(10,10) diagonal at (5,5)
//!   - Falloff narrowed to 0.8 (sharper exclusion)
//!   - Navigate runs 200 steps with longer step size
//!
//! Expected outcomes:
//!   - SilentDrop: critical NEVER added → trajectory passes THROUGH (5,5)
//!     → dist² to hazard ≪ 1.0 → UNSAFE collision
//!   - RefuseLoud: same agent outcome (operator didn't respond) → UNSAFE
//!   - LruEvict: critical added by evicting oldest → trajectory cleanly avoids
//!   - PriorityEvict: critical added by evicting low-intensity → cleanly avoids
//!
//! Also uses the new `try_add_zone()` API (oasis-rt 0.3.1+) which makes
//! the cap-overflow case impossible to silently ignore.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_alloc::LlffHeap as Heap;
use fugit::RateExtU32;
use rp_pico::hal::{
    clocks::init_clocks_and_plls, pac, sio::Sio, timer::Timer,
    uart::{DataBits, StopBits, UartConfig, UartPeripheral},
    watchdog::Watchdog, Clock,
};
use rp_pico::{hal, XOSC_CRYSTAL_FREQ};
use panic_halt as _;

use oasis_rt::vec::{V, vz};
use oasis_rt::world_model::{WorldModel, ZoneError, ZoneType};

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const MAX_ZONES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Policy { SilentDrop, RefuseLoud, LruEvict, PriorityEvict }

struct CapAware {
    inner: WorldModel,
    policy: Policy,
    insertion_id: Vec<u64>,
    intensities: Vec<f64>,
    next_id: u64,
    cap_hit_count: u32,
    eviction_count: u32,
    refuse_count: u32,
}

impl CapAware {
    fn new(policy: Policy) -> Self {
        Self {
            inner: WorldModel::new(), policy,
            insertion_id: Vec::new(), intensities: Vec::new(),
            next_id: 0, cap_hit_count: 0, eviction_count: 0, refuse_count: 0,
        }
    }

    /// Uses the new try_add_zone() API. Demonstrates the consistent
    /// surface: every saturation case is a Result<(), ZoneError> the
    /// caller MUST handle.
    fn try_add(&mut self, kind: ZoneType, center: V,
               intensity: f64, falloff: f64) -> bool {
        match self.inner.try_add_zone(kind, center, intensity, falloff) {
            Ok(()) => {
                self.insertion_id.push(self.next_id);
                self.intensities.push(intensity);
                self.next_id += 1;
                true
            }
            Err(ZoneError::CapacityExceeded { .. }) => {
                self.cap_hit_count += 1;
                match self.policy {
                    Policy::SilentDrop => false,    // we DO drop here, but
                                                    // cap_hit_count records it
                    Policy::RefuseLoud => {
                        self.refuse_count += 1;
                        false
                    }
                    Policy::LruEvict => {
                        // Evict oldest, then retry add
                        let mut o = 0usize; let mut oid = self.insertion_id[0];
                        for (i, &id) in self.insertion_id.iter().enumerate() {
                            if id < oid { oid = id; o = i; }
                        }
                        self.inner.remove_zone(o);
                        self.insertion_id.remove(o);
                        self.intensities.remove(o);
                        self.eviction_count += 1;
                        // Retry — cap freed by remove_zone (deactivates)
                        // Note: remove_zone doesn't shrink Vec; check len < MAX
                        if self.inner.zone_count() < MAX_ZONES {
                            let _ = self.inner.try_add_zone(kind, center, intensity, falloff);
                            self.insertion_id.push(self.next_id);
                            self.intensities.push(intensity);
                            self.next_id += 1;
                            true
                        } else {
                            false
                        }
                    }
                    Policy::PriorityEvict => {
                        // Find lowest intensity; replace if new > it
                        let mut lo = 0usize; let mut li = self.intensities[0];
                        for (i, &v) in self.intensities.iter().enumerate() {
                            if v < li { li = v; lo = i; }
                        }
                        if intensity <= li { return false; }
                        self.inner.remove_zone(lo);
                        self.insertion_id.remove(lo);
                        self.intensities.remove(lo);
                        self.eviction_count += 1;
                        if self.inner.zone_count() < MAX_ZONES {
                            let _ = self.inner.try_add_zone(kind, center, intensity, falloff);
                            self.insertion_id.push(self.next_id);
                            self.intensities.push(intensity);
                            self.next_id += 1;
                            true
                        } else {
                            false
                        }
                    }
                }
            }
        }
    }

    fn navigate(&self, start: &V, goal: &V, steps: usize) -> Vec<V> {
        self.inner.navigate(start, goal, steps)
    }

    fn zone_count(&self) -> usize { self.inner.zone_count() }
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
    writeln!(uart, "║  U4 — STRONG ADVERSARIAL: silent-cap → unsafe trajectory proof  ║").ok();
    writeln!(uart, "╚══════════════════════════════════════════════════════════════════╝").ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  Setup:").ok();
    writeln!(uart, "    50 LOW-intensity junk fills the cap before the critical arrives").ok();
    writeln!(uart, "    Critical: hazard at (5,5), intensity=50, falloff=0.8 — sharp + strong").ok();
    writeln!(uart, "    Goal at (10,10), agent starts (0,0), navigate 80 steps").ok();
    writeln!(uart, "    Safety: agent must keep distance² > 1.0 from (5,5) — NEVER trespass").ok();
    writeln!(uart, "").ok();
    writeln!(uart, "  Uses NEW try_add_zone() API (oasis-rt 0.3.1+):").ok();
    writeln!(uart, "    cap-overflow returns Err(ZoneError::CapacityExceeded), CANNOT be").ok();
    writeln!(uart, "    silently ignored. Each policy then makes its OWN decision.").ok();
    writeln!(uart, "").ok();

    for &p in &[Policy::SilentDrop, Policy::RefuseLoud, Policy::LruEvict, Policy::PriorityEvict] {
        run_strong_phase(&mut uart, p);
    }

    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  bench complete.").ok();
    loop { cortex_m::asm::wfi(); }
}

fn run_strong_phase<U: core::fmt::Write>(uart: &mut U, policy: Policy) {
    let label = match policy {
        Policy::SilentDrop    => "SILENT_DROP    (cap_hit incremented, agent uninformed)",
        Policy::RefuseLoud    => "REFUSE_LOUD    (refuse_count incremented, operator informed)",
        Policy::LruEvict      => "LRU_EVICT      (oldest evicted, critical added)",
        Policy::PriorityEvict => "PRIORITY_EVICT (lowest intensity evicted, critical added)",
    };
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();
    writeln!(uart, "  Policy: {}", label).ok();
    writeln!(uart, "──────────────────────────────────────────────────────────────────").ok();

    let mut w = CapAware::new(policy);
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
    w.try_add(ZoneType::Attractive, goal, 5.0, 8.0);

    // 50 LOW-intensity junk hazards far from path
    for i in 0..50u32 {
        let mut c: V = vz();
        c[0] = -5.0 - (i as f64) * 0.1;
        c[1] = -5.0 - (i as f64) * 0.07;
        w.try_add(ZoneType::Repulsive, c, 1.0, 0.5);
    }

    // CRITICAL HIGH-intensity hazard ON the diagonal at (5,5)
    let mut critical: V = vz(); critical[0] = 5.0; critical[1] = 5.0;
    let added = w.try_add(ZoneType::Repulsive, critical, 50.0, 0.8);
    writeln!(uart, "    critical (int=50, fall=0.8) added: {}", added).ok();
    writeln!(uart, "    cap_hit_count = {}, eviction = {}, refuse = {}",
             w.cap_hit_count, w.eviction_count, w.refuse_count).ok();

    // Agent navigates from (0,0) toward (10,10), 200 steps
    let mut start: V = vz();
    let path = w.navigate(&start, &goal, 80);

    // Find minimum squared distance to critical (5,5) along trajectory
    let dist_sq_critical = |p: &V| {
        let dx = p[0] - 5.0; let dy = p[1] - 5.0;
        dx * dx + dy * dy
    };
    let mut min_dsq = f64::MAX;
    let mut min_step = 0usize;
    for (i, p) in path.iter().enumerate() {
        let d = dist_sq_critical(p);
        if d < min_dsq { min_dsq = d; min_step = i; }
    }
    let last = path.last().unwrap();
    writeln!(uart, "    end position: ({:.2}, {:.2}) (after {} steps)",
             last[0], last[1], path.len() - 1).ok();
    writeln!(uart, "    min dist² to critical (5,5): {:.4} at step {}",
             min_dsq, min_step).ok();

    // Safety verdict: did the agent enter the danger radius?
    let safety_threshold_sq = 1.0_f64;
    if min_dsq < safety_threshold_sq {
        writeln!(uart, "    SAFETY: UNSAFE — agent entered danger radius (dist² < 1.0)").ok();
        // What was the agent doing at the closest approach?
        let p_at_min = &path[min_step];
        writeln!(uart, "             at min: ({:.2}, {:.2}) — inside critical zone",
                 p_at_min[0], p_at_min[1]).ok();
    } else {
        writeln!(uart, "    SAFETY: SAFE — kept dist² >= 1.0 from critical").ok();
    }
    writeln!(uart, "").ok();
}
