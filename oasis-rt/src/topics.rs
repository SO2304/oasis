//! OASIS — Named Topic Pub/Sub (replacement for rclcpp::Publisher/Subscriber)
//!
//! Closes the biggest ROS 2 ergonomic gap: named topics like "/cmd_vel" with
//! typed pub/sub. Under the hood, topic names hash to 8-byte identifiers;
//! envelopes flow through the spore mesh (multi-hop, forward-secret, sender-
//! authenticated) for free.
//!
//! # Comparison to ROS 2
//! ```text
//!   rclcpp::create_publisher<Twist>("/cmd_vel", qos)      →  7+ indirections
//!   rclcpp::create_subscription<Twist>(..., callback)     →  DDS QoS tuning
//!
//!   topics::publish("/cmd_vel", &payload)                 →  1 function call
//!   topics::subscribe("/cmd_vel", my_handler)             →  direct hashmap dispatch
//! ```
//!
//! # Wire format SPORE\x09 (topic envelope)
//! ```text
//!   [0..6]   "SPORE\x09"   magic
//!   [6..14]  topic_hash    u64 LE (SHA-256(name)[..8])
//!   [14..18] payload_len   u32 LE
//!   [18..]   payload       opaque bytes
//! ```
//!
//! # Composability
//! Wrap a topic envelope inside a SPORE\x08 mesh envelope → multi-hop delivery
//! to all subscribers anywhere in the mesh. Wrap inside SPORE\x07 → crypto.
//! No ROS 2 equivalent ships this compositionally.

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap as HashMap,
    string::{String, ToString},
    vec::Vec,
};
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::collections::HashMap;

pub const SPORE_V9_MAGIC: &[u8] = b"SPORE\x09";
pub const TOPIC_HEADER_LEN: usize = 6 + 8 + 4; // magic + hash + len
pub const TOPIC_HASH_LEN: usize = 8;

/// Deterministic 64-bit hash of a topic name.
/// First 8 bytes of SHA-256 — collision probability ≈ 2^-64 per pair.
/// Pure function, Kani-verifiable.
#[inline]
pub fn hash_topic(name: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(name.as_bytes());
    let out: [u8; 32] = h.finalize().into();
    u64::from_le_bytes(out[..TOPIC_HASH_LEN].try_into().unwrap())
}

/// Build a topic envelope ready for transport.
#[inline]
pub fn wrap_topic(name: &str, payload: &[u8]) -> Vec<u8> {
    let hash = hash_topic(name);
    wrap_topic_hash(hash, payload)
}

/// Same but with pre-computed hash (skip SHA-256 on hot paths).
pub fn wrap_topic_hash(hash: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(TOPIC_HEADER_LEN + payload.len());
    write_topic_envelope_into(&mut out, hash, payload);
    out
}

/// Write a topic envelope into a pre-allocated buffer. The buffer should
/// have at least `TOPIC_HEADER_LEN + payload.len()` bytes of remaining
/// capacity to avoid reallocation. Used by zero-copy combined builders
/// like `mesh::MeshRouter::origin_wrap_with` to avoid the intermediate
/// `Vec` allocation that `wrap_topic` would produce.
///
/// Pre-2026-04-22 audit found this allocation was the dominant cost at
/// large payloads (1 MB → 7.8× slower than rclcpp intra-process).
#[inline]
pub fn write_topic_envelope_into(buf: &mut Vec<u8>, hash: u64, payload: &[u8]) {
    buf.extend_from_slice(SPORE_V9_MAGIC);
    buf.extend_from_slice(&hash.to_le_bytes());
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
}

/// Parse a topic envelope. Zero-copy payload slice.
pub fn parse_topic(envelope: &[u8]) -> Result<(u64, &[u8]), &'static str> {
    if envelope.len() < TOPIC_HEADER_LEN {
        return Err("topic envelope too short");
    }
    if &envelope[..6] != SPORE_V9_MAGIC {
        return Err("bad topic magic");
    }
    let hash = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let plen = u32::from_le_bytes(envelope[14..18].try_into().unwrap()) as usize;
    if envelope.len() < TOPIC_HEADER_LEN + plen {
        return Err("topic payload truncated");
    }
    Ok((hash, &envelope[TOPIC_HEADER_LEN..TOPIC_HEADER_LEN + plen]))
}

/// Local in-process topic router. Like rclcpp's executor but without the 8-layer
/// stack; direct HashMap dispatch. Thread-safe: wrap in Arc<Mutex<_>> if shared.
///
/// Handler signature: `fn(hash, payload_bytes)`. Kept trait-object-free to
/// allow zero-alloc dispatch on the hot path. Caller types the payload.
pub struct TopicRouter {
    subscribers: HashMap<u64, Vec<fn(u64, &[u8])>>,
    names: HashMap<u64, String>, // debug introspection only
}

impl Default for TopicRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl TopicRouter {
    pub fn new() -> Self {
        Self { subscribers: HashMap::new(), names: HashMap::new() }
    }

    /// Subscribe a fn-pointer handler. Multiple handlers per topic allowed.
    pub fn subscribe(&mut self, name: &str, handler: fn(u64, &[u8])) {
        let h = hash_topic(name);
        self.names.entry(h).or_insert_with(|| name.to_string());
        self.subscribers.entry(h).or_default().push(handler);
    }

