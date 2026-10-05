//! OASIS — Request/Response Services (replaces rclcpp::Service / rclcpp::Client)
//!
//! Closes the last big ROS 2 API gap: RPC-style services. Examples:
//! `/compute_path`, `/set_goal`, `/get_robot_status`. The client sends a
//! request; the server returns a response. Same transport as topics:
//! envelope flows through mesh for free → multi-hop services work out of the box.
//!
//! # Wire format
//!
//! Request `SPORE\x0A`:
//! ```text
//!   [0..6]    "SPORE\x0A"      magic
//!   [6..14]   request_id       u64 LE (caller-chosen, must be unique per in-flight)
//!   [14..22]  service_hash     u64 LE (SHA-256(name)[..8])
//!   [22..26]  payload_len      u32 LE
//!   [26..]    request_payload
//! ```
//!
//! Response `SPORE\x0B`:
//! ```text
//!   [0..6]    "SPORE\x0B"      magic
//!   [6..14]   request_id       u64 LE (echoes request)
//!   [14..15]  status            u8 (0 = OK, 1 = service_not_found, 2 = handler_error)
//!   [15..19]  payload_len       u32 LE
//!   [19..]    response_payload  (empty when status != 0)
//! ```
//!
//! # Composability
//!
//! Wrap requests in SPORE\x08 (mesh) for multi-hop RPC. Wrap in SPORE\x07 (ECDH)
//! for authenticated RPC. ROS 2 needs separate SecureROS + rmw config layers.

use crate::topics::hash_topic;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap as HashMap,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::HashMap;

pub const SPORE_VA_MAGIC: &[u8] = b"SPORE\x0A"; // request
pub const SPORE_VB_MAGIC: &[u8] = b"SPORE\x0B"; // response
pub const REQ_HEADER_LEN: usize = 6 + 8 + 8 + 4; // 26 bytes
pub const RESP_HEADER_LEN: usize = 6 + 8 + 1 + 4; // 19 bytes

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Ok = 0,
    ServiceNotFound = 1,
    HandlerError = 2,
}

impl ServiceStatus {
    #[inline]
    pub fn from_u8(b: u8) -> ServiceStatus {
        match b {
            0 => Self::Ok,
            1 => Self::ServiceNotFound,
            _ => Self::HandlerError,
        }
    }
}

/// Wrap a request envelope.
pub fn wrap_request(service_name: &str, request_id: u64, payload: &[u8]) -> Vec<u8> {
    let hash = hash_topic(service_name);
    wrap_request_hash(hash, request_id, payload)
}

