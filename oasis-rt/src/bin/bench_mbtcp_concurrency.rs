//! `bench_mbtcp_concurrency` — what happens when several HMIs share one gateway (pilot A.4).
//!
//! The gateway serves **one order at a time**: `serve_gateway_conn` takes the state lock
//! and `handle_frame` keeps it across the PLC round trip. That is right for one device and
//! a ceiling for a fleet, and the documentation has said so without ever measuring it.
//! This measures it, the only way the repo accepts a number: **K=10 rounds, median ±
//! half-spread**.
//!
//! For each concurrency N, N threads each open their own HMI socket to the agent and issue
//! W writes. Reported per N:
//!
//! - **throughput**, writes acknowledged per second for the whole round;
//! - **latency**, median over every write of the round, with its spread;
//! - **the slowest thread's median against the fastest's** — the starvation signal, since
//!   an average hides one HMI waiting behind nine;
//! - **refusals**, by Modbus exception code. A bench that counted only acks would call a
//!   refused write "missing" and report a flattering throughput.
//!
//! `--plc-delay-us` gives the PLC stand-in a response time. Loopback answers in tens of
//! microseconds; a real machine has a scan cycle, and the lock is held across that wait,
//! so without a delay the measurement understates the effect by an order of magnitude.
//!
//! ```text
//! cargo run --release --bin bench_mbtcp_concurrency
//! cargo run --release --bin bench_mbtcp_concurrency -- --rounds 10 --writes 10 --plc-delay-us 2000
//! ```
//!
//! ⚠️ One host, loopback, a PLC stand-in: see `common/mbtcp_harness.rs`. The absolute
//! numbers are a floor; what this bench is for is the **shape** against N.

#[path = "common/mbtcp_harness.rs"]
mod harness;

use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use harness::{band, conf, dial, hmi_write_detail, spawn_agent_tagged, spawn_gateway_counting, spawn_plc_delayed, Wr, REG};

/// One thread's outcome for one round.
struct ThreadOut {
    acks: Vec<u64>,
    exceptions: Vec<u8>,
    transport: usize,
}

/// One round at concurrency `n`: `n` threads, `writes` writes each, all at once.
///
/// Returns every ack latency, every exception code, and the wall time of the round. The
/// threads are started before any of them writes — a round where thread 1 finishes before
/// thread 10 starts would measure nothing about concurrency.
fn round(agent: &str, n: usize, writes: usize, base: u16) -> (Vec<ThreadOut>, u64) {
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = std::sync::Arc::new(std::sync::Mutex::new(go_rx));
    let mut handles = Vec::with_capacity(n);
    for t in 0..n {
        let agent = agent.to_string();
        let ready = ready_tx.clone();
        let go = std::sync::Arc::clone(&go_rx);
        handles.push(thread::spawn(move || {
            let mut out = ThreadOut { acks: Vec::new(), exceptions: Vec::new(), transport: 0 };
            let Some(mut sock) = dial(&agent) else {
                out.transport += 1;
                let _ = ready.send(());
                return out;
            };
            // Connected and idle: the socket exists before the start signal, so the round
            // measures writes and not TCP setup.
            let _ = ready.send(());
            {
                let g = go.lock().unwrap();
                let _ = g.recv();
            }
            for i in 0..writes {
                let tid = base.wrapping_add((t * writes + i) as u16);
                let value = 100 + ((t * writes + i) as u16 % 400);
                match hmi_write_detail(&mut sock, tid, REG, value) {
                    Wr::Ack(us) => out.acks.push(us),
                    Wr::Exception(code, _) => out.exceptions.push(code),
                    Wr::Transport => out.transport += 1,
                }
            }
            out
        }));
    }
    for _ in 0..n {
        let _ = ready_rx.recv();
    }
    let t0 = Instant::now();
    for _ in 0..n {
        let _ = go_tx.send(());
    }
    let outs: Vec<ThreadOut> = handles
        .into_iter()
        .map(|h| h.join().unwrap_or(ThreadOut { acks: Vec::new(), exceptions: Vec::new(), transport: 1 }))
        .collect();
    (outs, t0.elapsed().as_micros() as u64)
}

