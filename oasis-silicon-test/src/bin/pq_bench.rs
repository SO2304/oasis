//! OASIS Phase 1.1 bake-off firmware (RP2040): ML-DSA-44 verification on silicon.
//!
//! Spec: docs/specs/PQ_AUTHORITY_SPEC.md §4-5. Verifies a hybrid `OAU1` revocation
//! signed on the PC (`oasis-operator-key/examples/pq_payloads.rs`, public blobs in
//! `src/pq/`) and reports, one parsable line each:
//!   OASIS|<board>|<test>|PASS|FAIL|<value>|<unit>|<git_hash>
//! - correctness: valid accepted, one flipped bit refused (each verifier);
//! - time per verify, K = 5 (median/min/max, hardware 1 MHz timer);
//! - peak stack per verify (stack painted with a known word, then scanned).
//!
//! Verifiers linked: `bench_rc` = RustCrypto `ml-dsa` (through
//! `oasis_rt::authority`), `bench_lx` = `libcrux-ml-dsa`. Flash size is compared by
//! building with one feature at a time. USB-CDC: `r` = run, `b` = BOOTSEL.

#![no_std]
#![no_main]

extern crate alloc;

use core::fmt::Write as _;
use rp2040_hal as hal;

use embedded_alloc::Heap;
use hal::clocks::Clock;
use hal::pac;
use usb_device::class_prelude::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDevice};
use usb_device::prelude::{UsbDeviceBuilder, UsbVidPid};
use usbd_serial::SerialPort;

#[allow(unused_imports)]
use oasis_rt::authority::{
    ed25519_verify, mldsa44_verify, parse_oau1, signed_message, verify_authority, AuthPolicy,
    AuthorityKeys, AUTH_DOMAIN, MLDSA44_PK_LEN, MLDSA44_SIG_LEN,
};

#[panic_handler]
fn on_panic(_info: &core::panic::PanicInfo) -> ! {
    hal::rom_data::reset_to_usb_boot(0, 0);
    loop {}
}
#[cortex_m_rt::exception]
unsafe fn HardFault(_ef: &cortex_m_rt::ExceptionFrame) -> ! {
    hal::rom_data::reset_to_usb_boot(0, 0);
    loop {}
}

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XTAL_HZ: u32 = 12_000_000;
const BOARD_ID: &str = env!("OASIS_BOARD_ID");
const GIT_HASH: &str = env!("OASIS_GIT_HASH");
const NETWORK_ID: [u8; 8] = *b"OASISnet";
/// Ed25519 operator key (same as ef.rs / ef_payloads.rs).
const OPERATOR_ED_PUB: [u8; 32] = [
    0x0b, 0xee, 0xf5, 0xa9, 0xe6, 0x79, 0xe6, 0xa3, 0xe1, 0x34, 0xfe, 0x27, 0x83, 0x7b, 0xff, 0x32,
    0xc7, 0xcb, 0x5f, 0x5d, 0x44, 0xea, 0x09, 0xbc, 0xb0, 0xe5, 0x42, 0xba, 0xd6, 0xa4, 0xc0, 0xcc,
];
static OPERATOR_MLDSA_PK: &[u8; MLDSA44_PK_LEN] = include_bytes!("../pq/op_mldsa44.pk");
static REV_HYBRID: &[u8] = include_bytes!("../pq/rev_hybrid_e1.oau1");

type Usb = hal::usb::UsbBus;

#[hal::entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        // Only verify_authority's small signed-message Vec uses the heap; the
        // verifiers themselves are allocation-free. Keep the rest for the stack.
        const HEAP_SIZE: usize = 16 * 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }
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
    let sysclk_hz = clocks.system_clock.freq().to_Hz();
    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
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
            .product("oasis-pq-bench")
            .serial_number(BOARD_ID)])
        .unwrap()
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();
    let mut io = Io { usb_dev, serial };
    loop {
        io.poll();
        let mut rx = [0u8; 16];
        if let Ok(n) = io.serial.read(&mut rx) {
            if rx[..n].contains(&b'b') {
                hal::rom_data::reset_to_usb_boot(0, 0);
            }
            if rx[..n].contains(&b'r') {
                run(&mut io, &timer, sysclk_hz);
            }
        }
    }
}

