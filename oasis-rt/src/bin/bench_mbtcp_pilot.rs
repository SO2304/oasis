//! `bench_mbtcp_pilot` — what the authority layer costs on a Modbus TCP link (pilot A.4).
//!
//! A pilot customer's first question after "does it refuse what it should" is "what does
//! it add to a write". This measures it the only way the repo accepts a number: **K=10
//! rounds, median ± half-spread**, never a single shot.
//!
//! Three arms, all against the same PLC on the same loopback:
//!
//! 1. **direct** — HMI → PLC, plain Modbus TCP, no OASIS. The baseline.
//! 2. **oasis** — HMI → agent → gateway → PLC, the shipped path
//!    (`mbtcp_pilot::serve_hmi` and `handle_gateway_conn`, the same calls the binaries
//!    make and the socket integration test drives).
//! 3. **crypto+gate only** — sign, verify and decide in memory, no sockets. This is what
//!    says whether the overhead is the authority layer or the plumbing around it.
//!
//! ```text
//! cargo run --release --bin bench_mbtcp_pilot
//! cargo run --release --bin bench_mbtcp_pilot -- --rounds 10 --writes 50
//! ```
//!
//! ⚠️ **The PLC here is a 30-line local responder, not `rmodbus` and not a PLC.** `rmodbus`
//! is a dev-dependency, unavailable to a `[[bin]]`, and the correctness of the frame is
//! already proven against it by `tests/mbtcp_pilot_sockets.rs` and by phase 1.4 on three
//! RP2040. What this bench measures is the **difference between arms through the same
//! peer**; the absolute figures are a floor, because a real PLC adds its own response time
//! and a real network adds a link.
//! ⚠️ Loopback on one host. No radio, no RS-485, no field bus.

#[path = "common/mbtcp_harness.rs"]
mod harness;

use std::thread;
use std::time::{Duration, Instant};

use harness::{band, conf, fp, hmi_write, registry, seed, spawn_agent, spawn_gateway, spawn_null_relay, spawn_plc, NET, REG, UNIT};

use oasis_rt::mbtcp_net::connect_timeout;
use oasis_rt::mesh::{inner_slice, MeshDecision, MeshPubRegistry, MeshRouter};
use oasis_rt::modbus_gateway::{encode_omb1, parse_omb1, Gateway, OrderContext, RegRule};
use oasis_rt::modbus_tcp::{decide_tcp, parse_tcp_write};

/// One round of `writes` HMI writes against `peer`, returning the round's median.
fn round(peer: &str, writes: usize, base_tid: u16) -> Option<u64> {
    let mut sock = connect_timeout(peer, Duration::from_millis(2000)).ok()?;
    sock.set_nodelay(true).ok();
    sock.set_read_timeout(Some(Duration::from_millis(2000))).ok();
    let mut samples = Vec::with_capacity(writes);
    for i in 0..writes {
        let v = 100 + (i as u16 % 400);
        match hmi_write(&mut sock, base_tid.wrapping_add(i as u16), v) {
            Some(us) => samples.push(us),
            None => return None,
        }
    }
    if samples.is_empty() {
        return None;
    }
    Some(band(&mut samples).0)
}

