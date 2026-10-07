//! Phase 1.4: an existing ("brownfield") Modbus RTU device on an RP2040 (board A).
//!
//! Spec: docs/specs/MODBUS_GATEWAY_SPEC.md §2. No OASIS code, no key, no
//! authentication: an RTU slave (unit 0x11) built on `rmodbus`, an independent,
//! existing Modbus library. It executes every well-formed frame, like the devices
//! FrostyGoop wrote to. It is the ground truth of the gateway tests: it counts every
//! byte and every receive error on its bus and logs every frame.
//!
//! Bus: UART1, TX = GP4, RX = GP5, 19 200 baud 8E1, TTL 3.3 V. A frame ends after
//! 3.5 character times of silence (11 bits per character: 2.005 ms).
//! GP0 is held high (UART idle): the A->B mesh wire stays quiet.
//! USB-CDC: `@D` status, `@Z<hex>` feed bytes to the parser locally (control, never
//! counted as bus traffic, no response on the bus), `b` BOOTSEL.
//! Logs: `OASIS|<board>|MODBUS|<event>|<value>|device|<git_hash>`.

#![no_std]
#![no_main]

use core::fmt::Write as _;
use rp2040_hal as hal;

use hal::clocks::Clock;
use hal::fugit::RateExtU32;
use hal::gpio::{FunctionUart, PinState, PullUp};
use hal::pac;
use hal::uart::{DataBits, Parity, StopBits, UartConfig, UartPeripheral};
use rmodbus::server::context::ModbusContext;
use rmodbus::server::storage::ModbusStorage;
use rmodbus::server::ModbusFrame;
use rmodbus::{ModbusFrameBuf, ModbusProto};
use usb_device::class_prelude::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDevice};
use usb_device::prelude::{UsbDeviceBuilder, UsbVidPid};
use usbd_serial::SerialPort;

#[panic_handler]
fn on_panic(_info: &core::panic::PanicInfo) -> ! {
    hal::rom_data::reset_to_usb_boot(0, 0);
    loop {}
}

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XTAL_HZ: u32 = 12_000_000;
const BOARD_ID: &str = env!("OASIS_BOARD_ID");
const GIT_HASH: &str = env!("OASIS_GIT_HASH");
const UNIT: u8 = 0x11;
const BAUD: u32 = 19_200;
/// 3.5 characters of 11 bits at 19 200 baud, rounded up (µs).
const FRAME_GAP_US: u64 = 2_006;
type Usb = hal::usb::UsbBus;
/// 16 coils, 16 discrete inputs, 16 input registers, 64 holding registers.
type Store = ModbusStorage<16, 16, 16, 64>;

#[hal::entry]
fn main() -> ! {
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
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // The A->B mesh wire (GP0) stays at the UART idle level; this firmware never uses it.
    let _gp0 = pins.gpio0.into_push_pull_output_in_state(PinState::High);
    // RX pulled up: an idle or unwired line reads as idle, not as a stream of breaks.
    let uart1 = UartPeripheral::new(
        pac.UART1,
        (
            pins.gpio4.into_function::<FunctionUart>(),
            pins.gpio5
                .into_pull_type::<PullUp>()
                .into_function::<FunctionUart>(),
        ),
        &mut pac.RESETS,
    )
    .enable(
        UartConfig::new(
            BAUD.Hz(),
            DataBits::Eight,
            Some(Parity::Even),
            StopBits::One,
        ),
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
            .product("modbus-device")
            .serial_number(BOARD_ID)])
        .unwrap()
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();
    let mut io = Io { usb_dev, serial };

    let mut store = Store::default();
    let mut st = Stats::default();
    let mut frame = [0u8; 256];
    let mut flen = 0usize;
    let mut last_rx_us = 0u64;
    let mut line = [0u8; 600];
    let mut line_len = 0usize;
    let mut in_line = false;
    let mut announced = false;

    loop {
        io.poll();
        if !announced && io.usb_dev.state() == usb_device::device::UsbDeviceState::Configured {
            announced = true;
            io.log(
                "BOOT",
                format_args!(
                    "modbus-device unit={:#04x} uart1 tx=GP4 rx=GP5 {} 8E1",
                    UNIT, BAUD
                ),
            );
        }
        // ── the Modbus bus: count everything, frame by silence ───────────────────
        let mut b = [0u8; 32];
        match uart1.read_raw(&mut b) {
            Ok(n) if n > 0 => {
                st.rx_bytes += n as u32;
                let k = n.min(frame.len() - flen);
                frame[flen..flen + k].copy_from_slice(&b[..k]);
                flen += k;
                st.overflow += (n - k) as u32;
                last_rx_us = timer.get_counter().ticks();
            }
            Ok(_) => {}
            Err(nb::Error::WouldBlock) => {}
            Err(nb::Error::Other(e)) => {
                st.rx_errors += 1;
                st.rx_bytes += e.discarded.len() as u32;
                last_rx_us = timer.get_counter().ticks();
            }
        }
        if flen > 0 && timer.get_counter().ticks().wrapping_sub(last_rx_us) >= FRAME_GAP_US {
            st.frames += 1;
            let resp = handle(&mut store, &frame[..flen], &mut st);
            io.log(
                "MB_DEV_RX",
                format_args!(
                    "n={},len={},hex={},result={}",
                    st.frames,
                    flen,
                    Hx(&frame[..flen]),
                    resp.1
                ),
            );
            if !resp.0.is_empty() {
                uart1.write_full_blocking(&resp.0);
            }
            log_regs(&mut io, &store, "MB_DEV_REGS");
            flen = 0;
        }
        // ── USB commands ─────────────────────────────────────────────────────────
        let mut raw = [0u8; 64];
        let n_raw = match io.serial.read(&mut raw) {
            Ok(n) => n,
            Err(_) => 0,
        };
        for &byte in &raw[..n_raw] {
            if in_line {
                if byte == b'\n' || byte == b'\r' {
                    in_line = false;
                    run_line(&mut io, &mut store, &mut st, &line[..line_len]);
                } else if line_len < line.len() {
                    line[line_len] = byte;
                    line_len += 1;
                }
            } else if byte == b'@' {
                in_line = true;
                line_len = 0;
            } else if byte == b'b' {
                hal::rom_data::reset_to_usb_boot(0, 0);
            }
        }
    }
}

