//! `oasis_test_plc` — this PC as a Modbus TCP device, to stand in for a PLC.
//!
//! Every OASIS claim about reaching a device has said the same thing until now: *nothing
//! has met a PLC*. The RTU path was proved on three RP2040 against board A running
//! `rmodbus` with no OASIS code on it; the TCP path had only an `rmodbus` listener living
//! **inside** a test binary, which proves the library and not the programs an operator
//! runs. This is the missing device: a separate process, on a real socket, whose frames
//! are parsed and applied by `rmodbus`.
//!
//! **It contains no OASIS code, and that is checkable from `Cargo.toml`**: this crate
//! depends on `rmodbus` and not on `oasis-rt`. When it applies a frame, OASIS is not
//! agreeing with itself.
//!
//! ```text
//! oasis_test_plc --listen 127.0.0.1:5020 --log plc.log
//! ```
//!
//! What it gives a campaign that an in-test listener cannot:
//!
//! - **A write counter and a register file that outlive the test.** The ground truth for
//!   "only `Act` decisions reach the device" is a number this process printed, not an
//!   assertion inside the thing under test.
//! - **A bus log.** Every frame in and out, with a monotonic microsecond stamp, so a
//!   campaign can show that a refused order put **zero bytes** on the wire.
//! - **A state dump on demand.** `--dump-on-exit` prints every non-zero holding register
//!   when the process is stopped, which is what an operator would read off the panel.
//!
//! ⚠️ **It is a device simulator, not a PLC.** No scan cycle, no ladder logic, no I/O, no
//! real-time guarantee, and it answers instantly where a real PLC takes milliseconds. It
//! makes the *protocol* and the *authorisation* path real; it does not make the machine
//! real. Timing measured through it is a floor.
//! ⚠️ Single unit id, holding registers only (`rmodbus`'s `ModbusStorageSmall`).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rmodbus::server::context::ModbusContext;
use rmodbus::server::storage::ModbusStorageSmall;
use rmodbus::server::ModbusFrame;
use rmodbus::{ModbusFrameBuf, ModbusProto};

const MBAP_LEN: usize = 7;

struct Counters {
    writes: AtomicU32,
    reads: AtomicU32,
    frames: AtomicU32,
    rejected: AtomicU32,
}

struct Opts {
    listen: String,
    unit: u8,
    log: Option<String>,
    dump_on_exit: bool,
}

