//! The gateway and agent logic, in the library so the integration test exercises **the
//! shipped path** and not a copy of it (pilot phase A).
//!
//! `oasis_mbtcp_gateway` and `oasis_mbtcp_agent` are thin wrappers over these functions:
//! they parse a config, bind a socket and call in. A test that reimplemented the exchange
//! would prove something about the test.
//!
//! That last sentence was aspirational when this module was written, and the gap it left
//! was not theoretical. Both binaries carried their own copy of the logic, and the agent's
//! copy still had the defect the library had already fixed: it rebuilt the refusal from an
//! invented `Reason::NotAuthorized` and `RuleCheck::Ok`, which `refusal_code` maps to 0x0A,
//! so an unlisted register reached the HMI as "gateway path unavailable" instead of
//! "illegal data address". The integration test was green throughout, because it tested
//! this module and the operator would have run the other one. Everything down to the HMI
//! loop now lives here, and the binaries hold argument parsing, a listener and a thread.
//!
//! Split into [`agent`] and [`gateway`] when connection reuse landed: R10 caps a file at
//! 400 lines and this had earned the split rather than an exemption.

use crate::actuation::Reason;
use crate::modbus_gateway::RuleCheck;
use crate::modbus_tcp::{refusal_code, Outcome};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod agent;
pub mod gateway;

pub use agent::{serve_hmi, AgentState, GatewayLink, SeqStore, TxCounterStore, CLOCK_REFRESH_MS};
pub use gateway::{handle_gateway_conn, serve_gateway_conn, GatewayState, Served};
/// Re-exported so a consumer of the gateway does not have to name the crate that
/// owns the quorum rule. It became reachable from a `[[bin]]` on 2026-10-09, when the
/// dev-dependency cycle that hid it was removed.
pub use oasis_operator_key::OperatorAuthority;

/// Reply to the agent: `"OMR1" | cmd_seq u32 | kind u8 | code u8`.
pub const OMR1_MAGIC: [u8; 4] = *b"OMR1";
pub const OMR1_LEN: usize = 10;
/// The agent asks the gateway for its clock with these four bytes.
pub const CLOCK_REQ: [u8; 4] = *b"OTQ1";
/// And gets `"OTM1" | boot_id u64 | now_ms u64` back.
pub const CLOCK_REPLY_LEN: usize = 20;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

pub fn encode_reply(cmd_seq: u32, outcome: &Outcome) -> [u8; OMR1_LEN] {
    let mut b = [0u8; OMR1_LEN];
    b[0..4].copy_from_slice(&OMR1_MAGIC);
    b[4..8].copy_from_slice(&cmd_seq.to_le_bytes());
    let (kind, code) = match outcome {
        Outcome::Done => (0u8, 0u8),
        Outcome::Refused(r, rules) => (1, refusal_code(*r, *rules)),
        Outcome::PlcException(c) => (2, *c),
        Outcome::NoAnswer => (3, 0),
    };
    b[8] = kind;
    b[9] = code;
    b
}

pub fn decode_reply(reply: &[u8]) -> Option<(u32, Outcome)> {
    if reply.len() != OMR1_LEN || reply[0..4] != OMR1_MAGIC {
        return None;
    }
    let seq = u32::from_le_bytes(reply[4..8].try_into().unwrap());
    let outcome = match (reply[8], reply[9]) {
        (0, _) => Outcome::Done,
        // The reply carries the Modbus exception the gateway chose. The agent must render
        // **that** code and nothing else: an earlier version threw it away and recomputed
        // one from a reason it invented, so an unlisted register reached the HMI as 0x0A
        // "gateway path unavailable" instead of 0x02 "illegal data address". The
        // integration test caught it. Reconstructing the rule from the code is how the
        // agent reports the gateway's decision without pretending to know its reason.
        (1, code) => Outcome::Refused(
            Reason::OutOfLimits,
            match code {
                crate::modbus_tcp::EXC_ILLEGAL_ADDRESS => RuleCheck::RegisterNotAllowed(0),
                crate::modbus_tcp::EXC_ILLEGAL_VALUE => RuleCheck::ValueOutOfRange(0, 0),
                _ => RuleCheck::Ok,
            },
        ),
        (2, code) => Outcome::PlcException(code),
        _ => Outcome::NoAnswer,
    };
    Some((seq, outcome))
}

#[cfg(test)]
mod tests;
