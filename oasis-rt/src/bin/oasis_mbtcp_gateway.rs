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
//! oasis_mbtcp_gateway --config gateway.conf --journal /var/oasis/jrn
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
use oasis_rt::mbtcp_pilot::{serve_gateway_conn, GatewayState, Served};
use oasis_rt::mesh::MeshRouter;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.iter().position(|a| a == "--config").and_then(|i| args.get(i + 1)) {
        Some(p) => p.clone(),
        None => {
            eprintln!("usage: oasis_mbtcp_gateway --config <file> [--journal <prefix>]");
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
    let mut router = MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone());
    // Revocation now has an effect on this path: the gate reads `router.is_revoked`, which
    // was hard-coded false until 2026-10-09. The list comes from the config, because this
    // gateway does not handle a signed ORV1 over its link yet — that is the real gap.
    // Printed at startup because this fails CLOSED: a `peer` line with no permission
    // field grants nothing, and an operator must see that here rather than deduce it from
    // a refusal later.
    for e in &conf.perms.entries {
        let names = [(oasis_rt::enrollment::perm::ACTUATE, "ACTUATE"), (oasis_rt::enrollment::perm::STOP, "STOP"), (oasis_rt::enrollment::perm::SUPERVISE, "SUPERVISE")]
            .iter()
            .filter(|(b, _)| e.permissions & b == *b)
            .map(|(_, n)| *n)
            .collect::<Vec<_>>()
            .join("|");
        println!("GATEWAY peer fp={} permissions={}", e.fp.iter().map(|b| format!("{b:02x}")).collect::<String>(), if names.is_empty() { "none".to_string() } else { names });
    }
    for fp in &conf.revoked {
        router.revoke(*fp);
        println!("GATEWAY revoked fp={}", fp.iter().map(|b| format!("{b:02x}")).collect::<String>());
    }

    // Where the journal is written. Without it the chain lives in RAM and dies with the
    // process, which is what this binary did until 2026-10-09 while its own header said
    // it appended every decision. `--journal` is how an operator gets evidence that
    // outlives the run; `oasis_journal_verify` reads
    // `cat <prefix>.head <prefix>.entries` directly.
    let jprefix = args.iter().position(|a| a == "--journal").and_then(|i| args.get(i + 1));
    let state = match jprefix {
        Some(p) => match GatewayState::with_journal(router, boot_id, p) {
            Ok(mut s) => {
                println!("GATEWAY journal={p}.entries + {p}.head");
                // A signed ORV1 arriving over the link is applied only if the configured
                // authority accepts its signatures, and the list is written here BEFORE
                // being applied so a restart does not forget a revocation.
                s.authority = conf.authority.clone();
                s.rev_path = Some(format!("{p}.rev").into());
                // Printed so an operator reads the quorum off the startup log rather than
                // inferring it from a refusal — the same reason the permission table is
                // printed below.
                match &conf.authority {
                    Some(oasis_rt::mbtcp_pilot::OperatorAuthority::Single { pub_key }) => {
                        println!("GATEWAY authority=single key={} rev_store={p}.rev", pub_key[..8].iter().map(|b| format!("{b:02x}")).collect::<String>())
                    }
                    Some(oasis_rt::mbtcp_pilot::OperatorAuthority::Multisig { pub_keys, k }) => println!(
                        "GATEWAY authority=quorum k={k} of n={} keys=[{}] rev_store={p}.rev",
                        pub_keys.len(),
                        pub_keys.iter().map(|q| q[..4].iter().map(|b| format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>().join(",")
                    ),
                    Some(other) => println!("GATEWAY authority={other:?} rev_store={p}.rev"),
                    None => println!("GATEWAY no authority configured: every ORV1 over the link is refused"),
                }
                Arc::new(Mutex::new(s))
            }
            Err(e) => {
                eprintln!("journal {p}: {e}");
                std::process::exit(2);
            }
        },
        None => {
            eprintln!("GATEWAY warning: no --journal <prefix>, decisions are kept in RAM only and lost on exit");
            Arc::new(Mutex::new(GatewayState::new(router, boot_id)))
        }
    };

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
                    let log = |served: Served| match served {
                        Served::Clock { boot_id } => println!("GATEWAY {peer} CLOCK boot_id={boot_id}"),
                        Served::MeshDrop(why) => println!("GATEWAY {peer} MESH_DROP why={why:?} -> no PLC write"),
                        Served::NotAnOrder => println!("GATEWAY {peer} NOT_AN_ORDER -> no PLC write"),
                        Served::Read { check, served } => println!("GATEWAY {peer} READ {check:?} served={served} (never writes)"),
                        // A configuration change, journalled as one (1.1.9 §5): who may
                        // command is configuration, so applying or refusing a list is
                        // itself evidence.
                        Served::Revocation { decision, epoch } => println!("GATEWAY {peer} REVOCATION {decision:?} epoch={epoch}"),
                        Served::Decided { cmd_seq, decision, outcome, plc_written } => {
                            println!("GATEWAY {peer} seq={cmd_seq} {decision:?} outcome={outcome:?} plc_written={plc_written}")
                        }
                    };
                    // The normal end of a connection is an error kind: the agent closed,
                    // or said nothing before the timeout. Also what a third party poking
                    // the port produces. Logged, never fatal: one bad peer must not stop
                    // the gateway.
                    if let Err(e) = serve_gateway_conn(&mut s, &state, &conf, log) {
                        println!("GATEWAY {peer} conn_end err={:?}", e.kind());
                    }
                });
            }
            Err(e) => println!("GATEWAY accept err={:?}", e.kind()),
        }
    }
}