fn arg(name: &str, default: u64) -> u64 {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let rounds = arg("--rounds", 10) as usize;
    let writes = arg("--writes", 10) as usize;
    let delay = arg("--plc-delay-us", 0);

    let plc = spawn_plc_delayed(delay);
    let (gw, gwlog) = spawn_gateway_counting(conf("127.0.0.1:0", &plc, 0xCC));
    let agent = spawn_agent_tagged(conf("127.0.0.1:0", &gw, 0xAA), "conc");

    println!("bench_mbtcp_concurrency  rounds={rounds} writes/HMI={writes} plc_delay={delay} us");
    println!("  plc={plc}  gateway={gw}  agent={agent}");
    println!();
    println!("   N   throughput (ack/s)      latency median        slowest/fastest HMI    refusals");

    let mut summary: Vec<(usize, u64, u64, f64, f64, usize)> = Vec::new();
    for n in [1usize, 2, 5, 10] {
        let mut tput = Vec::with_capacity(rounds);
        let mut lat = Vec::with_capacity(rounds);
        let mut ratio = Vec::with_capacity(rounds);
        let mut exc: Vec<(u8, usize)> = Vec::new();
        let mut transport = 0usize;
        for r in 0..rounds {
            let base = (n as u16) * 1000 + (r as u16) * 100;
            let (outs, wall_us) = round(&agent, n, writes, base);
            let mut all: Vec<u64> = Vec::new();
            let mut per_thread_med: Vec<u64> = Vec::new();
            for o in &outs {
                transport += o.transport;
                for &c in &o.exceptions {
                    match exc.iter_mut().find(|e| e.0 == c) {
                        Some(e) => e.1 += 1,
                        None => exc.push((c, 1)),
                    }
                }
                if !o.acks.is_empty() {
                    let mut v = o.acks.clone();
                    per_thread_med.push(band(&mut v).0);
                    all.extend_from_slice(&o.acks);
                }
            }
            if all.is_empty() {
                continue;
            }
            // Acks per second for the round, from the wall time of the whole round.
            tput.push(all.len() as u64 * 1_000_000 / wall_us.max(1));
            lat.push(band(&mut all).0);
            per_thread_med.sort_unstable();
            let (fast, slow) = (*per_thread_med.first().unwrap(), *per_thread_med.last().unwrap());
            ratio.push(if fast == 0 { 0 } else { 100 * slow / fast });
        }
        if tput.is_empty() {
            println!("  {n:>2}   no acknowledged write — every attempt refused or dropped");
            continue;
        }
        let (tm, ts) = band(&mut tput);
        let (lm, ls) = band(&mut lat);
        let (rm, _) = band(&mut ratio);
        let nexc: usize = exc.iter().map(|e| e.1).sum();
        let elist = if exc.is_empty() {
            "none".to_string()
        } else {
            exc.iter().map(|(c, k)| format!("0x{c:02X}x{k}")).collect::<Vec<_>>().join(" ")
        };
        println!("  {n:>2}   {tm:>8} +/-{ts:>4.0}%        {lm:>8} us +/-{ls:>3.0}%        {:>5.2}x              {elist}", rm as f64 / 100.0);
        summary.push((n, tm, lm, ts, ls, nexc + transport));
    }

    println!();
    if let (Some(first), Some(last)) = (summary.first(), summary.last()) {
        let (n1, t1, l1, ..) = *first;
        let (n2, t2, l2, ..) = *last;
        println!("  N {n1} -> {n2}: throughput {t1} -> {t2} ack/s, latency {l1} -> {l2} us");
        // "It grew from N=1" is not the question — a pipeline of depth 1 is beaten by
        // depth 2 almost whatever the bottleneck. The question is where it stops growing.
        let peak = summary.iter().max_by_key(|s| s.1).unwrap();
        let plateau = summary.iter().find(|s| s.1 * 100 >= peak.1 * 90).unwrap();
        println!("  Peak throughput {} ack/s at N={}; within 10 % of it from N={} on.", peak.1, peak.0, plateau.0);
        if plateau.0 < n2 {
            let per = if l1 == 0 { 0 } else { (l2 / l1) as usize };
            println!("  So the path saturates at N={} and does not improve to N={n2}: the gateway's", plateau.0);
            println!("  state lock spans the PLC round trip and the agent serialises emission, so");
            println!("  concurrent HMIs queue. Latency is then ~{per}x at N={n2} against N={n1} — queueing,");
            println!("  not work. Capacity above this needs **distinct origins**, one enrolled identity");
            println!("  per HMI, which the per-origin map and per-origin sequence (A.1) allow.");
        } else {
            println!("  Throughput still grows at the largest N measured: the lock is not binding here.");
        }
    }
    let refused: usize = summary.iter().map(|s| s.5).sum();
    if refused > 0 {
        println!();
        println!("  {refused} write(s) refused or dropped across all N.");
        // Read the gateway's own record rather than inferring which of the nine
        // conditions fired from a 0x0A: a plausible wrong story is written down exactly
        // here otherwise.
        let g = gwlog.lock().unwrap();
        let mut by: Vec<(String, usize)> = Vec::new();
        for (_, d) in g.iter() {
            if *d == oasis_rt::actuation::Decision::Act {
                continue;
            }
            let k = format!("{d:?}");
            match by.iter_mut().find(|e| e.0 == k) {
                Some(e) => e.1 += 1,
                None => by.push((k, 1)),
            }
        }
        println!("  The gateway's own record of every non-Act decision, {} in all:", by.iter().map(|e| e.1).sum::<usize>());
        for (k, c) in &by {
            println!("    {c:>4}  {k}");
        }
        println!("  Acts: {}", g.iter().filter(|(_, d)| *d == oasis_rt::actuation::Decision::Act).count());
    }
}
