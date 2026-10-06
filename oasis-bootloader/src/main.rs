//! OASIS A/B bootloader for the RP2040 test nodes (Phase 1.3).
//!
//! The embassy-boot-rp example bootloader, with OASIS's partition layout
//! (`memory.x`). All decisions are embassy-boot's: on `Swap` it copies DFU into
//! ACTIVE with a progress index (resumable after a power cut); if the application did
//! not call `mark_booted()` before the next reset, it reverts. A watchdog (8 s) is
//! started here and keeps running into the application, so a new image that hangs
//! is reset and reverted. No signature is checked here: see
//! docs/specs/FIRMWARE_UPDATE_SPEC.md §1 and §7.

#![no_std]
#![no_main]

use core::cell::RefCell;

use cortex_m_rt::{entry, exception};
use embassy_boot_rp::*;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::Duration;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};

const FLASH_SIZE: usize = 2 * 1024 * 1024;

/// Boot breadcrumbs (diagnostics, Phase 1.3 bring-up): one byte appended at the first
/// erased (0xFF) position of this 4 KiB sector at each stage. Flash survives every
/// reset and power cycle; the application's `@X` command dumps it. Bootloader codes:
/// 0xB1 initialised, 0xB2..0xB5 prepared (state Boot/Swap/Revert/DfuDetach), 0xB9 jumping.
const CRUMB_OFFSET: u32 = 0x1F_0000;

/// A value as two breadcrumbs `0xE0|high nibble`, `0xE0|low nibble` (never 0xFF,
/// which would read as erased).
fn crumb_hex<F: NorFlash + ReadNorFlash>(flash: &mut F, v: u8) {
    crumb(flash, 0xE0 | (v >> 4));
    crumb(flash, 0xE0 | (v & 0x0F));
}

/// Boot guard, owned by the bootloader: watchdog SCRATCH2 counts consecutive boots
/// that did not reach the application's main loop (the application clears it there).
/// SCRATCH2/3 persist across watchdog and soft resets (the bootrom's USB boot uses
/// SCRATCH0/1 and 4..7). After `MAX_FAILED_BOOTS` the board enters BOOTSEL instead
/// of jumping, so a broken application can always be reflashed over USB.
const GUARD_MAGIC: u32 = 0x0A5E_0000;
const MAX_FAILED_BOOTS: u32 = 3;
// RP2040 WATCHDOG (base 0x40058000): SCRATCH2 at +0x14, SCRATCH3 at +0x18 (rp-pac offsets).
const WD_SCRATCH2: *mut u32 = 0x4005_8014 as *mut u32;
const WD_SCRATCH3: *mut u32 = 0x4005_8018 as *mut u32;
const WD_SCRATCH1: *mut u32 = 0x4005_8010 as *mut u32;

fn crumb<F: NorFlash + ReadNorFlash>(flash: &mut F, b: u8) {
    let mut buf = [0u8; 64];
    let mut off = 0u32;
    while off < 4096 {
        if flash.read(CRUMB_OFFSET + off, &mut buf).is_err() {
            return;
        }
        if let Some(i) = buf.iter().position(|&x| x == 0xFF) {
            let _ = flash.write(CRUMB_OFFSET + off + i as u32, &[b]);
            return;
        }
        off += 64;
    }
}

