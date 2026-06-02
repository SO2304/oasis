//! OASIS — UDP MITM loss proxy
//!
//! Forwards UDP datagrams from `--listen` to `--forward`, dropping packets
//! according to a configurable loss model. Used to validate spore v2 over
//! actual network sockets (not in-memory bench).
//!
//! Loss models:
//!   --model uniform --loss 0.30    Each packet dropped IID with p=0.30
//!   --model burst   --loss 0.30 --burst-mean 5
//!       Two-state (Gilbert-Elliott): GOOD state drops nothing, BAD state
//!       drops everything. Mean burst length = burst_mean packets.
//!       Steady-state P(BAD) = `loss`.
//!
//! Usage:
//!   udp_loss_proxy --listen 127.0.0.1:5001 --forward 127.0.0.1:5002 \
//!                  --model burst --loss 0.30 --burst-mean 5

use std::net::{SocketAddr, UdpSocket};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cfg = match Cfg::parse(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            print_usage();
            return ExitCode::from(2);
        }
    };

    let sock = match UdpSocket::bind(&cfg.listen) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bind {} failed: {}", cfg.listen, e);
            return ExitCode::FAILURE;
        }
    };
    eprintln!("[loss_proxy] listen {} → forward {}, model={}, loss={:.0}%, burst_mean={}", cfg.listen, cfg.forward, cfg.model, cfg.loss * 100.0, cfg.burst_mean);

    let mut state = State::new(cfg.loss, cfg.burst_mean);
    let mut buf = [0u8; 65535];
    let mut total = 0u64;
    let mut dropped = 0u64;
    let forward_addr: SocketAddr = cfg.forward.parse().expect("bad --forward addr");

    loop {
        let (len, _src) = match sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("recv error: {}", e);
                continue;
            }
        };
        total += 1;
        let drop = match cfg.model.as_str() {
            "uniform" => state.uniform_drop(),
            "burst" => state.burst_drop(),
            _ => false,
        };
        if drop {
            dropped += 1;
        } else if let Err(e) = sock.send_to(&buf[..len], forward_addr) {
            eprintln!("forward error: {}", e);
        }
        if total % 50 == 0 {
            eprintln!("[loss_proxy] {} pkts, {} dropped ({:.1}%)", total, dropped, dropped as f64 * 100.0 / total as f64);
        }
    }
}

struct State {
    loss: f64,
    burst_mean: f64,
    rng: u32,
    in_bad: bool,
}

impl State {
    fn new(loss: f64, burst_mean: f64) -> Self {
        Self { loss, burst_mean, rng: 0xCAFEBABE, in_bad: false }
    }
    fn next_rand(&mut self) -> f64 {
        // LCG: deterministic for reproducibility
        self.rng = self.rng.wrapping_mul(1103515245).wrapping_add(12345);
        (self.rng as f64) / (u32::MAX as f64)
    }
    fn uniform_drop(&mut self) -> bool {
        self.next_rand() < self.loss
    }
    /// Gilbert-Elliott two-state Markov: GOOD→GOOD, BAD→BAD with
    /// transition probs that yield steady-state P(BAD)=loss
    /// and mean burst length = burst_mean packets.
    fn burst_drop(&mut self) -> bool {
        // P(stay in BAD) = 1 - 1/burst_mean
        // P(stay in GOOD) = 1 - (loss / (burst_mean * (1-loss)))
        let p_stay_bad = 1.0 - 1.0 / self.burst_mean.max(1.0);
        let p_to_bad = if 1.0 - self.loss > 0.0 { self.loss / (self.burst_mean.max(1.0) * (1.0 - self.loss)) } else { 1.0 };
        let r = self.next_rand();
        if self.in_bad {
            if r > p_stay_bad {
                self.in_bad = false;
            }
        } else {
            if r < p_to_bad {
                self.in_bad = true;
            }
        }
        self.in_bad
    }
}

#[derive(Default)]
struct Cfg {
    listen: String,
    forward: String,
    model: String,
    loss: f64,
    burst_mean: f64,
}

impl Cfg {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut cfg = Cfg { listen: "127.0.0.1:5001".into(), forward: "127.0.0.1:5002".into(), model: "uniform".into(), loss: 0.0, burst_mean: 5.0 };
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--listen" => {
                    cfg.listen = args.get(i + 1).ok_or("--listen needs value")?.clone();
                    i += 2;
                }
                "--forward" => {
                    cfg.forward = args.get(i + 1).ok_or("--forward needs value")?.clone();
                    i += 2;
                }
                "--model" => {
                    cfg.model = args.get(i + 1).ok_or("--model needs value")?.clone();
                    i += 2;
                }
                "--loss" => {
                    cfg.loss = args.get(i + 1).ok_or("--loss needs value")?.parse().map_err(|_| "--loss: bad f64")?;
                    i += 2;
                }
                "--burst-mean" => {
                    cfg.burst_mean = args.get(i + 1).ok_or("--burst-mean needs value")?.parse().map_err(|_| "--burst-mean: bad f64")?;
                    i += 2;
                }
                "-h" | "--help" => return Err("help".into()),
                other => return Err(format!("unknown arg: {}", other)),
            }
        }
        Ok(cfg)
    }
}

fn print_usage() {
    eprintln!("Usage: udp_loss_proxy [OPTIONS]");
    eprintln!("  --listen HOST:PORT       UDP socket to receive from   (default 127.0.0.1:5001)");
    eprintln!("  --forward HOST:PORT      UDP socket to forward to     (default 127.0.0.1:5002)");
    eprintln!("  --model uniform|burst    Loss model                   (default uniform)");
    eprintln!("  --loss 0.0..1.0          Steady-state loss rate       (default 0.0)");
    eprintln!("  --burst-mean N           Mean burst length (model=burst) (default 5)");
}