    /// Dispatch an envelope to all subscribers of its topic. Returns # dispatched.
    pub fn dispatch(&self, envelope: &[u8]) -> Result<u32, &'static str> {
        let (hash, payload) = parse_topic(envelope)?;
        let count = match self.subscribers.get(&hash) {
            Some(handlers) => {
                for h in handlers {
                    h(hash, payload);
                }
                handlers.len() as u32
            }
            None => 0,
        };
        Ok(count)
    }

    /// List known topic names (for introspection, like `ros2 topic list`).
    pub fn list_topics(&self) -> Vec<&str> {
        self.names.values().map(|s| s.as_str()).collect()
    }

    pub fn subscriber_count(&self, name: &str) -> usize {
        let h = hash_topic(name);
        self.subscribers.get(&h).map(|v| v.len()).unwrap_or(0)
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: hash_topic is DETERMINISTIC — same name → same hash (pure function).
    /// Kani on a single-byte name to keep solver tractable.
    #[kani::proof]
    fn proof_topic_hash_deterministic_1byte() {
        let b: u8 = kani::any();
        let s = std::str::from_utf8(std::slice::from_ref(&b)).unwrap_or("x");
        let h1 = hash_topic(s);
        let h2 = hash_topic(s);
        assert_eq!(h1, h2);
    }

    /// PROVE: wrap_topic + parse_topic preserves (hash, payload) bit-identical.
    #[kani::proof]
    fn proof_topic_envelope_roundtrip() {
        let hash: u64 = kani::any();
        let payload: [u8; 4] = kani::any();
        let env = wrap_topic_hash(hash, &payload);
        let (h, p) = parse_topic(&env).unwrap();
        assert_eq!(h, hash);
        assert_eq!(p, &payload[..]);
    }

    /// PROVE: parse_topic on any too-short input returns Err, never panics.
    #[kani::proof]
    fn proof_topic_parse_rejects_short_inputs() {
        let len: usize = kani::any();
        kani::assume(len < TOPIC_HEADER_LEN);
        let buf = vec![0u8; len];
        let r = parse_topic(&buf);
        assert!(r.is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn hash_topic_deterministic() {
        assert_eq!(hash_topic("/cmd_vel"), hash_topic("/cmd_vel"));
        assert_ne!(hash_topic("/cmd_vel"), hash_topic("/odom"));
    }

    #[test]
    fn envelope_roundtrip() {
        let env = wrap_topic("/sensors/imu", b"roll,pitch,yaw");
        let (hash, payload) = parse_topic(&env).unwrap();
        assert_eq!(hash, hash_topic("/sensors/imu"));
        assert_eq!(payload, b"roll,pitch,yaw");
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut env = wrap_topic("/x", b"y");
        env[0] = b'Z';
        assert!(parse_topic(&env).is_err());
    }

    #[test]
    fn parse_rejects_truncated() {
        let mut env = wrap_topic("/x", b"hello world");
        env.truncate(env.len() - 5);
        assert!(parse_topic(&env).is_err());
    }

    // Global state for handler-based tests (fn ptr can't capture).
    // Serialized to avoid mutex-poisoning contamination across tests.
    use serial_test::serial;
    static CALL_COUNT: Mutex<u32> = Mutex::new(0);
    fn counter_handler(_h: u64, _p: &[u8]) {
        if let Ok(mut g) = CALL_COUNT.lock() {
            *g += 1;
        } else {
            // Recover from poisoning transparently for test purposes
            CALL_COUNT.clear_poison();
            *CALL_COUNT.lock().unwrap() += 1;
        }
    }

    #[test]
    #[serial]
    fn subscribe_and_dispatch() {
        CALL_COUNT.clear_poison();
        *CALL_COUNT.lock().unwrap() = 0;
        let mut router = TopicRouter::new();
        router.subscribe("/cmd_vel", counter_handler);
        router.subscribe("/cmd_vel", counter_handler); // second handler
        let env = wrap_topic("/cmd_vel", b"0,0,0");
        let n = router.dispatch(&env).unwrap();
        assert_eq!(n, 2);
        assert_eq!(*CALL_COUNT.lock().unwrap(), 2);
    }

    #[test]
    fn unknown_topic_dispatches_zero() {
        let router = TopicRouter::new();
        let env = wrap_topic("/unknown", b"");
        assert_eq!(router.dispatch(&env).unwrap(), 0);
    }

    #[test]
    fn introspection_list_topics() {
        let mut router = TopicRouter::new();
        router.subscribe("/a", counter_handler);
        router.subscribe("/b", counter_handler);
        let topics = router.list_topics();
        assert!(topics.contains(&"/a"));
        assert!(topics.contains(&"/b"));
    }

    #[test]
    fn topic_router_is_thread_safe_wrapped() {
        let router = Arc::new(Mutex::new(TopicRouter::new()));
        router.lock().unwrap().subscribe("/ping", counter_handler);
        let r2 = router.clone();
        let t = std::thread::spawn(move || {
            r2.lock().unwrap().subscribe("/pong", counter_handler);
        });
        t.join().unwrap();
        assert_eq!(router.lock().unwrap().list_topics().len(), 2);
    }

    #[test]
    fn invariant_hashes_collision_resistant_across_10k_names() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..10_000u32 {
            let name = format!("/topic_{:05}", i);
            let h = hash_topic(&name);
            assert!(seen.insert(h), "collision at {}", name);
        }
    }

    #[test]
    #[serial]
    fn bench_dispatch_latency() {
        CALL_COUNT.clear_poison();
        let mut router = TopicRouter::new();
        router.subscribe("/fast", counter_handler);
        let env = wrap_topic("/fast", b"x");
        let start = std::time::Instant::now();
        const N: u32 = 100_000;
        for _ in 0..N {
            router.dispatch(&env).unwrap();
        }
        let per_us = start.elapsed().as_nanos() as f64 / N as f64 / 1000.0;
        assert!(per_us < 2.0, "dispatch too slow: {} µs/op", per_us);
        eprintln!("topic dispatch latency: {:.2} µs/op", per_us);
    }
}