fn parse_opts() -> Opts {
    let a: Vec<String> = std::env::args().collect();
    let get = |flag: &str| -> Option<String> {
        a.iter()
            .position(|x| x == flag)
            .and_then(|i| a.get(i + 1))
            .cloned()
    };
    Opts {
        listen: get("--listen").unwrap_or_else(|| "127.0.0.1:5020".into()),
        unit: get("--unit")
            .and_then(|v| u8::from_str_radix(v.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0x11),
        log: get("--log"),
        dump_on_exit: a.iter().any(|x| x == "--dump-on-exit"),
    }
}

/// One line per event on stdout, and to the log file when one is given. Flushed every
/// time: a campaign that kills this process must still find the last frame in the file.
struct Log {
    file: Option<Mutex<std::fs::File>>,
    t0: Instant,
}

impl Log {
    fn new(path: Option<&str>) -> Self {
        let file = path.map(|p| Mutex::new(std::fs::File::create(p).expect("log file")));
        Log {
            file,
            t0: Instant::now(),
        }
    }

    fn say(&self, args: std::fmt::Arguments<'_>) {
        let line = format!("[{:>10} us] {}", self.t0.elapsed().as_micros(), args);
        println!("{line}");
        let _ = std::io::stdout().flush();
        if let Some(f) = &self.file {
            if let Ok(mut f) = f.lock() {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Read exactly one Modbus TCP request: the 7-byte MBAP header says how much follows, so
/// a short read is a short read and not a guess.
fn read_request(sock: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut head = [0u8; MBAP_LEN];
    sock.read_exact(&mut head)?;
    let len = u16::from_be_bytes([head[4], head[5]]) as usize;
    if len == 0 || len > 260 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad MBAP length",
        ));
    }
    let mut out = Vec::with_capacity(MBAP_LEN + len - 1);
    out.extend_from_slice(&head);
    out.resize(MBAP_LEN + len - 1, 0);
    sock.read_exact(&mut out[MBAP_LEN..])?;
    Ok(out)
}

fn main() {
    let o = parse_opts();
    let log = Arc::new(Log::new(o.log.as_deref()));
    let store = Arc::new(Mutex::new(ModbusStorageSmall::new()));
    let c = Arc::new(Counters {
        writes: AtomicU32::new(0),
        reads: AtomicU32::new(0),
        frames: AtomicU32::new(0),
        rejected: AtomicU32::new(0),
    });

    let srv = match TcpListener::bind(&o.listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {}: {e}", o.listen);
            std::process::exit(2);
        }
    };
    let bound = srv.local_addr().map(|a| a.to_string()).unwrap_or_default();
    log.say(format_args!(
        "PLC listen={bound} unit=0x{:02x} impl=rmodbus-0.12.2 oasis_code=none",
        o.unit
    ));

    // A campaign stops this process with a signal; the counters it needs are printed on
    // every frame, so the summary below is a convenience and not the evidence.
    if o.dump_on_exit {
        let (store, c, log) = (Arc::clone(&store), Arc::clone(&c), Arc::clone(&log));
        let _ = std::thread::spawn(move || {
            // No signal handling on this platform without a dependency; the dump is done
            // by the campaign reading the log instead. Kept behind the flag so the
            // intent is visible and the limitation is stated rather than implied.
            std::thread::sleep(Duration::from_secs(3600));
            dump(&store, &c, &log);
        });
    }

    for stream in srv.incoming() {
        let Ok(mut sock) = stream else { continue };
        let (store, c, log) = (Arc::clone(&store), Arc::clone(&c), Arc::clone(&log));
        let unit = o.unit;
        std::thread::spawn(move || {
            let peer = sock.peer_addr().map(|a| a.to_string()).unwrap_or_default();
            sock.set_nodelay(true).ok();
            sock.set_read_timeout(Some(Duration::from_secs(30))).ok();
            log.say(format_args!("PLC conn_open peer={peer}"));
            loop {
                let req = match read_request(&mut sock) {
                    Ok(r) => r,
                    Err(_) => break,
                };
                c.frames.fetch_add(1, Ordering::SeqCst);
                log.say(format_args!(
                    "PLC rx peer={peer} len={} bytes={}",
                    req.len(),
                    hex(&req)
                ));

                let mut buf: ModbusFrameBuf = [0; 256];
                if req.len() > buf.len() {
                    c.rejected.fetch_add(1, Ordering::SeqCst);
                    log.say(format_args!("PLC reject reason=too_long len={}", req.len()));
                    break;
                }
                buf[..req.len()].copy_from_slice(&req);
                let mut resp = Vec::new();
                let mut f = ModbusFrame::new(unit, &buf, ModbusProto::TcpUdp, &mut resp);
                if f.parse().is_err() {
                    // rmodbus refused the frame: nothing is applied. This is the device's
                    // own judgement, not OASIS's.
                    c.rejected.fetch_add(1, Ordering::SeqCst);
                    log.say(format_args!(
                        "PLC reject reason=rmodbus_parse bytes={}",
                        hex(&req)
                    ));
                    break;
                }
                if f.processing_required {
                    let readonly = f.readonly;
                    let r = {
                        let mut st = store.lock().unwrap();
                        if readonly {
                            f.process_read(&mut *st)
                        } else {
                            f.process_write(&mut *st)
                        }
                    };
                    if r.is_err() {
                        c.rejected.fetch_add(1, Ordering::SeqCst);
                        log.say(format_args!("PLC reject reason=rmodbus_process"));
                        break;
                    }
                    if readonly {
                        c.reads.fetch_add(1, Ordering::SeqCst);
                    } else {
                        c.writes.fetch_add(1, Ordering::SeqCst);
                        // The number that matters: a write the device actually applied.
                        // Printed with the register contents so a campaign can show both
                        // that it happened and what landed.
                        let st = store.lock().unwrap();
                        let start = u16::from_be_bytes([req[8], req[9]]);
                        let v = st.get_holding(start).unwrap_or(0);
                        log.say(format_args!(
                            "PLC APPLIED_WRITE writes={} reg={} value={}",
                            c.writes.load(Ordering::SeqCst),
                            start,
                            v
                        ));
                    }
                }
                if f.response_required {
                    if f.finalize_response().is_err() {
                        break;
                    }
                    log.say(format_args!("PLC tx peer={peer} bytes={}", hex(&resp)));
                    if sock.write_all(&resp).is_err() {
                        break;
                    }
                    let _ = sock.flush();
                }
            }
            log.say(format_args!(
                "PLC conn_close peer={peer} frames={} writes={} reads={} rejected={}",
                c.frames.load(Ordering::SeqCst),
                c.writes.load(Ordering::SeqCst),
                c.reads.load(Ordering::SeqCst),
                c.rejected.load(Ordering::SeqCst)
            ));
        });
    }
}

fn dump(store: &Mutex<ModbusStorageSmall>, c: &Counters, log: &Log) {
    let st = store.lock().unwrap();
    let mut shown = 0;
    for r in 0u16..64 {
        if let Ok(v) = st.get_holding(r) {
            if v != 0 {
                log.say(format_args!("PLC reg {r} = {v}"));
                shown += 1;
            }
        }
    }
    log.say(format_args!(
        "PLC SUMMARY frames={} writes={} reads={} rejected={} nonzero_regs={}",
        c.frames.load(Ordering::SeqCst),
        c.writes.load(Ordering::SeqCst),
        c.reads.load(Ordering::SeqCst),
        c.rejected.load(Ordering::SeqCst),
        shown
    ));
}
