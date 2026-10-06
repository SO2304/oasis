//! OASIS wired-UART mesh node (RP2040) — A→B→C relay over a real wire.
//!
//! Each node runs a v0A (Ed25519 per-node signed) `MeshRouter`, uses **UART0
//! full-duplex** (TX=GP0 pin1, RX=GP1 pin2), verifies the signature against a
//! 3-node pubkey registry, logs over USB-CDC, and relays forwarded envelopes.
//! This is a **wired UART link**, NOT LoRa — no radio is involved.
//!
//! (UART1/GP4 was tried first but failed internal loopback on this silicon;
//! UART0 passes internal loopback, so the link uses UART0 only.)
//!
//! Wiring chain: A.GP0→B.GP1, B.GP0→C.GP1, common GND.
//! USB-CDC control: `o`=originate, `s`=RX status, `l`=UART0 internal self-test,
//! `b`=reboot to BOOTSEL. `K`=strict v0B (refuse v8/v9/v0A), `k`=legacy allowed.
//! Phase 1.1 (hybrid authority, `pq.rs`), line commands: `@Q<hex>` append to the
//! staged `OAU1` message, `@C` clear it, `@F` fragment it (`OFR1`) and originate
//! each fragment in its own v0B envelope, `@T<idx>` same with fragment `idx`
//! altered after fragmentation, `@I` policy/reassembly status.
//! Phase 1.2 (`enroll.rs`): the identity is generated on the board at first boot (no
//! compiled seeds any more) and the v0B registry holds only enrolled nodes. `@E<challenge
//! hex><nonce hex>` identity + proof of possession (first call before enrollment mixes
//! the nonce into the key once and reboots), `@N` entropy statistics, `@L` run the staged
//! message through the authority gate locally (then store-and-forward), `@W` who-am-I.
//! Phase 1.3 (feature `bootloaded`, `update.rs`): `@U<offset 8 hex><data hex>` write
//! the new image into DFU, `@M` check the staged manifest (`@Q`) against it and, if
//! accepted, mark the update and reset (the bootloader swaps), `@V` version/floor.

#![no_std]
#![no_main]

extern crate alloc;

use core::fmt::Write as _;
use rp2040_hal as hal;

use embedded_alloc::Heap;
use hal::clocks::Clock;
use hal::fugit::RateExtU32;
use hal::gpio::FunctionUart;
use hal::pac;
use hal::uart::{DataBits, StopBits, UartConfig, UartPeripheral};
use usb_device::class_prelude::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDevice};
use usb_device::prelude::{UsbDeviceBuilder, UsbVidPid};
use usbd_serial::SerialPort;

use oasis_rt::mesh::{mesh_v0b_verify, mesh_v10_verify, MeshDecision, MeshEdPub, MeshEdSeed, MeshPubRegistry, MeshRouter};
use oasis_rt::spore_crypto::CounterTracker;
use oasis_rt::actuation::parse_oac1;
use oasis_rt::mesh::inner_slice;

#[path = "../ef.rs"]
mod ef;
#[path = "../pq.rs"]
mod pq;
#[path = "../enroll.rs"]
mod enroll;
#[cfg(feature = "bootloaded")]
#[path = "../update.rs"]
mod update;
use oasis_rt::fragment::{fragment, FragOutcome};
use oasis_rt::tx_lease::{DualSlotStore, SlotIo, TxLease, TX_LEASE_BLOCK, TX_LEASE_RECORD_LEN};

/// Upper bound on counters a single USB command buffer can consume (the
/// commands sum to <= 900: X=600, N=200, G=50, Y=12, `@F`/`@T` <= 32 fragments,
/// others 1 each). One
/// durable reservation covering this is made BEFORE any command in the buffer
/// originates, so no counter is ever used above the persisted ceiling.
const CMD_MAX_ORIGINATIONS: u64 = 1024;

/// Sender counter lease on flash: two alternating 4 KiB sectors (below the
/// receiver-window sector at 0x1FF000). A torn write damages only one slot.
const LEASE_SECTORS: [u32; 2] = [0x1F_D000, 0x1F_E000];
struct FlashSlots;
impl SlotIo for FlashSlots {
    fn read(&self, slot: usize) -> [u8; TX_LEASE_RECORD_LEN] {
        let p = (0x1000_0000usize + LEASE_SECTORS[slot] as usize) as *const u8;
        let mut r = [0u8; TX_LEASE_RECORD_LEN];
        for (i, b) in r.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile(p.add(i)) };
        }
        r
    }
    fn write(&mut self, slot: usize, rec: &[u8; TX_LEASE_RECORD_LEN]) -> bool {
        let mut buf = [0xFFu8; 256]; // flash program granularity
        buf[..TX_LEASE_RECORD_LEN].copy_from_slice(rec);
        cortex_m::interrupt::free(|_| unsafe {
            rp2040_flash::flash::flash_range_erase(LEASE_SECTORS[slot], 4096, true);
            rp2040_flash::flash::flash_range_program(LEASE_SECTORS[slot], &buf, true);
        });
        true // durability is confirmed by DualSlotStore's read-back
    }
}

/// Test-harness only: erase both lease sectors (factory reset).
fn wipe_lease_sectors() {
    cortex_m::interrupt::free(|_| unsafe {
        for s in LEASE_SECTORS {
            rp2040_flash::flash::flash_range_erase(s, 4096, true);
        }
    });
}

#[global_allocator]
static HEAP: Heap = Heap::empty();

// Standalone build: a crash reboots into BOOTSEL (visible, reflashable). Under the
// A/B bootloader (Phase 1.3) it resets instead, so that an image which has not
// confirmed itself is reverted by the bootloader rather than parked in BOOTSEL.
#[panic_handler]
fn on_panic(_: &core::panic::PanicInfo) -> ! {
    #[cfg(not(feature = "bootloaded"))]
    hal::rom_data::reset_to_usb_boot(0, 0);
    cortex_m::peripheral::SCB::sys_reset();
}
#[cortex_m_rt::exception]
unsafe fn HardFault(_ef: &cortex_m_rt::ExceptionFrame) -> ! {
    #[cfg(not(feature = "bootloaded"))]
    hal::rom_data::reset_to_usb_boot(0, 0);
    cortex_m::peripheral::SCB::sys_reset();
}

// boot2 belongs to the bootloader when there is one.
#[cfg(not(feature = "bootloaded"))]
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XTAL_HZ: u32 = 12_000_000;
const BOARD_ID: &str = env!("OASIS_BOARD_ID");
const GIT_HASH: &str = env!("OASIS_GIT_HASH");
// v0B mesh network id (domain separation); all 3 boards share one network.
const NETWORK_ID: [u8; 8] = *b"OASISnet";
type Usb = hal::usb::UsbBus;

/// Microsecond timestamp from the RP2040 1 MHz TIMER (32-bit, wraps ~71 min).
#[inline]
fn now_us() -> u32 {
    unsafe { (*pac::TIMER::ptr()).timerawl().read().bits() }
}
/// Median of 5 samples (sorts in place).
fn median5(s: &mut [u32; 5]) -> u32 {
    s.sort_unstable();
    s[2]
}

/// Flash persistence of the v0B counter window (T8 reboot-replay). Uses the
/// last 4 KiB sector of a 2 MiB flash and rp2040-flash's RAM-safe bootrom
/// helpers. Survives power loss; restored on boot so a replayed counter is
/// rejected after a reboot.
mod persist {
    use alloc::vec::Vec;
    const SECTOR: u32 = 0x1F_F000; // offset of the last 4K sector (2 MiB flash)
    const XIP_BASE: usize = 0x1000_0000;
    const MAGIC: [u8; 4] = *b"V0BP";

    /// Erase + program the sector with [magic(4)][len(4 LE)][tracker bytes].
    pub fn save(bytes: &[u8]) -> bool {
        if bytes.len() > 4096 - 8 {
            return false;
        }
        let mut buf = [0xFFu8; 4096];
        buf[0..4].copy_from_slice(&MAGIC);
        buf[4..8].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        buf[8..8 + bytes.len()].copy_from_slice(bytes);
        cortex_m::interrupt::free(|_| unsafe {
            rp2040_flash::flash::flash_range_erase(SECTOR, 4096, true);
            rp2040_flash::flash::flash_range_program(SECTOR, &buf, true);
        });
        true
    }