pub fn wrap_request_hash(service_hash: u64, request_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(REQ_HEADER_LEN + payload.len());
    out.extend_from_slice(SPORE_VA_MAGIC);
    out.extend_from_slice(&request_id.to_le_bytes());
    out.extend_from_slice(&service_hash.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Parse a request envelope. Zero-copy payload slice.
pub fn parse_request(envelope: &[u8]) -> Result<(u64, u64, &[u8]), &'static str> {
    if envelope.len() < REQ_HEADER_LEN {
        return Err("request envelope too short");
    }
    if &envelope[..6] != SPORE_VA_MAGIC {
        return Err("bad request magic");
    }
    let request_id = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let service_hash = u64::from_le_bytes(envelope[14..22].try_into().unwrap());
    let plen = u32::from_le_bytes(envelope[22..26].try_into().unwrap()) as usize;
    if envelope.len() < REQ_HEADER_LEN + plen {
        return Err("request payload truncated");
    }
    Ok((request_id, service_hash, &envelope[REQ_HEADER_LEN..REQ_HEADER_LEN + plen]))
}

/// Wrap a response envelope.
pub fn wrap_response(request_id: u64, status: ServiceStatus, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(RESP_HEADER_LEN + payload.len());
    out.extend_from_slice(SPORE_VB_MAGIC);
    out.extend_from_slice(&request_id.to_le_bytes());
    out.push(status as u8);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Parse a response envelope.
pub fn parse_response(envelope: &[u8]) -> Result<(u64, ServiceStatus, &[u8]), &'static str> {
    if envelope.len() < RESP_HEADER_LEN {
        return Err("response envelope too short");
    }
    if &envelope[..6] != SPORE_VB_MAGIC {
        return Err("bad response magic");
    }
    let request_id = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let status = ServiceStatus::from_u8(envelope[14]);
    let plen = u32::from_le_bytes(envelope[15..19].try_into().unwrap()) as usize;
    if envelope.len() < RESP_HEADER_LEN + plen {
        return Err("response payload truncated");
    }
    Ok((request_id, status, &envelope[RESP_HEADER_LEN..RESP_HEADER_LEN + plen]))
}

/// Server-side router: dispatch incoming requests to registered handlers.
/// Handlers are `fn(&[u8]) -> Vec<u8>` — pure transform of request bytes to
/// response bytes. For error-returning handlers, wrap the Result in the payload.
pub struct ServiceRouter {
    handlers: HashMap<u64, fn(&[u8]) -> Vec<u8>>,
    names: HashMap<u64, String>, // introspection
}

impl Default for ServiceRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceRouter {
    pub fn new() -> Self {
        Self { handlers: HashMap::new(), names: HashMap::new() }
    }

    pub fn register(&mut self, name: &str, handler: fn(&[u8]) -> Vec<u8>) {
        let h = hash_topic(name);
        self.names.insert(h, name.to_string());
        self.handlers.insert(h, handler);
    }

    /// Handle an incoming request envelope. Returns a response envelope.
    /// If the service is not registered, returns a `ServiceNotFound` response.
    pub fn handle(&self, request_env: &[u8]) -> Result<Vec<u8>, &'static str> {
        let (request_id, service_hash, payload) = parse_request(request_env)?;
        match self.handlers.get(&service_hash) {
            Some(handler) => {
                let response = handler(payload);
                Ok(wrap_response(request_id, ServiceStatus::Ok, &response))
            }
            None => Ok(wrap_response(request_id, ServiceStatus::ServiceNotFound, &[])),
        }
    }

    pub fn list_services(&self) -> Vec<&str> {
        self.names.values().map(|s| s.as_str()).collect()
    }

    pub fn service_count(&self) -> usize {
        self.handlers.len()
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: request envelope roundtrip preserves (id, hash, payload).
    #[kani::proof]
    fn proof_services_request_roundtrip() {
        let req_id: u64 = kani::any();
        let hash: u64 = kani::any();
        let payload: [u8; 4] = kani::any();
        let env = wrap_request_hash(hash, req_id, &payload);
        let (rid, h, p) = parse_request(&env).unwrap();
        assert_eq!(rid, req_id);
        assert_eq!(h, hash);
        assert_eq!(p, &payload[..]);
    }

    /// PROVE: response envelope roundtrip preserves (id, status, payload).
    #[kani::proof]
    fn proof_services_response_roundtrip() {
        let req_id: u64 = kani::any();
        let status_byte: u8 = kani::any();
        let status = ServiceStatus::from_u8(status_byte);
        let payload: [u8; 4] = kani::any();
        let env = wrap_response(req_id, status, &payload);
        let (rid, s, p) = parse_response(&env).unwrap();
        assert_eq!(rid, req_id);
        assert_eq!(s as u8, status as u8);
        assert_eq!(p, &payload[..]);
    }

    /// PROVE: parse_request on any too-short input returns Err, never panics.
    #[kani::proof]
    fn proof_services_request_parse_rejects_short() {
        let len: usize = kani::any();
        kani::assume(len < REQ_HEADER_LEN);
        let buf = vec![0u8; len];
        assert!(parse_request(&buf).is_err());
    }

    /// PROVE: parse_response on any too-short input returns Err, never panics.
    #[kani::proof]
    fn proof_services_response_parse_rejects_short() {
        let len: usize = kani::any();
        kani::assume(len < RESP_HEADER_LEN);
        let buf = vec![0u8; len];
        assert!(parse_response(&buf).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_envelope_roundtrip() {
        let env = wrap_request("/compute_path", 42, b"start=0,0 goal=5,5");
        let (id, hash, payload) = parse_request(&env).unwrap();
        assert_eq!(id, 42);
        assert_eq!(hash, hash_topic("/compute_path"));
        assert_eq!(payload, b"start=0,0 goal=5,5");
    }

    #[test]
    fn response_envelope_roundtrip() {
        let env = wrap_response(99, ServiceStatus::Ok, b"path=[(0,0),(5,5)]");
        let (id, status, payload) = parse_response(&env).unwrap();
        assert_eq!(id, 99);
        assert_eq!(status, ServiceStatus::Ok);
        assert_eq!(payload, b"path=[(0,0),(5,5)]");
    }

    #[test]
    fn parse_request_rejects_bad_magic() {
        let mut env = wrap_request("/x", 1, b"y");
        env[0] = b'Z';
        assert!(parse_request(&env).is_err());
    }

    #[test]
    fn parse_response_rejects_bad_magic() {
        let mut env = wrap_response(1, ServiceStatus::Ok, b"y");
        env[0] = b'Z';
        assert!(parse_response(&env).is_err());
    }

    #[test]
    fn parse_truncated_rejected() {
        let mut env = wrap_request("/x", 1, b"hello world");
        env.truncate(env.len() - 5);
        assert!(parse_request(&env).is_err());
    }

    fn echo_handler(input: &[u8]) -> Vec<u8> {
        input.to_vec()
    }
    fn add_42_handler(_input: &[u8]) -> Vec<u8> {
        vec![42]
    }

    #[test]
    fn server_echo_service() {
        let mut router = ServiceRouter::new();
        router.register("/echo", echo_handler);
        let req = wrap_request("/echo", 1, b"hello");
        let resp_env = router.handle(&req).unwrap();
        let (id, status, payload) = parse_response(&resp_env).unwrap();
        assert_eq!(id, 1);
        assert_eq!(status, ServiceStatus::Ok);
        assert_eq!(payload, b"hello");
    }

    #[test]
    fn server_multiple_services() {
        let mut router = ServiceRouter::new();
        router.register("/echo", echo_handler);
        router.register("/answer", add_42_handler);
        assert_eq!(router.service_count(), 2);
        let r1 = router.handle(&wrap_request("/echo", 1, b"ping")).unwrap();
        let (_, _, p1) = parse_response(&r1).unwrap();
        assert_eq!(p1, b"ping");
        let r2 = router.handle(&wrap_request("/answer", 2, b"?")).unwrap();
        let (_, _, p2) = parse_response(&r2).unwrap();
        assert_eq!(p2, &[42]);
    }

    #[test]
    fn unknown_service_returns_not_found() {
        let router = ServiceRouter::new();
        let req = wrap_request("/missing", 99, b"");
        let resp_env = router.handle(&req).unwrap();
        let (id, status, _) = parse_response(&resp_env).unwrap();
        assert_eq!(id, 99);
        assert_eq!(status, ServiceStatus::ServiceNotFound);
    }

    #[test]
    fn service_introspection() {
        let mut router = ServiceRouter::new();
        router.register("/a", echo_handler);
        router.register("/b", echo_handler);
        let svcs = router.list_services();
        assert_eq!(svcs.len(), 2);
        assert!(svcs.contains(&"/a"));
        assert!(svcs.contains(&"/b"));
    }

    #[test]
    fn different_request_ids_preserved() {
        // Ensures concurrent in-flight requests don't get confused — each
        // response carries its original request_id for correlation.
        let mut router = ServiceRouter::new();
        router.register("/echo", echo_handler);
        let r1 = router.handle(&wrap_request("/echo", 111, b"a")).unwrap();
        let r2 = router.handle(&wrap_request("/echo", 222, b"b")).unwrap();
        let r3 = router.handle(&wrap_request("/echo", 333, b"c")).unwrap();
        assert_eq!(parse_response(&r1).unwrap().0, 111);
        assert_eq!(parse_response(&r2).unwrap().0, 222);
        assert_eq!(parse_response(&r3).unwrap().0, 333);
    }

    #[test]
    fn status_byte_roundtrip() {
        for status in [ServiceStatus::Ok, ServiceStatus::ServiceNotFound, ServiceStatus::HandlerError] {
            let env = wrap_response(1, status, b"");
            let (_, parsed, _) = parse_response(&env).unwrap();
            assert_eq!(parsed as u8, status as u8);
        }
    }

    #[test]
    fn bench_service_dispatch_latency() {
        let mut router = ServiceRouter::new();
        router.register("/fast", echo_handler);
        let req = wrap_request("/fast", 1, b"payload");
        let start = std::time::Instant::now();
        const N: u32 = 50_000;
        for _ in 0..N {
            let _ = router.handle(&req).unwrap();
        }
        let per_us = start.elapsed().as_nanos() as f64 / N as f64 / 1000.0;
        assert!(per_us < 5.0, "service dispatch too slow: {} µs/op", per_us);
        eprintln!("service dispatch latency: {:.2} µs/op (echo handler)", per_us);
    }
}