fn run(io: &mut Io, timer: &hal::Timer, sysclk_hz: u32) {
    emit(
        io,
        "PQ_BOOT",
        true,
        format_args!("sysclk={}Hz", sysclk_hz),
        "info",
    );
    let linked = (cfg!(feature = "bench_rc"), cfg!(feature = "bench_lx"));
    emit(
        io,
        "PQ_LINKED",
        true,
        format_args!("rc={} lx={}", linked.0, linked.1),
        "info",
    );

    let p = match parse_oau1(REV_HYBRID) {
        Ok(p) => p,
        Err(_) => {
            emit(
                io,
                "PQ_PARSE",
                false,
                format_args!("{}B", REV_HYBRID.len()),
                "bytes",
            );
            return;
        }
    };
    let msg = signed_message(p.kind, p.suite, &p.network_id, p.content);
    let mut sig = [0u8; MLDSA44_SIG_LEN];
    sig.copy_from_slice(p.mldsa44_sig);
    let mut bad = sig;
    bad[1000] ^= 0x01;
    emit(
        io,
        "PQ_PARSE",
        true,
        format_args!("{}B msg={}B", REV_HYBRID.len(), msg.len()),
        "bytes",
    );

    // Ed25519 alone, for scale.
    let ed_ok = ed25519_verify(&OPERATOR_ED_PUB, &msg, p.ed25519_sig);
    emit(io, "PQ_ED_OK", ed_ok, format_args!("{}", ed_ok), "bool");
    let (m, lo, hi) = timeit(io, timer, || {
        core::hint::black_box(ed25519_verify(&OPERATOR_ED_PUB, &msg, p.ed25519_sig));
    });
    emit(
        io,
        "PQ_ED_TIME",
        true,
        format_args!("{}/{}/{}", m, lo, hi),
        "us_med/min/max_K5",
    );
    let st = stack_peak(|| {
        core::hint::black_box(ed25519_verify(&OPERATOR_ED_PUB, &msg, p.ed25519_sig));
    });
    emit(io, "PQ_ED_STACK", true, format_args!("{}", st), "bytes");

    #[cfg(feature = "bench_rc")]
    {
        let pk: &[u8] = OPERATOR_MLDSA_PK;
        let ok = mldsa44_verify(pk, &msg, AUTH_DOMAIN, &sig);
        let rej = !mldsa44_verify(pk, &msg, AUTH_DOMAIN, &bad);
        emit(io, "PQ_RC_OK", ok, format_args!("{}", ok), "bool");
        emit(
            io,
            "PQ_RC_TAMPER",
            rej,
            format_args!("refused={}", rej),
            "bool",
        );
        let (m, lo, hi) = timeit(io, timer, || {
            core::hint::black_box(mldsa44_verify(pk, &msg, AUTH_DOMAIN, &sig));
        });
        emit(
            io,
            "PQ_RC_TIME",
            ok,
            format_args!("{}/{}/{}", m, lo, hi),
            "us_med/min/max_K5",
        );
        let st = stack_peak(|| {
            core::hint::black_box(mldsa44_verify(pk, &msg, AUTH_DOMAIN, &sig));
        });
        emit(io, "PQ_RC_STACK", true, format_args!("{}", st), "bytes");

        // The full gate as the firmware will run it: parse, precheck, ML-DSA, Ed25519.
        let keys = AuthorityKeys {
            ed25519: OPERATOR_ED_PUB,
            mldsa44: OPERATOR_MLDSA_PK,
        };
        let pol = AuthPolicy::default();
        let full = verify_authority(&pol, &NETWORK_ID, &keys, REV_HYBRID).is_ok();
        emit(io, "PQ_AUTH_OK", full, format_args!("{}", full), "bool");
        let (m, lo, hi) = timeit(io, timer, || {
            core::hint::black_box(verify_authority(&pol, &NETWORK_ID, &keys, REV_HYBRID).is_ok());
        });
        emit(
            io,
            "PQ_AUTH_TIME",
            full,
            format_args!("{}/{}/{}", m, lo, hi),
            "us_med/min/max_K5",
        );
        let st = stack_peak(|| {
            core::hint::black_box(verify_authority(&pol, &NETWORK_ID, &keys, REV_HYBRID).is_ok());
        });
        emit(io, "PQ_AUTH_STACK", true, format_args!("{}", st), "bytes");
    }

    #[cfg(feature = "bench_lx")]
    {
        use libcrux_ml_dsa::ml_dsa_44 as lx;
        let lx_verify = |s: &[u8; MLDSA44_SIG_LEN]| -> bool {
            let vk = lx::MLDSA44VerificationKey::new(*OPERATOR_MLDSA_PK);
            let sg = lx::MLDSA44Signature::new(*s);
            lx::verify(&vk, &msg, AUTH_DOMAIN, &sg).is_ok()
        };
        let ok = lx_verify(&sig);
        let rej = !lx_verify(&bad);
        emit(io, "PQ_LX_OK", ok, format_args!("{}", ok), "bool");
        emit(
            io,
            "PQ_LX_TAMPER",
            rej,
            format_args!("refused={}", rej),
            "bool",
        );
        let (m, lo, hi) = timeit(io, timer, || {
            core::hint::black_box(lx_verify(&sig));
        });
        emit(
            io,
            "PQ_LX_TIME",
            ok,
            format_args!("{}/{}/{}", m, lo, hi),
            "us_med/min/max_K5",
        );
        let st = stack_peak(|| {
            core::hint::black_box(lx_verify(&sig));
        });
        emit(io, "PQ_LX_STACK", true, format_args!("{}", st), "bytes");
    }

    emit(
        io,
        "PQ_HEAP",
        true,
        format_args!("used={}", HEAP.used()),
        "bytes",
    );
    emit(io, "PQ_END", true, format_args!("-"), "end");
}