    /// Test-harness only: erase the receiver-window sector.
    pub fn wipe() {
        cortex_m::interrupt::free(|_| unsafe {
            rp2040_flash::flash::flash_range_erase(SECTOR, 4096, true);
        });
    }

    /// Read back the persisted tracker bytes (XIP-mapped), or None if absent.
    pub fn load() -> Option<Vec<u8>> {
        let p = (XIP_BASE + SECTOR as usize) as *const u8;
        let hdr = unsafe { core::slice::from_raw_parts(p, 8) };
        if hdr[0..4] != MAGIC {
            return None;
        }
        let len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
        if len == 0 || len > 4096 - 8 {
            return None;
        }
        let data = unsafe { core::slice::from_raw_parts(p.add(8), len) };
        Some(data.to_vec())
    }
}

// UART0 pins: GP0 = TX, GP1 = RX.
type UartPins0 = (
    hal::gpio::Pin<hal::gpio::bank0::Gpio0, FunctionUart, hal::gpio::PullDown>,
    hal::gpio::Pin<hal::gpio::bank0::Gpio1, FunctionUart, hal::gpio::PullDown>,
);
type Uart0 = UartPeripheral<hal::uart::Enabled, pac::UART0, UartPins0>;

/// Lowercase hex of a byte slice, for logs.
struct Hx<'a>(&'a [u8]);
impl core::fmt::Display for Hx<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

/// No usable identity (entropy health tests or flash write failed): the node stays
/// mute, keeps USB alive, answers every input with the reason; `b` BOOTSEL, `!` wipe.
fn identity_failed(io: &mut Io, reason: &str) -> ! {
    loop {
        io.poll();
        let mut rx = [0u8; 16];
        if let Ok(n) = io.serial.read(&mut rx) {
            if rx[..n].contains(&b'b') {
                hal::rom_data::reset_to_usb_boot(0, 0);
            }
            if rx[..n].contains(&b'!') {
                enroll::wipe_sectors();
                cortex_m::peripheral::SCB::sys_reset();
            }
            if n > 0 {
                io.log("IDENTITY_FAIL", format_args!("{}", reason));
            }
        }
    }
}