#[entry]
fn main() -> ! {
    // Leave the clocks close to their power-on state: ring oscillator only, XOSC and
    // both PLLs untouched, so the application (rp2040-hal) initialises its clocks as
    // after a cold boot. The ROSC is set to the one configuration embassy-rp documents
    // as nominal (6.5 MHz: Medium range, divider 16). ClockConfig::rosc() asks for an
    // unmeasured 140 MHz (High range): flash is read at half the system clock, and an
    // uncalibrated fast ROSC can corrupt XIP fetches. Bring-up history:
    // evidence/silicon/2026-10-06/fwupdate/.
    let mut clocks = embassy_rp::clocks::ClockConfig::rosc();
    clocks.rosc = Some(embassy_rp::clocks::RoscConfig {
        hz: 6_500_000,
        range: embassy_rp::clocks::RoscRange::Medium,
        drive_strength: [0; 8],
        div: 16,
    });
    let p = embassy_rp::init(embassy_rp::config::Config::new(clocks));

    let mut flash = WatchdogFlash::<FLASH_SIZE>::start(p.FLASH, p.WATCHDOG, Duration::from_secs(8));
    // What the previous boot left: application stage (SCRATCH3) and guard count.
    let s2 = unsafe { WD_SCRATCH2.read_volatile() };
    let s3 = unsafe { WD_SCRATCH3.read_volatile() };
    let s1 = unsafe { WD_SCRATCH1.read_volatile() };
    crumb(&mut flash, 0xC1);
    for b in s3.to_le_bytes() {
        crumb_hex(&mut flash, b);
    }
    crumb_hex(&mut flash, s2 as u8);
    crumb(&mut flash, 0xC2);
    for b in s1.to_le_bytes() {
        crumb_hex(&mut flash, b);
    }
    let failed = if s2 & 0xFFFF_0000 == GUARD_MAGIC {
        s2 & 0xFFFF
    } else {
        0
    };
    if failed >= MAX_FAILED_BOOTS {
        unsafe { WD_SCRATCH2.write_volatile(0) };
        crumb(&mut flash, 0xBF);
        embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    }
    unsafe { WD_SCRATCH2.write_volatile(GUARD_MAGIC | (failed + 1)) };
    crumb(&mut flash, 0xB1);
    let flash = Mutex::new(RefCell::new(flash));

    let config = BootLoaderConfig::from_linkerfile_blocking(&flash, &flash, &flash);
    let active_offset = config.active.offset();
    let bl: BootLoader = BootLoader::prepare(config);
    let code = match bl.state {
        State::Boot => 0xB2,
        State::Swap => 0xB3,
        State::Revert => 0xB4,
        State::DfuDetach => 0xB5,
    };
    // NVIC state at hand-over (embassy_rp::init enables IO_IRQ_BANK0, IO_IRQ_QSPI and
    // DMA_IRQ_0): enabled (ISER) and pending (ISPR) masks go to the breadcrumbs.
    const NVIC_ISER: *mut u32 = 0xE000_E100 as *mut u32;
    const NVIC_ICER: *mut u32 = 0xE000_E180 as *mut u32;
    const NVIC_ISPR: *mut u32 = 0xE000_E200 as *mut u32;
    const NVIC_ICPR: *mut u32 = 0xE000_E280 as *mut u32;
    const SYST_CSR: *mut u32 = 0xE000_E010 as *mut u32;
    let (iser, ispr) = unsafe { (NVIC_ISER.read_volatile(), NVIC_ISPR.read_volatile()) };
    flash.lock(|f| {
        let mut f = f.borrow_mut();
        crumb(&mut *f, code);
        crumb(&mut *f, 0xD1);
        for b in iser.to_le_bytes().iter().chain(ispr.to_le_bytes().iter()) {
            crumb_hex(&mut *f, *b);
        }
        // What the jump will use: the application's vector table (initial MSP, reset
        // vector) and its first code word, as read through XIP right now.
        crumb(&mut *f, 0xD2);
        for addr in [0x1000_8000u32, 0x1000_8004, 0x1000_8100] {
            let w = unsafe { (addr as *const u32).read_volatile() };
            for b in w.to_le_bytes() {
                crumb_hex(&mut *f, b);
            }
        }
        crumb(&mut *f, 0xB9);
    });
    // Hand the application a clean interrupt state, as after a reset: every NVIC line
    // disabled and not pending, SysTick off. The application (rp2040-hal) installs
    // none of embassy's handlers, so a pending line would land in its default handler.
    cortex_m::interrupt::disable();
    unsafe {
        NVIC_ICER.write_volatile(0xFFFF_FFFF);
        NVIC_ICPR.write_volatile(0xFFFF_FFFF);
        SYST_CSR.write_volatile(0);
        cortex_m::interrupt::enable();
    }

    unsafe { bl.load(embassy_rp::flash::FLASH_BASE as u32 + active_offset) }
}

#[no_mangle]
#[cfg_attr(target_os = "none", link_section = ".HardFault.user")]
unsafe extern "C" fn HardFault() {
    cortex_m::peripheral::SCB::sys_reset();
}

#[exception]
unsafe fn DefaultHandler(_: i16) -> ! {
    cortex_m::peripheral::SCB::sys_reset();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // A reset retries the boot; the swap/revert state survives in flash.
    cortex_m::peripheral::SCB::sys_reset();
}
