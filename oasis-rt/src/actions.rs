//! OASIS — Action Framework (replaces rclcpp Actions: long-running RPC
//! with progress feedback + cancellation).
//!
//! Pattern: client sends a GOAL, server streams FEEDBACK events, then emits
//! a RESULT (or an error). Client can send a CANCEL at any time to abort.
//!
//! Use cases: "goto waypoint X" (50% of the way, canceled halfway), "start
//! SLAM mapping" (progress = % coverage), "execute nav mission" (multi-leg).
//!
//! # Wire formats
//!
//! Goal `SPORE\x0C`:
//! ```text
//!   [0..6]    "SPORE\x0C"
//!   [6..14]   action_id        u64 LE — client-chosen, unique per in-flight
//!   [14..22]  action_hash      u64 LE — SHA-256(name)[..8]
//!   [22..26]  payload_len      u32 LE
//!   [26..]    goal_payload
//! ```
//!
//! Feedback `SPORE\x0D`:
//! ```text
//!   [0..6]    "SPORE\x0D"
//!   [6..14]   action_id        u64 LE
//!   [14..18]  payload_len      u32 LE
//!   [18..]    feedback_payload (e.g. "progress=0.5")
//! ```
//!
//! Result `SPORE\x0E`:
//! ```text
//!   [0..6]    "SPORE\x0E"
//!   [6..14]   action_id        u64 LE
//!   [14..15]  status           u8 (0=Succeeded, 1=Aborted, 2=Canceled)
//!   [15..19]  payload_len      u32 LE
//!   [19..]    result_payload
//! ```
//!
//! Cancel `SPORE\x0F`:
//! ```text
//!   [0..6]    "SPORE\x0F"
//!   [6..14]   action_id        u64 LE
//! ```
//!
//! # Composability
//!
//! Each envelope type is independent — clients/servers can chose which to wrap
//! in mesh (SPORE\x08) or ECDH (SPORE\x07). Typical: goal+cancel authenticated,
//! feedback+result may be best-effort mesh broadcast to all subscribers.

use crate::topics::hash_topic;
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

pub const SPORE_VC_MAGIC: &[u8] = b"SPORE\x0C"; // goal
pub const SPORE_VD_MAGIC: &[u8] = b"SPORE\x0D"; // feedback
pub const SPORE_VE_MAGIC: &[u8] = b"SPORE\x0E"; // result
pub const SPORE_VF_MAGIC: &[u8] = b"SPORE\x0F"; // cancel

pub const GOAL_HEADER_LEN: usize = 6 + 8 + 8 + 4; // 26
pub const FEEDBACK_HEADER_LEN: usize = 6 + 8 + 4; // 18
pub const RESULT_HEADER_LEN: usize = 6 + 8 + 1 + 4; // 19
pub const CANCEL_LEN: usize = 6 + 8; // 14

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    Succeeded = 0,
    Aborted = 1,
    Canceled = 2,
}

impl ActionStatus {
    #[inline]
    pub fn from_u8(b: u8) -> ActionStatus {
        match b {
            0 => Self::Succeeded,
            2 => Self::Canceled,
            _ => Self::Aborted,
        }
    }
}

// ── GOAL ────────────────────────────────────────────────────────

pub fn wrap_goal(action_name: &str, action_id: u64, payload: &[u8]) -> Vec<u8> {
    wrap_goal_hash(hash_topic(action_name), action_id, payload)
}