#[hal::entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 96 * 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }

    // Phase 1.3: boot guard + init breadcrumbs (update.rs), before anything that could hang.
    #[cfg(feature = "bootloaded")]
    let (guard_failed, guard_prev_stage) = update::boot_guard_enter();
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        XTAL_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    // UART0 full-duplex: TX = GP0 (pin 1), RX = GP1 (pin 2).
    let mut uart0: Uart0 = UartPeripheral::new(
        pac.UART0,
        (
            pins.gpio0.into_function::<FunctionUart>(),
            pins.gpio1.into_function::<FunctionUart>(),
        ),
        &mut pac.RESETS,
    )
    .enable(
        UartConfig::new(115_200.Hz(), DataBits::Eight, None, StopBits::One),
        clocks.peripheral_clock.freq(),
    )
    .unwrap();

    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));
    let serial = SerialPort::new(&usb_bus);
    let usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::default()
            .manufacturer("OASIS")
            .product("oasis-uart-mesh")
            .serial_number(BOARD_ID)])
        .unwrap()
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();

    let mut io = Io { usb_dev, serial };
    #[cfg(feature = "bootloaded")]
    update::stage(2); // clocks, UART and USB objects created
    // Phase 1.2: identity generated on this board at first boot, never output.
    enroll::rosc_enable();
    let id_boot = enroll::boot_identity();
    let (id_source, id_attempts) = match &id_boot {
        enroll::IdBoot::Loaded { .. } => ("loaded", 0),
        enroll::IdBoot::Generated { attempts, .. } => ("generated", *attempts),
        _ => ("failed", 0),
    };
    let (my_seed, id_mixed) = match id_boot {
        enroll::IdBoot::Loaded { seed, mixed } => (seed, mixed),
        enroll::IdBoot::Generated { seed, .. } => (seed, false),
        enroll::IdBoot::EntropyFail => identity_failed(&mut io, "entropy_health_tests_failed"),
        enroll::IdBoot::PersistFail => identity_failed(&mut io, "flash_write_failed"),
    };
    let my_pk = oasis_rt::identity::public_key(&my_seed);
    let my_fp = oasis_rt::identity::fingerprint(&my_pk);
    // Owner, enrolled-node registry and authority policy, all from flash.
    let mut pqs = pq::Pq::boot();
    #[cfg(feature = "bootloaded")]
    update::stage(3); // identity, owner, registry, policy loaded
    // Phase 1.3: confirm a freshly swapped image (self-test, floor, mark_booted)
    // while the bootloader's watchdog still runs, then stop the watchdog.
    #[cfg(feature = "bootloaded")]
    let mut fw_aligned = [0u8; 1];
    #[cfg(feature = "bootloaded")]
    let mut fw_up = update::updater(&mut fw_aligned);
    #[cfg(feature = "bootloaded")]
    let fw_boot = update::boot_confirm(&mut fw_up);
    #[cfg(feature = "bootloaded")]
    {
        if fw_boot == update::BootOutcome::SelfTestFailed {
            // Not confirmed: reset now, the bootloader reverts to the previous image.
            cortex_m::peripheral::SCB::sys_reset();
        }
        watchdog.disable();
        update::stage(4); // confirmed (if swapped), watchdog stopped
    }
    #[cfg(feature = "bootloaded")]
    let mut guard_cleared = false;
    // v0B router: handles v0A envelopes as a superset (origin_wrap/process
    // unchanged) AND v0B (origin_wrap_v0b/process_v0b dispatch). The registry starts
    // empty and is filled only from enrollment attestations.
    let mut router = MeshRouter::new_v0b(my_fp, NETWORK_ID, MeshEdSeed(my_seed), MeshPubRegistry::new());
    pqs.install_registry(&mut router);
    // A v0B router is strict by default (legacy envelopes dropped, no downgrade).
    // This test firmware also hosts the earlier v0A commands (o/R/F/T/X/N), so it
    // boots in legacy-allowed mode; send `K` on every node before a v0B-only run.
    router.set_allow_legacy(true);

    let mut deframer = Deframer::new();
    let mut rx_total: u32 = 0;
    let mut frame_total: u32 = 0;
    let mut crc_fails: u32 = 0;
    // T8: remember the last v0B envelope A originated, so it can be replayed
    // byte-for-byte across a reboot of the relay (same counter => must be rejected).
    let mut last_v0b = [0u8; MAX_ENV];
    let mut last_v0b_len = 0usize;
    io.log("BOOT", format_args!("uart-mesh board={} fp={} UART0 tx=GP0 rx=GP1 @115200", BOARD_ID, Hx(&my_fp)));
    io.log("IDENTITY_BOOT", format_args!("source={},attempts={},fp={}", id_source, id_attempts, Hx(&my_fp)));
    // T8: restore the persisted v0B counter window so a post-reboot replay is
    // rejected. Must happen BEFORE processing any v0B envelope.
    match persist::load() {
        Some(saved) => match router.restore_counter_tracker(&saved) {
            Ok(()) => io.log("PERSIST_LOADED", format_args!("bytes={}", saved.len())),
            Err(e) => io.log("PERSIST_BAD", format_args!("{}", e)),
        },
        None => io.log("PERSIST_NONE", format_args!("no saved v0b window")),
    }
    // Sender lease: resume AT the persisted ceiling, so every counter emitted
    // after this boot exceeds every counter emitted before it.
    let mut lstore = DualSlotStore::new(FlashSlots);
    let mut lease = TxLease::boot(&lstore, TX_LEASE_BLOCK);
    router.set_tx_counter(lease.resume_point());
    io.log("LEASE_BOOT", format_args!("resume={},ceiling={}", lease.resume_point(), lease.ceiling()));
    // boot_id for the actuation gate's time base = this boot's lease resume point.
    // Force one durable reservation NOW so the next boot resumes strictly higher,
    // even if this boot never receives a command (otherwise two boots could share
    // a boot_id and a command stamped for the previous boot would look fresh).
    let boot_id = lease.resume_point();
    if !lease.ensure(boot_id.saturating_add(1), &mut lstore) {
        io.log("LEASE_FAIL", format_args!("boot reservation failed; boot_id={} not unique", boot_id));
    }

    // Part C: actuator output on GP25 (on-board LED) + revocation/actuation state.
    let _led = pins.gpio25.into_push_pull_output();
    ef::led_set(false);
    let mut efs = ef::Ef::new(boot_id);
    match efs.restore(&mut router, &pqs.owner) {
        Ok(Some((e, by_current))) => {
            pqs.rev_signer = if by_current { pq::RevSigner::Current } else { pq::RevSigner::Previous };
            io.log(
                "REV_BOOT",
                format_args!("restored_epoch={},revoked={},signed_by_current_owner={}", e, efs.rev.revoked.len(), by_current),
            )
        }
        Ok(None) => io.log("REV_BOOT", format_args!("no_saved_list")),
        Err(r) => io.log("REV_BOOT", format_args!("persisted_list_rejected={:?}", r)),
    }
    io.log(
        "POLICY_BOOT",
        format_args!(
            "min_suite_revocation={},min_suite_policy={},legacy_orv1_allowed={}",
            pqs.policy.min_suite(oasis_rt::authority::kind::REVOCATION),
            pqs.policy.min_suite(oasis_rt::authority::kind::POLICY),
            pqs.policy.legacy_orv1_allowed()
        ),
    );
    // `@P<hex>` + newline from the PC: originate a v0B envelope with that payload.
    // Line bytes are diverted so hex digits never trigger single-byte commands.
    let mut line = [0u8; 600];
    let mut line_len = 0usize;
    let mut in_line = false;

    loop {
        io.poll();
        #[cfg(feature = "bootloaded")]
        if !guard_cleared {
            update::boot_guard_ok();
            guard_cleared = true;
        }

        let mut raw = [0u8; 16];
        if let Ok(n_raw) = io.serial.read(&mut raw) {
            // Split the input: `@...` line bytes (up to a newline) go to `line`,
            // the rest are single-byte commands.
            let mut rx = [0u8; 16];
            let mut n = 0usize;
            let mut line_done = false;
            for &byte in &raw[..n_raw] {
                if in_line {
                    if byte == b'\n' || byte == b'\r' {
                        in_line = false;
                        line_done = true;
                    } else if line_len < line.len() {
                        line[line_len] = byte;
                        line_len += 1;
                    }
                } else if byte == b'@' {
                    in_line = true;
                    line_len = 0;
                } else {
                    rx[n] = byte;
                    n += 1;
                }
            }
            // Make every counter this buffer (commands + a completed line) could
            // consume durable FIRST. On failure nothing in the buffer runs.
            let writes_before = lease.writes();
            let any_input = n > 0 || line_done;
            let lease_ok = !any_input
                || lease.ensure(router.tx_counter().saturating_add(CMD_MAX_ORIGINATIONS), &mut lstore);
            if !lease_ok {
                io.log("LEASE_FAIL", format_args!("tx={},ceiling={}", router.tx_counter(), lease.ceiling()));
            }
            let line_done = line_done && lease_ok;
            let n = if lease_ok { n } else { 0 };
            if lease.writes() != writes_before {
                io.log(
                    "LEASE_PERSIST",
                    format_args!("ceiling={},writes_since_boot={}", lease.ceiling(), lease.writes()),
                );
            }
            // `@P<hex>`: originate a v0B envelope carrying the PC-built payload
            // (operator-signed ORV1 list, or an OAC1 command). Stored for `z` replay.
            if line_done && line_len >= 1 && line[0] == b'P' {
                let mut pl = [0u8; 300];
                match ef::hex_decode(&line[1..line_len], &mut pl) {
                    Some(plen) => {
                        if let Some(env) = router.origin_wrap_v0b(&pl[..plen]) {
                            last_v0b_len = env.len().min(MAX_ENV);
                            last_v0b[..last_v0b_len].copy_from_slice(&env[..last_v0b_len]);
                            send_framed(&mut uart0, &env);
                            io.log(
                                "PAYLOAD_TX",
                                format_args!(
                                    "kind={},counter={},len={}",
                                    ef::content_kind(&pl[..plen]),
                                    router.tx_counter(),
                                    env.len()
                                ),
                            );
                        }
                    }
                    None => io.log("PAYLOAD_BAD_HEX", format_args!("chars={}", line_len - 1)),
                }
            }
            // Phase 1.1: staged authority message from the PC, fragmented on demand.
            if line_done && line_len >= 1 && line[0] == b'Q' {
                let mut chunk = [0u8; 300];
                match ef::hex_decode(&line[1..line_len], &mut chunk) {
                    Some(n) if pqs.stage.len() + n <= oasis_rt::fragment::MAX_ASSEMBLED => {
                        pqs.stage.extend_from_slice(&chunk[..n]);
                        io.log("STAGE", format_args!("len={}", pqs.stage.len()));
                    }
                    Some(_) => io.log("STAGE_FULL", format_args!("len={}", pqs.stage.len())),
                    None => io.log("STAGE_BAD_HEX", format_args!("chars={}", line_len - 1)),
                }
            }
            if line_done && line_len >= 1 && line[0] == b'C' {
                pqs.stage.clear();
                io.log("STAGE", format_args!("len=0"));
            }
            if line_done && line_len >= 1 && (line[0] == b'F' || line[0] == b'T') {
                let tamper = if line[0] == b'T' {
                    core::str::from_utf8(&line[1..line_len]).ok().and_then(|t| t.parse::<usize>().ok())
                } else {
                    None
                };
                let stage = core::mem::take(&mut pqs.stage);
                send_fragments(&mut io, &mut router, &mut uart0, &stage, tamper);
                pqs.stage = stage; // kept, so the same message can be resent
            }
            // Phase 1.2: identity + proof of possession. Before enrollment, the first call
            // mixes the tool's nonce into the key once, persists it and reboots; the
            // tool then calls again. The seed itself is never printed.
            if line_done && line_len >= 1 && line[0] == b'E' {
                let mut cn = [0u8; 64];
                match ef::hex_decode(&line[1..line_len], &mut cn) {
                    Some(64) => {
                        let mut challenge = [0u8; 32];
                        challenge.copy_from_slice(&cn[..32]);
                        let mut nonce = [0u8; 32];
                        nonce.copy_from_slice(&cn[32..]);
                        let enrolled = pqs.registry.get(&my_fp).is_some();
                        if !id_mixed && !enrolled {
                            match enroll::rekey_identity(&my_seed, &nonce) {
                                Some(s2) => {
                                    let fp2 = oasis_rt::identity::fingerprint(&oasis_rt::identity::public_key(&s2));
                                    io.log("REKEYED", format_args!("old_fp={},new_fp={},rebooting", Hx(&my_fp), Hx(&fp2)));
                                    for _ in 0..200_000 {
                                        io.poll();
                                    }
                                    cortex_m::peripheral::SCB::sys_reset();
                                }
                                None => io.log("REKEY_FAIL", format_args!("entropy_or_flash")),
                            }
                        } else {
                            io.log(
                                "IDENTITY",
                                format_args!("pk={},fp={},mixed={},enrolled={}", Hx(&my_pk), Hx(&my_fp), id_mixed, enrolled),
                            );
                            let sig = oasis_rt::identity::pop_sign(&my_seed, &NETWORK_ID, &challenge);
                            io.log("POP", format_args!("sig={}", Hx(&sig)));
                        }
                    }
                    _ => io.log("E_BAD", format_args!("need 128 hex chars (challenge + nonce)")),
                }
            }
            // Entropy statistics on 100 000 fresh raw bits + health tests on 4 096 (never the seed).
            if line_done && line_len >= 1 && line[0] == b'N' {
                let bits = enroll::sample_bits(100_000);
                let st = oasis_rt::identity::bit_stats(&bits, 100_000);
                let fresh = enroll::sample_bits(oasis_rt::identity::RAW_SAMPLES);
                let n = oasis_rt::identity::RAW_SAMPLES;
                io.log(
                    "ENTROPY",
                    format_args!(
                        "n={},ones={},longest_run={},mcv_h_milli={},rct_ok={},apt_ok={},delay_cycles={}",
                        st.n,
                        st.ones,
                        st.longest_run,
                        oasis_rt::identity::mcv_min_entropy_milli(&st),
                        oasis_rt::identity::rct_ok(&fresh, n),
                        oasis_rt::identity::apt_ok(&fresh, n),
                        enroll::ROSC_SAMPLE_DELAY_CYCLES
                    ),
                );
            }
            // Run the staged message through the SAME gate as a mesh-received one: USB is
            // not a privileged channel. Then the usual store-and-forward rule applies.
            if line_done && line_len >= 1 && line[0] == b'L' {
                let msg = pqs.stage.clone();
                let (done, g) = pqs.complete(&mut efs, &mut router, &msg);
                finish_authority(&mut io, &mut router, &mut uart0, &mut lease, &mut lstore, &pqs, &efs, &msg, done, &g);
            }
            if line_done && line_len >= 1 && line[0] == b'W' {
                io.log(
                    "WHOAMI",
                    format_args!(
                        "fp={},mixed={},owner_seq={},owner_ed={},previous_owner={},pending_offer={},peers={},epoch={},rev_signer={:?}",
                        Hx(&my_fp),
                        id_mixed,
                        pqs.owner.seq,
                        Hx(&pqs.owner.current.ed25519[..8]),
                        pqs.owner.previous.as_ref().map_or(false, |_| true),
                        pqs.pending.as_ref().map_or(0, |p| p.seq),
                        pqs.registry.entries.len(),
                        efs.rev.epoch,
                        pqs.rev_signer
                    ),
                );
                for e in &pqs.registry.entries {
                    io.log("PEER", format_args!("fp={},role={},perms={},seq={}", Hx(&e.fp), e.role, e.permissions, e.seq));
                }
            }
            // Phase 1.3: signed firmware update (see update.rs).
            #[cfg(feature = "bootloaded")]
            if line_done && line_len >= 9 && line[0] == b'U' {
                let mut off = [0u8; 4];
                let mut data = [0u8; 256];
                match (ef::hex_decode(&line[1..9], &mut off), ef::hex_decode(&line[9..line_len], &mut data)) {
                    (Some(4), Some(n)) => {
                        let o = u32::from_be_bytes(off) as usize;
                        match fw_up.write_firmware(o, &data[..n]) {
                            Ok(()) => {
                                if (o + n) % 65536 < n || n < 256 {
                                    io.log("DFU_PROGRESS", format_args!("written_to={}", o + n));
                                }
                            }
                            Err(_) => io.log("DFU_WRITE_FAIL", format_args!("offset={},len={}", o, n)),
                        }
                    }
                    _ => io.log("DFU_BAD_HEX", format_args!("chars={}", line_len)),
                }
            }
            #[cfg(feature = "bootloaded")]
            if line_done && line_len >= 1 && line[0] == b'M' {
                let msg = pqs.stage.clone();
                let keys = pqs.owner.current.keys();
                let t0 = now_us();
                match update::install(&mut fw_up, &pqs.policy, &keys, &msg) {
                    Ok(v) => {
                        io.log(
                            "FW_INSTALL",
                            format_args!(
                                "accepted,version={},running={},floor={},check_us={},resetting_for_swap",
                                v,
                                update::FW_VERSION,
                                update::floor(),
                                now_us().wrapping_sub(t0)
                            ),
                        );
                        for _ in 0..300_000 {
                            io.poll();
                        }
                        cortex_m::peripheral::SCB::sys_reset();
                    }
                    Err(r) => io.log(
                        "FW_INSTALL",
                        format_args!(
                            "refused={:?},running={},floor={},check_us={}",
                            r,
                            update::FW_VERSION,
                            update::floor(),
                            now_us().wrapping_sub(t0)
                        ),
                    ),
                }
            }
            #[cfg(feature = "bootloaded")]
            if line_done && line_len >= 1 && line[0] == b'V' {
                io.log(
                    "FW_STATUS",
                    format_args!(
                        "version={},floor={},boot={:?},bootloader_state={},fp={},guard_failed_before={},prev_boot_stage={}",
                        update::FW_VERSION,
                        update::floor(),
                        fw_boot,
                        update::state_name(&mut fw_up),
                        Hx(&my_fp),
                        guard_failed,
                        guard_prev_stage
                    ),
                );
            }
            if line_done && line_len >= 1 && line[0] == b'I' {
                io.log(
                    "PQ_STATUS",
                    format_args!(
                        "min_suite_revocation={},legacy_orv1_allowed={},epoch={},revoked={},reasm_in_use={},stage_len={}",
                        pqs.policy.min_suite(oasis_rt::authority::kind::REVOCATION),
                        pqs.policy.legacy_orv1_allowed(),
                        efs.rev.epoch,
                        efs.rev.revoked.len(),
                        pqs.reasm.in_use(),
                        pqs.stage.len()
                    ),
                );
            }
            // Test-harness factory reset of ALL persistence areas (receiver window,
            // sender lease, revocation list), then a full system reset so the RAM
            // state (incl. the router's revoked set) starts clean too.
            if rx[..n].contains(&b'!') {
                persist::wipe();
                wipe_lease_sectors();
                ef::wipe_rev_sectors();
                pq::wipe_policy_sectors();
                enroll::wipe_sectors();
                io.log("PERSIST_WIPED", format_args!("all persistence erased; resetting"));
                for _ in 0..200_000 {
                    io.poll();
                }
                cortex_m::peripheral::SCB::sys_reset();
            }
            // Part C harness: simulated sensor loss (R14), LED off, E/F status.
            if rx[..n].contains(&b'U') {
                efs.sensor_lost = !efs.sensor_lost;
                io.log("SENSOR", format_args!("lost={}", efs.sensor_lost));
            }
            if rx[..n].contains(&b'L') {
                ef::led_set(false);
                io.log("LED", format_args!("off,pin25={}", ef::led_pin_level()));
            }
            if rx[..n].contains(&b'S') {
                let r = &efs.act.rejects;
                io.log(
                    "EF_STATUS",
                    format_args!(
                        "boot_id={},now_ms={},epoch={},revoked={},sensor_lost={},pin25={},executed={},rejects={}/{}/{}/{}/{}/{}/{}",
                        efs.boot_id,
                        ef::now_ms64(),
                        efs.rev.epoch,
                        efs.rev.revoked.len(),
                        efs.sensor_lost,
                        ef::led_pin_level(),
                        efs.act.executed,
                        r[0], r[1], r[2], r[3], r[4], r[5], r[6]
                    ),
                );
            }
            if rx[..n].contains(&b'b') {
                hal::rom_data::reset_to_usb_boot(0, 0);
            }
            if rx[..n].contains(&b'o') {
                let mid = router.tx_counter();
                let env = router.origin_wrap(b"OASIS-uart-hello");
                send_framed(&mut uart0, &env);
                io.log("ORIGINATED", format_args!("msg_id={},len={}", mid, env.len()));
            }
            // ── v0B originate (payload/counter/network bound). Run on A.
            if rx[..n].contains(&b'O') {
                if let Some(env) = router.origin_wrap_v0b(b"OASIS-v0b-hello") {
                    send_framed(&mut uart0, &env);
                    io.log("V0B_ORIGINATED", format_args!("counter={},len={}", router.tx_counter(), env.len()));
                }
            }
            // ── T8: originate a v0B envelope AND store its exact bytes for a
            //    later byte-for-byte replay (across a reboot of B). Run on A.
            if rx[..n].contains(&b'Z') {
                if let Some(env) = router.origin_wrap_v0b(b"OASIS-v0b-T8") {
                    last_v0b_len = env.len().min(MAX_ENV);
                    last_v0b[..last_v0b_len].copy_from_slice(&env[..last_v0b_len]);
                    send_framed(&mut uart0, &env);
                    io.log("V0B_T8_TX", format_args!("counter={},len={}", router.tx_counter(), env.len()));
                }
            }
            // ── T8: replay the stored envelope byte-for-byte (same counter). After
            //    B reboots with its persisted window, B must DROP this. Run on A.
            if rx[..n].contains(&b'z') {
                if last_v0b_len > 0 {
                    send_framed(&mut uart0, &last_v0b[..last_v0b_len]);
                    io.log("V0B_T8_REPLAY", format_args!("len={}", last_v0b_len));
                }
            }
            // ── v0B pre-CRC bit-flip sweep (50): flip ONE bit before framing so
            //    the CRC is valid over corrupted bytes and the packet reaches B's
            //    verifier. v0B signs the payload+counter+network, so expect ~0
            //    ARRIVED at B (vs v0A's 12/50 accepted). Run on A.
            if rx[..n].contains(&b'G') {
                let seed = now_us() ^ 0x2468_ACE0;
                let mut rng = Rng::new(seed);
                let mut txd = 0u32;
                io.log("V0B_SWEEP", format_args!("Phase (Pre-CRC v0B bitflip,50)"));
                for seq in 1..=50u32 {
                    // Payload ends in a 3-digit sequence number; the relay logs the
                    // last 3 envelope bytes, so every packet is traceable end to end.
                    let mut pl = *b"OASIS-nz-seq000";
                    pl[12] = b'0' + ((seq / 100) % 10) as u8;
                    pl[13] = b'0' + ((seq / 10) % 10) as u8;
                    pl[14] = b'0' + (seq % 10) as u8;
                    if let Some(mut env) = router.origin_wrap_v0b(&pl) {
                        let idx = rng.upto(env.len() as u32) as usize;
                        let bit = rng.upto(8) as u8;
                        let field = match idx {
                            0..=5 => "magic",
                            6..=13 => "network",
                            14..=21 => "origin",
                            22..=29 => "counter",
                            30 => "ttl",
                            31..=32 => "hops",
                            33..=34 => "plen",
                            35..=98 => "sig",
                            _ => "payload",
                        };
                        env[idx] ^= 1u8 << bit; // single-bit corruption, pre-CRC
                        let mut wire = [0u8; MAX_ENV + 8];
                        let wlen = frame_into(&mut wire, &env);
                        uart0.write_full_blocking(&wire[..wlen]);
                        txd += 1;
                        io.log(
                            "SWEEP_TX",
                            format_args!(
                                "seq={:03},counter={},idx={},bit={},field={}",
                                seq,
                                router.tx_counter(),
                                idx,
                                bit,
                                field
                            ),
                        );
                    }
                    io.poll();
                    cortex_m::asm::delay(6_250_000); // ~50 ms
                }
                io.log("V0B_SWEEP_DONE", format_args!("tx={}", txd));
            }
            // ── v0B content-swap (suppression): send a payload-swapped copy with
            //    the ORIGINAL signature, then the real message. B must DROP the
            //    forged one ("bad mesh signature") and ARRIVE/relay the real one.
            //    Run on A. (payloads are equal length: 14 bytes each.)
            if rx[..n].contains(&b'W') {
                if let Some(orig) = router.origin_wrap_v0b(b"OASIS-v0b-REAL") {
                    let mut forged = orig.clone();
                    forged[99..].copy_from_slice(b"OASIS-v0b-FAKE"); // swap payload, keep sig
                    send_framed(&mut uart0, &forged);
                    io.log("V0B_SWAP_TX", format_args!("forged_content_kept_sig,len={}", forged.len()));
                    cortex_m::asm::delay(37_500_000); // ~300 ms gap (avoid FIFO overrun)
                    send_framed(&mut uart0, &orig);
                    io.log("V0B_REAL_TX", format_args!("len={}", orig.len()));
                }
            }
            // ── v0B vs v0A on-chip sign/verify timing (K=5 median, µs). Run on A.
            if rx[..n].contains(&b'Y') {
                let pl = b"OASIS-v0b-timing";
                let mypub = MeshEdPub(my_pk);
                let mut a_sign = [0u32; 5];
                let mut b_sign = [0u32; 5];
                for k in 0..5 {
                    let t0 = now_us();
                    let _ = router.origin_wrap(pl);
                    a_sign[k] = now_us().wrapping_sub(t0);
                    let t0 = now_us();
                    let _ = router.origin_wrap_v0b(pl);
                    b_sign[k] = now_us().wrapping_sub(t0);
                }
                let ea = router.origin_wrap(pl);
                let msg_id_a = u64::from_le_bytes(ea[6..14].try_into().unwrap());
                let fp_a: [u8; 8] = ea[14..22].try_into().unwrap();
                let sig_a: [u8; 64] = ea[25..89].try_into().unwrap();
                let eb = router.origin_wrap_v0b(pl).unwrap();
                let net_b: [u8; 8] = eb[6..14].try_into().unwrap();
                let fp_b: [u8; 8] = eb[14..22].try_into().unwrap();
                let ctr_b = u64::from_le_bytes(eb[22..30].try_into().unwrap());
                let sig_b: [u8; 64] = eb[35..99].try_into().unwrap();
                let pay_b = &eb[99..];
                let mut a_ver = [0u32; 5];
                let mut b_ver = [0u32; 5];
                for k in 0..5 {
                    let t0 = now_us();
                    let _ = mesh_v10_verify(&mypub, msg_id_a, fp_a, &sig_a);
                    a_ver[k] = now_us().wrapping_sub(t0);
                    let t0 = now_us();
                    let _ = mesh_v0b_verify(&mypub, net_b, fp_b, ctr_b, pay_b, &sig_b);
                    b_ver[k] = now_us().wrapping_sub(t0);
                }
                io.log(
                    "V0B_TIMING",
                    format_args!(
                        "v0a_sign_us={},v0b_sign_us={},v0a_verify_us={},v0b_verify_us={}",
                        median5(&mut a_sign), median5(&mut b_sign), median5(&mut a_ver), median5(&mut b_ver)
                    ),
                );
            }
            if rx[..n].contains(&b's') {
                io.log(
                    "STATUS",
                    format_args!(
                        "rx_bytes={},frames={},crc_fails={},tx={},lease_ceiling={},lease_writes={},fp={},peers={}",
                        rx_total,
                        frame_total,
                        crc_fails,
                        router.tx_counter(),
                        lease.ceiling(),
                        lease.writes(),
                        Hx(&my_fp),
                        pqs.registry.entries.len()
                    ),
                );
            }
            // ── T8: flush the v0B counter window to flash (explicit lease
            //    checkpoint). Production coalesces this to ~1 write per 256 msgs;
            //    the test flushes on demand before a power-cut. Run on the relay (B).
            // ── Strict v0B (`K`) refuses v8/v9/v0A; `k` re-allows them for the v0A commands.
            if rx[..n].contains(&b'K') {
                router.set_allow_legacy(false);
                io.log("MODE", format_args!("strict_v0b=true"));
            }
            if rx[..n].contains(&b'k') {
                router.set_allow_legacy(true);
                io.log("MODE", format_args!("strict_v0b=false"));
            }
            if rx[..n].contains(&b'P') {
                match router.counter_tracker_bytes() {
                    Some(b) => {
                        let ok = persist::save(&b);
                        io.log("PERSIST_SAVED", format_args!("ok={},bytes={}", ok, b.len()));
                    }
                    None => io.log("PERSIST_SAVED", format_args!("ok=false,no_tracker")),
                }
            }
            if rx[..n].contains(&b'l') {
                // UART0 internal HW loopback self-test (no pins).
                let regs = unsafe { &*pac::UART0::ptr() };
                regs.uartcr().modify(|_, w| w.lbe().set_bit());
                uart0.write_full_blocking(&[0x55, 0xAA, 0x11, 0x22, 0x33, 0x44]);
                cortex_m::asm::delay(400_000);
                let mut buf = [0u8; 16];
                let got = uart0.read_raw(&mut buf).unwrap_or(0);
                regs.uartcr().modify(|_, w| w.lbe().clear_bit());
                io.log("LOOPTEST", format_args!("uart0_internal_lbe_rx={}", got));
            }
            // ── Task 1: anti-replay. Send the SAME envelope bytes twice (same
            //    msg_id). Downstream dedup must Drop the 2nd. Run on A.
            if rx[..n].contains(&b'R') {
                let env = router.origin_wrap(b"OASIS-replay");
                send_framed(&mut uart0, &env);
                io.log("REPLAY_TX1", format_args!("len={}", env.len()));
                // Gap so B fully processes frame 1 (incl. USB logging) before frame 2
                // arrives — otherwise the 2nd frame overruns B's 32-byte RX FIFO.
                cortex_m::asm::delay(37_500_000); // ~300 ms @125 MHz
                send_framed(&mut uart0, &env); // identical bytes => identical msg_id
                io.log("REPLAY_TX2", format_args!("replay_same_msg_id,len={}", env.len()));
            }
            // ── Task 2: forge / MitM. Build an envelope that CLAIMS origin fp=A
            //    but is signed with a bad seed. Downstream verify must fail. Run on B.
            if rx[..n].contains(&b'F') {
                // Claims the first enrolled peer's fingerprint (A's compiled one before 1.2).
                let claimed = pqs.registry.entries.first().map(|e| e.fp).unwrap_or([0xAA; 8]);
                let mut forger = MeshRouter::new_ed25519_signed(
                    claimed,
                    MeshEdSeed([0x99u8; 32]), // NOT A's real seed
                    MeshPubRegistry::new(),
                );
                let env = forger.origin_wrap(b"FORGED-as-A");
                send_framed(&mut uart0, &env);
                io.log("FORGE_TX", format_args!("claim=A,bad_seed,len={}", env.len()));
            }
            // ── Task 4: TTL. Origin TTL=0 => the receiving node (B) is terminal
            //    (Arrived forward=false) and does NOT relay to C. Run on A.
            if rx[..n].contains(&b'T') {
                let env = router.origin_wrap_with_ttl(b"OASIS-ttl0", 0);
                send_framed(&mut uart0, &env);
                io.log("ORIGINATED_TTL0", format_args!("ttl=0,len={}", env.len()));
            }
            // ── Task 3: flood / disconnect resilience. Burst many fresh envelopes
            //    (each a new msg_id, so each is relayed). Unplug B->C mid-burst:
            //    TX into an open line never errors/panics; reconnect resumes. Run on A.
            if rx[..n].contains(&b'X') {
                let mut count = 0u32;
                for _ in 0..600u32 {
                    let env = router.origin_wrap(b"OASIS-spam");
                    send_framed(&mut uart0, &env);
                    count += 1;
                    if count % 50 == 0 {
                        io.log("SPAM", format_args!("sent={}", count));
                    }
                    io.poll();
                    cortex_m::asm::delay(4_000_000); // ~32 ms between packets
                }
                io.log("SPAM_DONE", format_args!("sent={}", count));
            }
            // ── Noise sweep (fault injection). 300 fresh packets (new msg_id each,
            //    bypassing anti-replay) across 3 escalating corruption levels. The
            //    wire frame is corrupted on a COPY just before TX; A never panics.
            if rx[..n].contains(&b'N') {
                let seed =
                    unsafe { (*pac::TIMER::ptr()).timerawl().read().bits() } ^ 0x1357_9BDF;
                let mut fi = FaultInjector::new(seed);
                let (mut txd, mut dropped, mut truncd, mut cryptod) = (0u32, 0u32, 0u32, 0u32);
                // 50 packets/phase (not the spec's 100): each packet costs a ~341 ms
                // v0A Ed25519 sign on the FPU-less M0+, so 400 would take ~2+ min.
                for pkt in 1..=200u32 {
                    match pkt {
                        1 => io.log("SWEEP", format_args!("Phase 1/4 (Normal,50)")),
                        51 => io.log("SWEEP", format_args!("Phase 2/4 (Medium,50)")),
                        101 => io.log("SWEEP", format_args!("Phase 3/4 (Extreme,50)")),
                        151 => io.log("SWEEP", format_args!("Phase 4/4 (Pre-CRC Crypto Fail,50)")),
                        _ => {}
                    }
                    if pkt <= 150 {
                        // Phases 1-3: corrupt the FINISHED wire frame -> framer CRC catches it.
                        let level = if pkt <= 50 {
                            NoiseLevel::Normal
                        } else if pkt <= 100 {
                            NoiseLevel::Medium
                        } else {
                            NoiseLevel::Extreme
                        };
                        let env = router.origin_wrap(b"OASIS-noise"); // fresh msg_id -> no dedup
                        let mut wire = [0u8; MAX_ENV + 8];
                        let wlen = frame_into(&mut wire, &env);
                        match fi.apply_noise(&mut wire[..wlen], level) {
                            None => dropped += 1, // simulated lost TX
                            Some(txl) => {
                                if txl != wlen {
                                    truncd += 1;
                                }
                                uart0.write_full_blocking(&wire[..txl]);
                                txd += 1;
                            }
                        }
                    } else {
                        // Phase 4: flip ONE bit in the envelope BEFORE framing, so the
                        // frame CRC8 is valid over the corrupted bytes and the packet
                        // reaches B's Ed25519 verifier. (v0A signs only magic||msg_id||
                        // origin_fp, so a flip in ttl/hops/payload is accepted — see report.)
                        let mut env = router.origin_wrap(b"OASIS-crypto-fail");
                        let idx = fi.rng.upto(env.len() as u32) as usize;
                        let bit = fi.rng.upto(8) as u8;
                        env[idx] ^= 1u8 << bit; // single-bit corruption, pre-CRC
                        let mut wire = [0u8; MAX_ENV + 8];
                        let wlen = frame_into(&mut wire, &env); // valid CRC over corrupted env
                        uart0.write_full_blocking(&wire[..wlen]);
                        txd += 1;
                        cryptod += 1;
                    }
                    io.poll();
                    cortex_m::asm::delay(6_250_000); // ~50 ms @125 MHz
                }
                io.log(
                    "SWEEP_DONE",
                    format_args!(
                        "tx={},tx_dropped={},truncated={},phase4_crypto={}",
                        txd, dropped, truncd, cryptod
                    ),
                );
            }
        }

        // UART0 RX (GP1) → deframe → process → log → relay on UART0 TX (GP0).
        let mut tmp = [0u8; 64];
        if let Ok(n) = uart0.read_raw(&mut tmp) {
            rx_total = rx_total.wrapping_add(n as u32);
            for i in 0..n {
                let mut owned = [0u8; MAX_ENV];
                let elen = match deframer.push(tmp[i]) {
                    DfOut::Frame(env) => {
                        frame_total = frame_total.wrapping_add(1);
                        let l = env.len();
                        owned[..l].copy_from_slice(env); // copy out; deframer borrow ends here
                        l
                    }
                    DfOut::CrcFail => {
                        // Wire corruption caught at the framer (CRC8).
                        crc_fails = crc_fails.wrapping_add(1);
                        // Log every failure so the archived log carries the exact total.
                        io.log("FRAMER_CRC_FAIL", format_args!("count={}", crc_fails));
                        0
                    }
                    DfOut::Pending => 0,
                };
                if elen == 0 {
                    continue;
                }
                // Per-packet receive trace: frame index, counter field as received
                // and the last 3 envelope bytes (the sweep's sequence number), so a
                // lost packet is identifiable rather than merely absent.
                {
                    let e = &owned[..elen];
                    let ctr = if elen >= 30 {
                        u64::from_le_bytes([e[22], e[23], e[24], e[25], e[26], e[27], e[28], e[29]])
                    } else {
                        0
                    };
                    let mut seq = [b'?'; 3];
                    if elen >= 3 {
                        seq.copy_from_slice(&e[elen - 3..elen]);
                    }
                    for b in seq.iter_mut() {
                        if !b.is_ascii_graphic() {
                            *b = b'?';
                        }
                    }
                    io.log(
                        "RXF",
                        format_args!(
                            "n={},len={},ctr={},seq={}",
                            frame_total,
                            elen,
                            ctr,
                            core::str::from_utf8(&seq).unwrap_or("???")
                        ),
                    );
                }
                match router.process(&owned[..elen]) {
                    MeshDecision::Arrived {
                        msg_id,
                        hops_seen,
                        forward,
                        envelope,
                    } => {
                        io.log(
                            "ARRIVED",
                            format_args!(
                                "msg_id={},hops={},sig=verified,forward={}",
                                msg_id, hops_seen, forward
                            ),
                        );
                        let inner = inner_slice(&envelope);
                        let mut origin = [0u8; 8];
                        if envelope.len() >= 22 {
                            origin.copy_from_slice(&envelope[14..22]);
                        }
                        let mut relay = forward;
                        match ef::content_kind(inner) {
                            // Signed revocation (spec E.3): verify, persist, apply; forward
                            // only a fresh epoch, exactly once. Rejects/duplicates stop here.
                            // Phase 1.1: once revocations require the hybrid suite, a legacy
                            // Ed25519-only ORV1 list is refused before any verification.
                            "ORV1" if !pqs.policy.legacy_orv1_allowed() => {
                                io.log(
                                    "REV",
                                    format_args!(
                                        "decision=LegacyRefused,min_suite_revocation={},epoch={},revoked={}",
                                        pqs.policy.min_suite(oasis_rt::authority::kind::REVOCATION),
                                        efs.rev.epoch,
                                        efs.rev.revoked.len()
                                    ),
                                );
                                relay = false;
                            }
                            // Phase 1.1: authority fragment. Never relayed as-is: the whole
                            // message is reassembled and verified first, then re-originated
                            // under this node's own counters (store-and-forward, spec §3).
                            "OFR1" => {
                                relay = false;
                                let hdr = oasis_rt::fragment::parse_ofr1(inner).map(|(h, _)| (h.idx, h.count));
                                let out = pqs.push_fragment(origin, inner);
                                let (idx, count) = hdr.unwrap_or((0, 0));
                                let tag = match &out {
                                    FragOutcome::Incomplete => "incomplete",
                                    FragOutcome::Duplicate => "duplicate",
                                    FragOutcome::Complete(_) => "complete",
                                    FragOutcome::Rejected(oasis_rt::fragment::FragReject::Malformed) => "rejected_malformed",
                                    FragOutcome::Rejected(oasis_rt::fragment::FragReject::NoSlot) => "rejected_no_slot",
                                    FragOutcome::Rejected(oasis_rt::fragment::FragReject::HashMismatch) => {
                                        "rejected_hash_mismatch"
                                    }
                                };
                                io.log(
                                    "FRAG_RX",
                                    format_args!("origin={},idx={},count={},outcome={}", Hx(&origin), idx, count, tag),
                                );
                                if let FragOutcome::Complete(msg) = out {
                                    let (done, g) = pqs.complete(&mut efs, &mut router, &msg);
                                    finish_authority(&mut io, &mut router, &mut uart0, &mut lease, &mut lstore, &pqs, &efs, &msg, done, &g);
                                }
                            }
                            "ORV1" => {
                                let d = efs.ingest_orv1(&mut router, inner, &pqs.owner.current);
                                if d == oasis_rt::mesh_revocation::RevDecision::Applied {
                                    pqs.rev_signer = pq::RevSigner::Current;
                                }
                                io.log(
                                    "REV",
                                    format_args!(
                                        "decision={:?},epoch={},revoked={},persisted_before_apply={}",
                                        d,
                                        efs.rev.epoch,
                                        efs.rev.revoked.len(),
                                        d == oasis_rt::mesh_revocation::RevDecision::Applied
                                    ),
                                );
                                relay = forward && d == oasis_rt::mesh_revocation::RevDecision::Applied;
                            }
                            // Actuation command for the actuator this board hosts (C).
                            "OAC1" if BOARD_ID == "C" => {
                                if let Some(cmd) = parse_oac1(inner) {
                                    let d = efs.decide(&router, &pqs.registry, &origin, true, &cmd);
                                    io.log(
                                        "ACT",
                                        format_args!(
                                            "decision={:?},seq={},origin={:02x},entropy={:.3},pin25={}",
                                            d,
                                            cmd.cmd_seq,
                                            origin[0],
                                            efs.last_entropy,
                                            ef::led_pin_level()
                                        ),
                                    );
                                }
                                relay = false; // consumed by the actuator
                            }
                            _ => {}
                        }
                        if relay {
                            send_framed(&mut uart0, &envelope);
                            io.log("RELAYED", format_args!("msg_id={},hops={}", msg_id, hops_seen));
                        }
                    }
                    MeshDecision::Drop(reason) => {
                        io.log("DROP", format_args!("{}", reason));
                        // A command that fails v0B (replay, forgery, revoked origin) is
                        // still logged as a gate decision so its outcome is on record.
                        let e = &owned[..elen];
                        if BOARD_ID == "C" && elen >= 99 + oasis_rt::actuation::OAC1_LEN {
                            if let Some(cmd) = parse_oac1(&e[99..99 + oasis_rt::actuation::OAC1_LEN]) {
                                let mut origin = [0u8; 8];
                                origin.copy_from_slice(&e[14..22]);
                                let d = efs.decide(&router, &pqs.registry, &origin, false, &cmd);
                                io.log(
                                    "ACT",
                                    format_args!(
                                        "decision={:?},v0b_drop={},seq={},origin={:02x},pin25={}",
                                        d,
                                        reason,
                                        cmd.cmd_seq,
                                        origin[0],
                                        ef::led_pin_level()
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// After an authority message went through the gate (received over the mesh, or
/// loaded with `@L`): log the decision and the resulting state, then re-originate it
/// if it changed this node's state, under a lease reservation.
#[allow(clippy::too_many_arguments)]
fn finish_authority(
    io: &mut Io,
    router: &mut MeshRouter,
    uart: &mut Uart0,
    lease: &mut TxLease,
    lstore: &mut DualSlotStore<FlashSlots>,
    pqs: &pq::Pq,
    efs: &ef::Ef,
    msg: &[u8],
    done: pq::Done,
    g: &pq::Gate,
) {
    io.log(
        "AUTH",
        format_args!(
            "kind={},suite={},len={},decision={:?},verify_us={},stack={},forward={}",
            g.kind,
            g.suite,
            msg.len(),
            done,
            g.verify_us,
            g.stack,
            done.forward()
        ),
    );
    match done {
        pq::Done::Revocation(_) => {
            io.log("REV", format_args!("source=OAU1,epoch={},revoked={}", efs.rev.epoch, efs.rev.revoked.len()))
        }
        pq::Done::Enroll(_) | pq::Done::Own(_) => io.log(
            "STATE",
            format_args!(
                "owner_seq={},owner_ed={},pending_offer={},peers={},rev_signer={:?}",
                pqs.owner.seq,
                Hx(&pqs.owner.current.ed25519[..8]),
                pqs.pending.as_ref().map_or(0, |p| p.seq),
                pqs.registry.entries.len(),
                pqs.rev_signer
            ),
        ),
        _ => {}
    }
    if done.forward() {
        // Re-originated fragments use this node's counters: make them durable first.
        let need = router.tx_counter().saturating_add(oasis_rt::fragment::MAX_FRAGMENTS as u64);
        if lease.ensure(need, lstore) {
            send_fragments(io, router, uart, msg, None);
        } else {
            io.log("LEASE_FAIL", format_args!("re-origination skipped"));
        }
    }
}

/// Fragment `msg` into `OFR1` fragments that fit one frame and originate each in its
/// own v0B envelope. `tamper = Some(i)`: flip the last byte of fragment `i` AFTER
/// fragmentation (test only), so its v0B signature is valid but the reassembled
/// message no longer hashes to `msg_id`.
/// Pause after each fragment. The wire has no flow control and the receiver polls
/// its 32-byte RX FIFO only between frames, so the sender's period must exceed the
/// receiver's per-frame work: v0B sign 178 ms + 26 ms on the wire, against v0B
/// verify 185 ms + logging. Without this gap the margin was ~10 ms and a Phase 1.2
/// run lost 9 of 14 fragments to FIFO overruns (evidence/silicon/2026-10-06/enroll/).
const FRAG_GAP_US: u32 = 100_000;

fn send_fragments(io: &mut Io, router: &mut MeshRouter, uart: &mut Uart0, msg: &[u8], tamper: Option<usize>) {
    let mut frags = match fragment(msg, pq::FRAG_MAX) {
        Some(f) => f,
        None => {
            io.log("FRAG_TX_FAIL", format_args!("len={}", msg.len()));
            return;
        }
    };
    if let Some(f) = tamper.and_then(|t| frags.get_mut(t)) {
        let n = f.len();
        f[n - 1] ^= 0x01;
    }
    let count = frags.len();
    for (i, f) in frags.iter().enumerate() {
        match router.origin_wrap_v0b(f) {
            Some(env) => {
                send_framed(uart, &env);
                let t0 = now_us();
                while now_us().wrapping_sub(t0) < FRAG_GAP_US {
                    io.poll();
                }
                io.log(
                    "FRAG_TX",
                    format_args!(
                        "idx={},count={},counter={},len={},tampered={}",
                        i,
                        count,
                        router.tx_counter(),
                        env.len(),
                        tamper == Some(i)
                    ),
                );
            }
            None => {
                io.log("FRAG_TX_FAIL", format_args!("idx={}", i));
                return;
            }
        }
    }
}

// ── USB-CDC logging ──────────────────────────────────────────────────────────
struct Io<'a> {
    usb_dev: UsbDevice<'a, Usb>,
    serial: SerialPort<'a, Usb>,
}
impl Io<'_> {
    fn poll(&mut self) {
        let _ = self.usb_dev.poll(&mut [&mut self.serial]);
    }
    fn log(&mut self, event: &str, value: core::fmt::Arguments) {
        let mut l = Line::new();
        let _ = write!(l, "OASIS|{}|UART|{}|{}|mesh|{}\r\n", BOARD_ID, event, value, GIT_HASH);
        let mut data: &[u8] = l.bytes();
        let mut guard = 0u32;
        while !data.is_empty() && guard < 400_000 {
            guard += 1;
            self.poll();
            match self.serial.write(data) {
                Ok(0) => {}
                Ok(k) => data = &data[k..],
                Err(usbd_serial::UsbError::WouldBlock) => {}
                Err(_) => break,
            }
        }
        let _ = self.serial.flush();
    }
}

struct Line {
    b: [u8; 320],
    n: usize,
}
impl Line {
    fn new() -> Self {
        Line { b: [0; 320], n: 0 }
    }
    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}
impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let by = s.as_bytes();
        let end = (self.n + by.len()).min(self.b.len());
        self.b[self.n..end].copy_from_slice(&by[..end - self.n]);
        self.n = end;
        Ok(())
    }
}

// ── UART framing: SYNC(0x55,0xAA) + len(u16 LE) + data + crc8 ─────────────────
fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
    }
    crc
}

fn send_framed(uart: &mut Uart0, env: &[u8]) {
    let len = env.len() as u16;
    let hdr = [0x55u8, 0xAA, (len & 0xFF) as u8, (len >> 8) as u8];
    uart.write_full_blocking(&hdr);
    uart.write_full_blocking(env);
    uart.write_full_blocking(&[crc8(env)]);
}

/// Build the on-the-wire frame (SYNC+len+env+crc8) into `out`, return its length.
/// Used by the fault-injection sweep so the frame bytes can be corrupted before TX.
fn frame_into(out: &mut [u8], env: &[u8]) -> usize {
    let len = env.len() as u16;
    out[0] = 0x55;
    out[1] = 0xAA;
    out[2] = (len & 0xFF) as u8;
    out[3] = (len >> 8) as u8;
    out[4..4 + env.len()].copy_from_slice(env);
    out[4 + env.len()] = crc8(env);
    4 + env.len() + 1
}

// ── Fault injection (no_std, no external RNG crate) ──────────────────────────
/// Xorshift32 PRNG. Seeded from the free-running RP2040 TIMER (variation per run)
/// XOR a fixed constant; deterministic given the same seed.
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self {
        Rng(if seed == 0 { 0x1357_9BDF } else { seed })
    }
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn pct(&mut self, p: u8) -> bool {
        (self.next() % 100) < p as u32
    }
    fn upto(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

#[derive(Clone, Copy)]
enum NoiseLevel {
    Normal,
    Medium,
    Extreme,
}

struct FaultInjector {
    rng: Rng,
}
impl FaultInjector {
    fn new(seed: u32) -> Self {
        FaultInjector { rng: Rng::new(seed) }
    }
    /// Corrupt the on-the-wire frame `buf` in place. Returns the (possibly
    /// truncated) length to transmit, or `None` if the frame is dropped entirely.
    fn apply_noise(&mut self, buf: &mut [u8], level: NoiseLevel) -> Option<usize> {
        let len = buf.len();
        if len == 0 {
            return Some(0);
        }
        // (corrupt%, max bit-flips, truncate%, drop%)
        let (corrupt_pct, max_flips, trunc_pct, drop_pct) = match level {
            NoiseLevel::Normal => (0u8, 0u32, 0u8, 0u8),
            NoiseLevel::Medium => (5, 1, 2, 0),
            NoiseLevel::Extreme => (20, 4, 0, 15),
        };
        if drop_pct > 0 && self.rng.pct(drop_pct) {
            return None; // simulate a lost TX
        }
        if corrupt_pct > 0 && self.rng.pct(corrupt_pct) {
            let flips = 1 + self.rng.upto(max_flips); // 1..=max_flips bit flips
            for _ in 0..flips {
                let idx = self.rng.upto(len as u32) as usize;
                buf[idx] ^= 1u8 << (self.rng.upto(8) as u8);
            }
        }
        if trunc_pct > 0 && self.rng.pct(trunc_pct) {
            return Some((1 + self.rng.upto(len as u32) as usize).min(len)); // cut the tail
        }
        Some(len)
    }
}

const MAX_ENV: usize = 300;
struct Deframer {
    state: u8,
    len: usize,
    idx: usize,
    buf: [u8; MAX_ENV],
}
impl Deframer {
    fn new() -> Self {
        Deframer { state: 0, len: 0, idx: 0, buf: [0; MAX_ENV] }
    }
    fn push(&mut self, b: u8) -> DfOut<'_> {
        match self.state {
            0 => {
                if b == 0x55 {
                    self.state = 1;
                }
            }
            1 => self.state = if b == 0xAA { 2 } else { 0 },
            2 => {
                self.len = b as usize;
                self.state = 3;
            }
            3 => {
                self.len |= (b as usize) << 8;
                self.idx = 0;
                self.state = if self.len == 0 || self.len > MAX_ENV { 0 } else { 4 };
            }
            4 => {
                self.buf[self.idx] = b;
                self.idx += 1;
                if self.idx >= self.len {
                    self.state = 5;
                }
            }
            5 => {
                self.state = 0;
                return if crc8(&self.buf[..self.len]) == b {
                    DfOut::Frame(&self.buf[..self.len])
                } else {
                    DfOut::CrcFail // corruption caught at the framer
                };
            }
            _ => self.state = 0,
        }
        DfOut::Pending
    }
}

enum DfOut<'a> {
    Frame(&'a [u8]),
    CrcFail,
    Pending,
}