/// Sign, verify and decide, with no socket anywhere: the authority layer's own cost.
fn crypto_and_gate_round(writes: usize) -> u64 {
    let mut origin = MeshRouter::new_v0b(fp(0xAA), NET, seed(0xAA), MeshPubRegistry::new());
    let mut router = MeshRouter::new_v0b(fp(0xCC), NET, seed(0xCC), registry(&[0xAA, 0xCC]));
    let mut gw = Gateway::new();
    let boot = 7u64;
    let w = parse_tcp_write(&[0, 1, 0, 0, 0, 6, UNIT, 0x06, (REG >> 8) as u8, REG as u8, 0, 200]).unwrap();
    let map = [RegRule { addr: REG, min: 0, max: 60000 }];
    let mut samples = Vec::with_capacity(writes);
    for i in 0..writes {
        let t0 = Instant::now();
        let order = w.to_order(1, i as u32 + 1, boot, 10_000);
        let (buf, n) = encode_omb1(&order).unwrap();
        let env = origin.origin_wrap_v0b(&buf[..n]).unwrap();
        let arrived = match router.process(&env) {
            MeshDecision::Arrived { envelope, .. } => envelope,
            MeshDecision::Drop(w) => panic!("own envelope dropped: {w:?}"),
        };
        let parsed = parse_omb1(inner_slice(&arrived)).unwrap();
        let ctx = OrderContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: boot, now_ms: 1_000, r14_safe: true };
        let _ = decide_tcp(&mut gw, &ctx, &parsed, UNIT, &map, i as u16);
        samples.push(t0.elapsed().as_micros() as u64);
    }
    band(&mut samples).0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let num = |flag: &str, default: usize| -> usize { args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default) };
    let rounds = num("--rounds", 10);
    let writes = num("--writes", 50);

    let plc = spawn_plc();
    // The control has to have the SAME SHAPE as the measured path, or the difference
    // between them is partly structure and partly OASIS and the bench cannot say which.
    // HMI -> agent -> gateway -> PLC is three hops, so the control is two chained relays:
    // HMI -> relay1 -> relay2 -> PLC. Comparing three hops against two was charging OASIS
    // for a loopback round trip it did not cause.
    let relay2 = spawn_null_relay(plc.clone());
    let relay = spawn_null_relay(relay2.clone());
    let gw = spawn_gateway(conf("127.0.0.1:0", &plc, 0xCC));
    let agent = spawn_agent(conf("127.0.0.1:0", &gw, 0xAA));
    thread::sleep(Duration::from_millis(150));

    println!("bench_mbtcp_pilot  K={rounds} rounds x {writes} writes");
    println!("  plc={plc}  null-relays={relay} -> {relay2}  gateway={gw}  agent={agent}");
    println!("(each round reports its own median; the band below is over the {rounds} round medians)");

    // Warm-up, discarded: the first exchange pays one-time costs (key material, page
    // faults, the OS learning the loopback path) that no steady-state write pays.
    let _ = round(&plc, 5, 1);
    let _ = round(&relay, 5, 1);
    let _ = round(&agent, 5, 1);

    let mut direct = Vec::with_capacity(rounds);
    let mut null = Vec::with_capacity(rounds);
    let mut oasis = Vec::with_capacity(rounds);
    let mut pure = Vec::with_capacity(rounds);
    for k in 0..rounds {
        let base = 1000 + (k as u16) * 100;
        for (name, peer, into) in [("direct", &plc, &mut direct), ("null relays", &relay, &mut null), ("oasis", &agent, &mut oasis)] {
            match round(peer, writes, base) {
                Some(us) => into.push(us),
                None => {
                    eprintln!("round {k}: the {name} arm did not complete (an order refused?)");
                    std::process::exit(1);
                }
            }
        }
        pure.push(crypto_and_gate_round(writes));
    }

    let (d, ds) = band(&mut direct);
    let (n, ns) = band(&mut null);
    let (o, os) = band(&mut oasis);
    let (c, cs) = band(&mut pure);
    println!();
    println!("  {:<34} {:>9} {:>10}", "arm", "median", "spread");
    println!("  {:<34} {:>7} us {:>9}", "1 hop   HMI -> PLC", d, format!("+/-{ds:.0}%"));
    println!("  {:<34} {:>7} us {:>9}", "3 hops  HMI -> 2 null relays -> PLC", n, format!("+/-{ns:.0}%"));
    println!("  {:<34} {:>7} us {:>9}", "3 hops  HMI -> agent -> gw -> PLC", o, format!("+/-{os:.0}%"));
    println!("  {:<34} {:>7} us {:>9}", "        sign+verify+gate, no sockets", c, format!("+/-{cs:.0}%"));
    println!();
    println!("  vs a direct write:        {:>6} us  ({:.1}x)", o.saturating_sub(d), o as f64 / d.max(1) as f64);
    println!("  of which the two hops:    {:>6} us  (what any two middleboxes cost here)", n.saturating_sub(d));
    let authority = o.saturating_sub(n);
    println!("  of which OASIS:           {:>6} us  ({:.1}x the same shape without it)", authority, o as f64 / n.max(1) as f64);
    println!();
    // The cross-check: OASIS's share of a round trip should be close to what the same
    // work costs with no socket in the way. If these two diverge, the gap is something
    // the bench has not named, and saying so is the point of printing both.
    let gap = authority as i64 - c as i64;
    println!("  cross-check: OASIS's share {authority} us vs sign+verify+gate alone {c} us -> {gap:+} us unaccounted");
    if gap.unsigned_abs() > c / 2 {
        println!("  ** the two disagree by more than half: the difference is NOT just crypto+gate.");
        println!("     Candidates not isolated here: the journal append, the OTM1 refresh once per");
        println!("     {} ms, and the gateway's per-frame state lock.", oasis_rt::mbtcp_pilot::CLOCK_REFRESH_MS);
    } else {
        println!("  the accounting closes: OASIS's added latency is its crypto and its gate, not plumbing.");
    }
    println!();
    println!("  Connections are reused on all three links (agent->gateway, gateway->PLC) and the");
    println!("  clock is refreshed once per {} ms via part K's TimeView rather than per write.", oasis_rt::mbtcp_pilot::CLOCK_REFRESH_MS);
    println!("  Before that, this bench measured 3172 us +/-724%: a connect per write is not just");
    println!("  slower, it makes the latency unpredictable and burns ephemeral ports.");
}
