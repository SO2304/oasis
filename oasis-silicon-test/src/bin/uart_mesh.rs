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
//! `b`=reboot to BOOTSEL.

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

use oasis_rt::mesh::{
    mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
};

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[panic_handler]
fn on_panic(_: &core::panic::PanicInfo) -> ! {
    hal::rom_data::reset_to_usb_boot(0, 0);
    loop {}
}
#[cortex_m_rt::exception]
unsafe fn HardFault(_ef: &cortex_m_rt::ExceptionFrame) -> ! {
    hal::rom_data::reset_to_usb_boot(0, 0);
    loop {}
}

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XTAL_HZ: u32 = 12_000_000;
const BOARD_ID: &str = env!("OASIS_BOARD_ID");
const GIT_HASH: &str = env!("OASIS_GIT_HASH");
type Usb = hal::usb::UsbBus;

// UART0 pins: GP0 = TX, GP1 = RX.
type UartPins0 = (
    hal::gpio::Pin<hal::gpio::bank0::Gpio0, FunctionUart, hal::gpio::PullDown>,
    hal::gpio::Pin<hal::gpio::bank0::Gpio1, FunctionUart, hal::gpio::PullDown>,
);
type Uart0 = UartPeripheral<hal::uart::Enabled, pac::UART0, UartPins0>;

fn fp_for(id: &str) -> [u8; 8] {
    match id {
        "A" => [0xAA; 8],
        "B" => [0xBB; 8],
        "C" => [0xCC; 8],
        _ => [0xDD; 8],
    }
}
fn seed_for(id: &str) -> [u8; 32] {
    match id {
        "A" => [0x11; 32],
        "B" => [0x22; 32],
        "C" => [0x33; 32],
        _ => [0x44; 32],
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

    let mut registry = MeshPubRegistry::new();
    for id in ["A", "B", "C"] {
        if let Ok(pk) = mesh_v10_pubkey_from_seed(&MeshEdSeed(seed_for(id))) {
            registry.insert(fp_for(id), pk);
        }
    }
    let mut router =
        MeshRouter::new_ed25519_signed(fp_for(BOARD_ID), MeshEdSeed(seed_for(BOARD_ID)), registry);

    let mut io = Io { usb_dev, serial };
    let mut deframer = Deframer::new();
    let mut rx_total: u32 = 0;
    let mut frame_total: u32 = 0;
    io.log("BOOT", format_args!("uart-mesh fp={} UART0 tx=GP0 rx=GP1 @115200", BOARD_ID));

    loop {
        io.poll();

        let mut rx = [0u8; 16];
        if let Ok(n) = io.serial.read(&mut rx) {
            if rx[..n].contains(&b'b') {
                hal::rom_data::reset_to_usb_boot(0, 0);
            }
            if rx[..n].contains(&b'o') {
                let mid = router.tx_counter();
                let env = router.origin_wrap(b"OASIS-uart-hello");
                send_framed(&mut uart0, &env);
                io.log("ORIGINATED", format_args!("msg_id={},len={}", mid, env.len()));
            }
            if rx[..n].contains(&b's') {
                io.log(
                    "STATUS",
                    format_args!("rx_bytes={},frames={}", rx_total, frame_total),
                );
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
                let mut forger = MeshRouter::new_ed25519_signed(
                    fp_for("A"),
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
        }

        // UART0 RX (GP1) → deframe → process → log → relay on UART0 TX (GP0).
        let mut tmp = [0u8; 64];
        if let Ok(n) = uart0.read_raw(&mut tmp) {
            rx_total = rx_total.wrapping_add(n as u32);
            for i in 0..n {
                if let Some(env) = deframer.push(tmp[i]) {
                    frame_total = frame_total.wrapping_add(1);
                    // Copy out so the deframer borrow ends before we TX.
                    let mut owned = [0u8; MAX_ENV];
                    let elen = env.len();
                    owned[..elen].copy_from_slice(env);
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
                            if forward {
                                send_framed(&mut uart0, &envelope);
                                io.log("RELAYED", format_args!("msg_id={},hops={}", msg_id, hops_seen));
                            }
                        }
                        MeshDecision::Drop(reason) => {
                            io.log("DROP", format_args!("{}", reason));
                        }
                    }
                }
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
    b: [u8; 160],
    n: usize,
}
impl Line {
    fn new() -> Self {
        Line { b: [0; 160], n: 0 }
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
    fn push(&mut self, b: u8) -> Option<&[u8]> {
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
                if crc8(&self.buf[..self.len]) == b {
                    return Some(&self.buf[..self.len]);
                }
            }
            _ => self.state = 0,
        }
        None
    }
}
