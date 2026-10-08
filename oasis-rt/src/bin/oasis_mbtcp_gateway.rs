//! `oasis_mbtcp_gateway` — the machine-side gateway (pilot phase A.1).
//!
//! Listens for v0B envelopes from the agent, verifies them, runs the **unchanged** Part F
//! gate, and writes to the PLC only on `Act`. Then it reads the PLC's answer with a
//! timeout, checks it, and returns the outcome to the agent in a length-prefixed reply.
//!
//! ```text
//! agent ──v0B over TCP (len-prefixed)──▶ gateway ──Modbus TCP──▶ PLC
//!       ◀──OMR1 reply─────────────────           ◀────────────
//! ```
//!
//! ```text
//! oasis_mbtcp_gateway --config gateway.conf
//! ```
//!
//! An audit found that `oasis-rt` contained **no socket at all**: the Modbus TCP layer was
//! pure, so it could decide and build frames but never reach a PLC. This binary is that
//! missing half, and it adds nothing to the decision: `decide_tcp` wraps the Part F gate
//! unchanged, and the frame is still built only in the `Act` branch.
//!
//! The exchange itself is `oasis_rt::mbtcp_pilot::handle_gateway_conn`, which is what the
//! socket integration test drives. This file used to hold a second copy of it; a copy
//! drifts, and the agent's copy had already drifted into a real defect, so there is now
//! one implementation and the test covers the shipped path.
//!
//! Every decision is appended to the **decision journal**, refusals included, which is
//! what Annex III 1.1.9 asks for and what only the RP2040 firmware did until now.
//!
//! ⚠️ **No PLC and no silicon.** It talks to whatever answers Modbus TCP at the configured
//! address; the integration test points it at an `rmodbus` server on localhost. The RTU
//! path is the one proven on three RP2040 against an independent device.
//! ⚠️ Keys come from the config file. There is **no compiled seed**, which was phase 1.2's
//! lesson, but the file itself is as readable as any file on the host.
//! ⚠️ **Orders are served one at a time.** The state lock is held across the PLC round
//! trip, so a hung PLC blocks other connections until the timeout. That is deliberate for
//! one device — the `cmd_seq` check and the journal chain are sequential by nature, and
//! two interleaved writes on one bus is not something to be clever about — but it is a
//! throughput ceiling, not an accident, and a fleet of devices would need one gateway per
//! device or a per-unit lock.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use oasis_rt::mbtcp_conf::Config;
use oasis_rt::mbtcp_pilot::{handle_gateway_conn, GatewayState, Served};
use oasis_rt::mesh::MeshRouter;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.iter().position(|a| a == "--config").and_then(|i| args.get(i + 1)) {
        Some(p) => p.clone(),
        None => {
            eprintln!("usage: oasis_mbtcp_gateway --config <file>");
            std::process::exit(2);
        }
    };
    let conf = match Config::load(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
            std::process::exit(2);
        }
    };

    // A fresh boot id per process start is what makes an order from a previous run
    // refusable; reusing one would reopen the replay window the gate exists to close.
    let boot_id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    let router = MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone());
    let state = Arc::new(Mutex::new(GatewayState::new(router, boot_id)));

    let listener = match TcpListener::bind(&conf.listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {}: {e}", conf.listen);
            std::process::exit(2);
        }
    };
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_default();
    println!("GATEWAY listen={bound} plc={} unit=0x{:02x} boot_id={boot_id} registers={}", conf.plc, conf.unit, conf.map.len());

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let state = Arc::clone(&state);
                let conf = conf.clone();
                std::thread::spawn(move || {
                    let peer = s.peer_addr().map(|a| a.to_string()).unwrap_or_default();
                    let served = {
                        let mut st = state.lock().unwrap();
                        handle_gateway_conn(&mut s, &mut st, &conf)
                    };
                    match served {
                        Ok(Served::Clock { boot_id }) => println!("GATEWAY {peer} CLOCK boot_id={boot_id}"),
                        Ok(Served::MeshDrop(why)) => println!("GATEWAY {peer} MESH_DROP why={why:?} -> no PLC write"),
                        Ok(Served::NotAnOrder) => println!("GATEWAY {peer} NOT_AN_ORDER -> no PLC write"),
                        Ok(Served::Decided { cmd_seq, decision, outcome, plc_written }) => {
                            println!("GATEWAY {peer} seq={cmd_seq} {decision:?} outcome={outcome:?} plc_written={plc_written}")
                        }
                        // The normal end of a short exchange, and also what a third party
                        // poking the port produces. Logged, never fatal: one bad peer must
                        // not stop the gateway.
                        Err(e) => println!("GATEWAY {peer} conn_end err={:?}", e.kind()),
                    }
                });
            }
            Err(e) => println!("GATEWAY accept err={:?}", e.kind()),
        }
    }
}