// ── measurement ─────────────────────────────────────────────────────────────

/// Median/min/max microseconds of `f` over K = 5 single runs.
fn timeit<F: FnMut()>(io: &mut Io, timer: &hal::Timer, mut f: F) -> (u64, u64, u64) {
    let mut s = [0u64; 5];
    for slot in s.iter_mut() {
        io.poll(); // outside the measured window
        let t0 = timer.get_counter().ticks();
        f();
        *slot = timer.get_counter().ticks().wrapping_sub(t0);
    }
    s.sort_unstable();
    (s[2], s[0], s[4])
}

const PAINT: u32 = 0xA5C3_5A3C;
extern "C" {
    static _stack_end: u32;
}

/// Peak stack used by `f`, in bytes below the caller's stack pointer: paint the
/// free stack with a known word, run `f`, then find the lowest overwritten word.
/// USB is polled (no interrupts), so nothing else runs on this stack meanwhile.
#[inline(never)]
fn stack_peak<F: FnMut()>(mut f: F) -> usize {
    let floor = (core::ptr::addr_of!(_stack_end) as usize + 64) & !3;
    let sp = cortex_m::register::msp::read() as usize;
    let top = (sp - 128) & !3; // keep clear of this frame
    let mut a = floor;
    while a < top {
        unsafe { core::ptr::write_volatile(a as *mut u32, PAINT) };
        a += 4;
    }
    f();
    let mut a = floor;
    while a < top && unsafe { core::ptr::read_volatile(a as *const u32) } == PAINT {
        a += 4;
    }
    if a == floor {
        usize::MAX // reached the floor: possible overflow, report it as such
    } else {
        sp - a
    }
}

// ── USB plumbing (same as main.rs) ──────────────────────────────────────────
struct Io<'a> {
    usb_dev: UsbDevice<'a, Usb>,
    serial: SerialPort<'a, Usb>,
}
impl Io<'_> {
    fn poll(&mut self) {
        let _ = self.usb_dev.poll(&mut [&mut self.serial]);
    }
    fn write_all(&mut self, mut data: &[u8]) {
        let mut guard: u32 = 0;
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
    b: [u8; 256],
    n: usize,
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

fn emit(io: &mut Io, test: &str, pass: bool, value: core::fmt::Arguments, unit: &str) {
    let mut l = Line { b: [0; 256], n: 0 };
    let _ = write!(
        l,
        "OASIS|{}|{}|{}|{}|{}|{}\r\n",
        BOARD_ID,
        test,
        if pass { "PASS" } else { "FAIL" },
        value,
        unit,
        GIT_HASH
    );
    io.write_all(&l.b[..l.n]);
}
