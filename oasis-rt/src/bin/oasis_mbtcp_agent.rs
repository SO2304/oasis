//! `oasis_mbtcp_agent` — the operator-side agent (pilot phase A.2).
//!
//! The HMI writes plain Modbus TCP to this agent exactly as it would to the PLC, and is
//! **not modified**. For each write the agent parses it, turns it into an `OMB1` order,
//! signs it in a v0B envelope, sends it to the gateway, waits for the signed outcome, and
//! answers the HMI.
//!
//! ```text
//! HMI ──Modbus TCP, clear──▶ agent ──v0B over TCP──▶ gateway ──▶ PLC
//!     ◀──ack or exception──        ◀──OMR1 reply───
//! ```
//!
//! ```text
//! oasis_mbtcp_agent --config agent.conf
//! ```
//!
//! Every byte of the exchange lives in `oasis_rt::mbtcp_pilot`, which is what the socket
//! integration test drives. This file is argument parsing, a listener and a thread per
//! connection — deliberately, because the earlier version of it held its own copy of the
//! exchange and that copy had a defect the library had already fixed: it rebuilt the
//! refusal from an invented reason, so an unlisted register reached the HMI as 0x0A
//! instead of 0x02. The test was green the whole time; it tested the library and the
//! operator would have run this. Two implementations is one too many.
//!
//! Two properties the pilot prompt is explicit about, both now in the tested path:
//!
//! - **The sequence is strictly increasing and persisted** (`mbtcp_pilot::SeqStore`),
//!   written before the order goes out, so a restart cannot reuse a number.
//! - **Never silence towards the HMI** (`mbtcp_pilot::serve_hmi`): a timeout, an
//!   unreachable gateway or a malformed reply all become an exception.
//!
//! The deadline comes from the **gateway's clock**, not ours: the agent asks for its
//! `boot_id` and `now_ms` before signing. That is the honest version of what part K solved
//! on the mesh with a signed `OTM1` beacon; here the link is a TCP connection to the one
//! party whose clock matters.
//!
//! ⚠️ **Reads (FC03/FC04) are refused**, not relayed. The pilot prompt asks for a decision
//! and this is it: the default must leave **one** device talking to the PLC, so the agent
//! does not open a second path for reads. A read returns exception 0x0A rather than
//! silently bypassing the gateway. Relaying reads through the gateway is the next step and
//! it is not done.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use oasis_rt::mbtcp_conf::Config;
use oasis_rt::mbtcp_pilot::{serve_hmi, AgentState, SeqStore};
use oasis_rt::mesh::MeshRouter;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.iter().position(|a| a == "--config").and_then(|i| args.get(i + 1)) {
        Some(p) => p.clone(),
        None => {
            eprintln!("usage: oasis_mbtcp_agent --config <file>");
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
    let seq = match SeqStore::load(&format!("{path}.seq")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sequence: {e}");
            std::process::exit(2);
        }
    };

    let origin = MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone());
    let next_seq = seq.peek();
    let agent = Arc::new(Mutex::new(AgentState { origin, seq }));

    let listener = match TcpListener::bind(&conf.listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {}: {e}", conf.listen);
            std::process::exit(2);
        }
    };
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_default();
    println!("AGENT listen={bound} gateway={} unit=0x{:02x} next_seq={next_seq}", conf.plc, conf.unit);

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let agent = Arc::clone(&agent);
                let conf = conf.clone();
                std::thread::spawn(move || {
                    let peer = s.peer_addr().map(|a| a.to_string()).unwrap_or_default();
                    // The normal end of an HMI connection is an error kind: it closed, or
                    // said nothing before the timeout. Logged, never fatal.
                    match serve_hmi(&mut s, &agent, &conf) {
                        Ok(()) => println!("AGENT hmi={peer} closed"),
                        Err(e) => println!("AGENT hmi={peer} end err={:?}", e.kind()),
                    }
                });
            }
            Err(e) => println!("AGENT accept err={:?}", e.kind()),
        }
    }
}