pub fn wrap_goal_hash(action_hash: u64, action_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(GOAL_HEADER_LEN + payload.len());
    out.extend_from_slice(SPORE_VC_MAGIC);
    out.extend_from_slice(&action_id.to_le_bytes());
    out.extend_from_slice(&action_hash.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn parse_goal(envelope: &[u8]) -> Result<(u64, u64, &[u8]), &'static str> {
    if envelope.len() < GOAL_HEADER_LEN {
        return Err("goal envelope too short");
    }
    if &envelope[..6] != SPORE_VC_MAGIC {
        return Err("bad goal magic");
    }
    let aid = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let ah = u64::from_le_bytes(envelope[14..22].try_into().unwrap());
    let plen = u32::from_le_bytes(envelope[22..26].try_into().unwrap()) as usize;
    if envelope.len() < GOAL_HEADER_LEN + plen {
        return Err("goal payload truncated");
    }
    Ok((aid, ah, &envelope[GOAL_HEADER_LEN..GOAL_HEADER_LEN + plen]))
}

// ── FEEDBACK ─────────────────────────────────────────────────────

pub fn wrap_feedback(action_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FEEDBACK_HEADER_LEN + payload.len());
    out.extend_from_slice(SPORE_VD_MAGIC);
    out.extend_from_slice(&action_id.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn parse_feedback(envelope: &[u8]) -> Result<(u64, &[u8]), &'static str> {
    if envelope.len() < FEEDBACK_HEADER_LEN {
        return Err("feedback envelope too short");
    }
    if &envelope[..6] != SPORE_VD_MAGIC {
        return Err("bad feedback magic");
    }
    let aid = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let plen = u32::from_le_bytes(envelope[14..18].try_into().unwrap()) as usize;
    if envelope.len() < FEEDBACK_HEADER_LEN + plen {
        return Err("feedback payload truncated");
    }
    Ok((aid, &envelope[FEEDBACK_HEADER_LEN..FEEDBACK_HEADER_LEN + plen]))
}

// ── RESULT ───────────────────────────────────────────────────────

pub fn wrap_result(action_id: u64, status: ActionStatus, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(RESULT_HEADER_LEN + payload.len());
    out.extend_from_slice(SPORE_VE_MAGIC);
    out.extend_from_slice(&action_id.to_le_bytes());
    out.push(status as u8);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn parse_result(envelope: &[u8]) -> Result<(u64, ActionStatus, &[u8]), &'static str> {
    if envelope.len() < RESULT_HEADER_LEN {
        return Err("result envelope too short");
    }
    if &envelope[..6] != SPORE_VE_MAGIC {
        return Err("bad result magic");
    }
    let aid = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let status = ActionStatus::from_u8(envelope[14]);
    let plen = u32::from_le_bytes(envelope[15..19].try_into().unwrap()) as usize;
    if envelope.len() < RESULT_HEADER_LEN + plen {
        return Err("result payload truncated");
    }
    Ok((aid, status, &envelope[RESULT_HEADER_LEN..RESULT_HEADER_LEN + plen]))
}

// ── CANCEL ───────────────────────────────────────────────────────

pub fn wrap_cancel(action_id: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(CANCEL_LEN);
    out.extend_from_slice(SPORE_VF_MAGIC);
    out.extend_from_slice(&action_id.to_le_bytes());
    out
}

pub fn parse_cancel(envelope: &[u8]) -> Result<u64, &'static str> {
    if envelope.len() < CANCEL_LEN {
        return Err("cancel envelope too short");
    }
    if &envelope[..6] != SPORE_VF_MAGIC {
        return Err("bad cancel magic");
    }
    Ok(u64::from_le_bytes(envelope[6..14].try_into().unwrap()))
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: goal envelope roundtrip.
    #[kani::proof]
    fn proof_actions_goal_roundtrip() {
        let aid: u64 = kani::any();
        let ah: u64 = kani::any();
        let payload: [u8; 4] = kani::any();
        let env = wrap_goal_hash(ah, aid, &payload);
        let (pid, ph, pp) = parse_goal(&env).unwrap();
        assert_eq!(pid, aid);
        assert_eq!(ph, ah);
        assert_eq!(pp, &payload[..]);
    }

    /// PROVE: feedback envelope roundtrip.
    #[kani::proof]
    fn proof_actions_feedback_roundtrip() {
        let aid: u64 = kani::any();
        let payload: [u8; 4] = kani::any();
        let env = wrap_feedback(aid, &payload);
        let (pid, pp) = parse_feedback(&env).unwrap();
        assert_eq!(pid, aid);
        assert_eq!(pp, &payload[..]);
    }

    /// PROVE: result envelope roundtrip preserves status byte.
    #[kani::proof]
    fn proof_actions_result_roundtrip() {
        let aid: u64 = kani::any();
        let status_byte: u8 = kani::any();
        let status = ActionStatus::from_u8(status_byte);
        let payload: [u8; 4] = kani::any();
        let env = wrap_result(aid, status, &payload);
        let (pid, s, pp) = parse_result(&env).unwrap();
        assert_eq!(pid, aid);
        assert_eq!(s as u8, status as u8);
        assert_eq!(pp, &payload[..]);
    }

    /// PROVE: cancel envelope roundtrip.
    #[kani::proof]
    fn proof_actions_cancel_roundtrip() {
        let aid: u64 = kani::any();
        let env = wrap_cancel(aid);
        let parsed_aid = parse_cancel(&env).unwrap();
        assert_eq!(parsed_aid, aid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_roundtrip() {
        let env = wrap_goal("/navigate_to_pose", 42, b"x=5.0 y=3.0");
        let (aid, hash, payload) = parse_goal(&env).unwrap();
        assert_eq!(aid, 42);
        assert_eq!(hash, hash_topic("/navigate_to_pose"));
        assert_eq!(payload, b"x=5.0 y=3.0");
    }

    #[test]
    fn feedback_roundtrip() {
        let env = wrap_feedback(42, b"progress=0.5");
        let (aid, payload) = parse_feedback(&env).unwrap();
        assert_eq!(aid, 42);
        assert_eq!(payload, b"progress=0.5");
    }

    #[test]
    fn result_succeeded_roundtrip() {
        let env = wrap_result(42, ActionStatus::Succeeded, b"x=5.0 y=3.0");
        let (aid, status, payload) = parse_result(&env).unwrap();
        assert_eq!(aid, 42);
        assert_eq!(status, ActionStatus::Succeeded);
        assert_eq!(payload, b"x=5.0 y=3.0");
    }

    #[test]
    fn result_canceled_roundtrip() {
        let env = wrap_result(42, ActionStatus::Canceled, b"");
        let (_, status, _) = parse_result(&env).unwrap();
        assert_eq!(status, ActionStatus::Canceled);
    }

    #[test]
    fn result_aborted_roundtrip() {
        let env = wrap_result(42, ActionStatus::Aborted, b"unreachable waypoint");
        let (_, status, p) = parse_result(&env).unwrap();
        assert_eq!(status, ActionStatus::Aborted);
        assert_eq!(p, b"unreachable waypoint");
    }

    #[test]
    fn cancel_roundtrip() {
        let env = wrap_cancel(42);
        let aid = parse_cancel(&env).unwrap();
        assert_eq!(aid, 42);
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut env = wrap_goal("/x", 1, b"");
        env[0] = b'Z';
        assert!(parse_goal(&env).is_err());

        let mut env = wrap_feedback(1, b"");
        env[0] = b'Z';
        assert!(parse_feedback(&env).is_err());

        let mut env = wrap_result(1, ActionStatus::Succeeded, b"");
        env[0] = b'Z';
        assert!(parse_result(&env).is_err());

        let mut env = wrap_cancel(1);
        env[0] = b'Z';
        assert!(parse_cancel(&env).is_err());
    }

    #[test]
    fn parse_rejects_truncated() {
        let env = wrap_goal("/x", 1, b"hello world");
        assert!(parse_goal(&env[..env.len() - 3]).is_err());
    }

    #[test]
    fn goal_feedback_result_sequence_example() {
        // Simulate a full action lifecycle
        let goal = wrap_goal("/navigate", 100, b"target=(5,5)");
        let (gid, _, _) = parse_goal(&goal).unwrap();
        assert_eq!(gid, 100);

        // Server streams 3 feedbacks
        for pct in [0.25_f64, 0.50, 0.75] {
            let fb = wrap_feedback(100, format!("progress={}", pct).as_bytes());
            let (fid, _) = parse_feedback(&fb).unwrap();
            assert_eq!(fid, 100);
        }

        // Final result
        let res = wrap_result(100, ActionStatus::Succeeded, b"arrived at (5,5)");
        let (rid, status, _) = parse_result(&res).unwrap();
        assert_eq!(rid, 100);
        assert_eq!(status, ActionStatus::Succeeded);
    }

    #[test]
    fn cancel_mid_action_sequence() {
        // Client cancels at 50% progress
        let _goal = wrap_goal("/navigate", 200, b"target=(99,99)");
        let _fb = wrap_feedback(200, b"progress=0.5");
        let cancel = wrap_cancel(200);
        let cid = parse_cancel(&cancel).unwrap();
        assert_eq!(cid, 200);
        // Server responds with Canceled status
        let res = wrap_result(200, ActionStatus::Canceled, b"aborted at (50,50)");
        let (_, status, _) = parse_result(&res).unwrap();
        assert_eq!(status, ActionStatus::Canceled);
    }

    #[test]
    fn bench_action_envelope_construction() {
        let start = std::time::Instant::now();
        const N: u32 = 100_000;
        for i in 0..N {
            let _ = wrap_goal("/action_a", i as u64, b"payload bytes");
        }
        let per_us = start.elapsed().as_nanos() as f64 / N as f64 / 1000.0;
        assert!(per_us < 5.0, "wrap_goal too slow: {} µs/op", per_us);
        eprintln!("action goal wrap: {:.2} µs/op", per_us);
    }
}