#[derive(Default)]
struct Stats {
    rx_bytes: u32,
    rx_errors: u32,
    overflow: u32,
    frames: u32,
    parse_errors: u32,
    writes: u32,
    local_frames: u32,
}

/// rmodbus on one frame: (response bytes, outcome label).
fn handle(store: &mut Store, req: &[u8], st: &mut Stats) -> (heapless::Vec<u8, 256>, &'static str) {
    let mut buf: ModbusFrameBuf = [0; 256];
    buf[..req.len()].copy_from_slice(req);
    let mut resp: heapless::Vec<u8, 256> = heapless::Vec::new();
    let mut f = ModbusFrame::new(UNIT, &buf, ModbusProto::Rtu, &mut resp);
    if f.parse().is_err() {
        st.parse_errors += 1;
        return (heapless::Vec::new(), "parse_error");
    }
    let mut label = "ignored";
    if f.processing_required {
        let r = if f.readonly {
            f.process_read(store)
        } else {
            f.process_write(store)
        };
        match r {
            Ok(()) if f.error.is_none() => {
                if !f.readonly {
                    st.writes += 1;
                    label = "write_applied";
                } else {
                    label = "read";
                }
            }
            Ok(()) => label = "exception",
            Err(e) => {
                let _ = f.set_modbus_error_if_unset(&e);
                label = "exception";
            }
        }
    }
    if f.response_required && f.finalize_response().is_ok() {
        return (resp, label);
    }
    (heapless::Vec::new(), label)
}

fn log_regs(io: &mut Io, store: &Store, ev: &str) {
    let r = |a: u16| store.get_holding(a).unwrap_or(0xFFFF);
    io.log(
        ev,
        format_args!(
            "r10={},r11={},r12={},r20={}",
            r(0x10),
            r(0x11),
            r(0x12),
            r(0x20)
        ),
    );
}

fn run_line(io: &mut Io, store: &mut Store, st: &mut Stats, line: &[u8]) {
    match line.first() {
        Some(b'D') => {
            io.log(
                "MB_DEV_STATUS",
                format_args!(
                    "rx_bytes={},rx_errors={},overflow={},frames={},parse_errors={},writes={},local_frames={}",
                    st.rx_bytes, st.rx_errors, st.overflow, st.frames, st.parse_errors, st.writes, st.local_frames
                ),
            );
            log_regs(io, store, "MB_DEV_REGS");
        }
        // Control (G8): the same parser, fed from USB instead of the bus. Not counted
        // as bus traffic; the response is logged, not sent.
        Some(b'Z') => {
            let mut req = [0u8; 256];
            match hex_decode(&line[1..], &mut req) {
                Some(n) => {
                    st.local_frames += 1;
                    let writes_before = st.writes;
                    let (resp, label) = handle(store, &req[..n], st);
                    let local_writes = st.writes - writes_before;
                    st.writes = writes_before; // bus writes only
                    io.log(
                        "MB_DEV_LOCAL",
                        format_args!(
                            "len={},hex={},result={},writes={},resp={}",
                            n,
                            Hx(&req[..n]),
                            label,
                            local_writes,
                            Hx(&resp)
                        ),
                    );
                    log_regs(io, store, "MB_DEV_REGS");
                }
                None => io.log("MB_DEV_BAD_HEX", format_args!("chars={}", line.len() - 1)),
            }
        }
        _ => {}
    }
}

fn hex_decode(h: &[u8], out: &mut [u8]) -> Option<usize> {
    if h.len() % 2 != 0 || h.len() / 2 > out.len() {
        return None;
    }
    let v = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    for i in 0..h.len() / 2 {
        out[i] = (v(h[2 * i])? << 4) | v(h[2 * i + 1])?;
    }
    Some(h.len() / 2)
}

struct Hx<'a>(&'a [u8]);
impl core::fmt::Display for Hx<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
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
        let mut l = Line { b: [0; 400], n: 0 };
        let _ = write!(
            l,
            "OASIS|{}|MODBUS|{}|{}|device|{}\r\n",
            BOARD_ID, event, value, GIT_HASH
        );
        let mut data: &[u8] = &l.b[..l.n];
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
    b: [u8; 400],
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
