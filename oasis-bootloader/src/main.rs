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

const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let flash = WatchdogFlash::<FLASH_SIZE>::start(p.FLASH, p.WATCHDOG, Duration::from_secs(8));
    let flash = Mutex::new(RefCell::new(flash));

    let config = BootLoaderConfig::from_linkerfile_blocking(&flash, &flash, &flash);
    let active_offset = config.active.offset();
    let bl: BootLoader = BootLoader::prepare(config);

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
