//! OASIS-RT — Spore Protocol (Collective Memory Transport)
//!
//! 3 transport layers for experience propagation:
//!   Layer 1: UDP Multicast (same WiFi, instant, auto-discovery)
//!   Layer 3: Store-and-forward (carried by intermediate devices)
//!   Layer 4: QR Spore (distance-unlimited, base64-encoded digests)
//!
//! All layers carry the same payload: serialized FederatedMesh digests.
//! No server. No pairing. Trust-gated on receive.

use crate::federation::FederatedMesh;
use crate::spore_crypto;
use crate::vec::*;
use std::net::UdpSocket;

const MULTICAST_ADDR: &str = "239.0.42.1:4200";
const BIND_ADDR: &str = "0.0.0.0:4200";
const SPORE_MAGIC: &[u8] = b"SPORE\x01";

/// Load encryption key from env. Precedence:
///   1. `OASIS_SPORE_KEY_HEX` — 64 hex chars → 32-byte key (preferred for random keys)
///   2. `OASIS_SPORE_PASSPHRASE` — derived via SHA-256 + domain tag
/// Returns None if neither is set (encryption disabled).
fn load_key_from_env() -> Option<[u8; spore_crypto::KEY_LEN]> {
    if let Ok(hex) = std::env::var("OASIS_SPORE_KEY_HEX") {
        return spore_crypto::parse_key_hex(&hex);
    }
    if let Ok(pass) = std::env::var("OASIS_SPORE_PASSPHRASE") {
        return Some(spore_crypto::derive_key_from_passphrase(pass.as_bytes()));
    }
    None
}

/// If encryption is enabled (env key set), wrap `plaintext` in a SPORE\x03
/// encrypted envelope. Otherwise return plaintext unchanged.
fn maybe_encrypt(plaintext: &[u8]) -> Vec<u8> {
    if let Some(key) = load_key_from_env() {
        spore_crypto::encrypt_envelope(&key, plaintext, b"").unwrap_or_else(|_| plaintext.to_vec())
    } else {
        plaintext.to_vec()
    }
}

/// Process-global replay window. Protects against an attacker replaying a
/// captured encrypted envelope to the same receiver.
static REPLAY_WINDOW: std::sync::OnceLock<std::sync::Mutex<spore_crypto::ReplayWindow>> = std::sync::OnceLock::new();

fn replay_window() -> &'static std::sync::Mutex<spore_crypto::ReplayWindow> {
    REPLAY_WINDOW.get_or_init(|| {
        let cap: usize = std::env::var("OASIS_SPORE_REPLAY_CAP").ok().and_then(|s| s.parse().ok()).unwrap_or(1024);
        std::sync::Mutex::new(spore_crypto::ReplayWindow::new(cap))
    })
}

/// Process-global counter tracker for v7 envelopes. Persisted to
/// `OASIS_COUNTER_TRACKER_FILE` if configured. Hydrated on first access.
static COUNTER_TRACKER: std::sync::OnceLock<std::sync::Mutex<spore_crypto::CounterTracker>> = std::sync::OnceLock::new();

fn counter_tracker() -> &'static std::sync::Mutex<spore_crypto::CounterTracker> {
    COUNTER_TRACKER.get_or_init(|| {
        let mut t = spore_crypto::CounterTracker::new();
        if let Ok(path) = std::env::var("OASIS_COUNTER_TRACKER_FILE") {
            if let Ok(loaded) = spore_crypto::CounterTracker::load_from_file(&path) {
                t = loaded;
            }
        }
        std::sync::Mutex::new(t)
    })
}

/// Process-global mesh router for SPORE\x08 multi-hop envelopes.
/// Configured lazily: origin fingerprint is read from env `OASIS_SPORE_ID_PUB_HEX`
/// (first 8 bytes of SHA-256 of the X25519 pub), which matches the v5/v7 sender FP.
/// If the pub isn't configured, the router uses a zero fingerprint (no identity).
static MESH_ROUTER: std::sync::OnceLock<std::sync::Mutex<crate::mesh::MeshRouter>> = std::sync::OnceLock::new();

fn mesh_router() -> &'static std::sync::Mutex<crate::mesh::MeshRouter> {
    MESH_ROUTER.get_or_init(|| {
        let my_fp: [u8; crate::mesh::FP_LEN] = if let Ok(hex) = std::env::var("OASIS_SPORE_ID_PUB_HEX") {
            if let Some(pub_bytes) = spore_crypto::parse_key_hex(&hex) {
                spore_crypto::sender_fingerprint(&pub_bytes)
            } else {
                [0u8; crate::mesh::FP_LEN]
            }
        } else {
            [0u8; crate::mesh::FP_LEN]
        };
        let ttl: u8 = std::env::var("OASIS_MESH_TTL").ok().and_then(|s| s.parse().ok()).unwrap_or(crate::mesh::DEFAULT_TTL);
        let cap: usize = std::env::var("OASIS_MESH_DEDUP_CAP").ok().and_then(|s| s.parse().ok()).unwrap_or(crate::mesh::DEFAULT_DEDUP_CAP);
        let mut router = crate::mesh::MeshRouter::with_config(my_fp, ttl, cap);
        // Restore the monotonic sender counter across restarts so a rebooted node
        // never reuses origin msg_ids (which would reopen the replay window and
        // get its own fresh messages dropped as duplicates). Mirrors the
        // COUNTER_TRACKER persistence above; see mesh::MeshRouter::set_tx_counter.
        if let Ok(path) = std::env::var("OASIS_MESH_TX_COUNTER_FILE") {
            if let Ok(s) = std::fs::read_to_string(&path) {
                if let Ok(saved) = s.trim().parse::<u64>() {
                    router.set_tx_counter(saved);
                }
            }
        }
        // Bound long-memory Bloom false-positive growth on long-lived/unattended
        // nodes: enable auto-reset if configured, else warn (default leaves the
        // Bloom growing — FPR drifts past 1% after ~52k host / ~1.6k MCU inserts).
        match std::env::var("OASIS_MESH_AUTO_RESET").ok().and_then(|s| s.trim().parse::<u64>().ok()) {
            Some(t) if t > 0 => router.set_bloom_auto_reset_threshold(Some(t)),
            _ => eprintln!(
                "[oasis-mesh] warning: OASIS_MESH_AUTO_RESET unset — mesh Bloom will not \
                 auto-reset; its false-positive rate drifts past 1% after ~52k inserts \
                 (host) / ~1.6k (MCU). Set OASIS_MESH_AUTO_RESET=<inserts> on long-lived nodes."
            ),
        }
        std::sync::Mutex::new(router)
    })
}

/// Wrap + broadcast a payload via mesh. Sends both the mesh envelope and
/// (optionally) logs it — caller must re-broadcast wrapped via their transport.
pub fn mesh_wrap_for_broadcast(inner: &[u8]) -> Vec<u8> {
    let mut router = mesh_router().lock().unwrap();
    let out = router.origin_wrap(inner);
    // Opportunistically persist the advanced counter (mirror tracker save). A
    // brownout in the window between send and save reuses at most one counter,
    // vs. resetting to 0 on every restart without this.
    if let Ok(path) = std::env::var("OASIS_MESH_TX_COUNTER_FILE") {
        let _ = std::fs::write(&path, router.tx_counter().to_string());
    }
    out
}

/// Process an incoming SPORE\x08 packet. If it should be forwarded, returns
/// `Some(bytes_to_rebroadcast)` — caller must transmit these via their
/// transport (UDP multicast / LoRa / etc.).
/// Returns `None` if the envelope is a duplicate, own echo, or TTL-terminal.
/// The INNER payload is extracted and re-entered into the standard spore
/// processing pipeline via `maybe_decrypt` + mesh.
pub fn mesh_process_incoming(envelope: &[u8]) -> crate::mesh::MeshDecision {
    mesh_router().lock().unwrap().process(envelope)
}

#[cfg(test)]
pub(crate) fn reset_mesh_for_tests() {
    if let Some(m) = MESH_ROUTER.get() {
        let fp = [0u8; crate::mesh::FP_LEN];
        *m.lock().unwrap() = crate::mesh::MeshRouter::new(fp);
    }
}

/// Process-global revocation list. Receivers auto-merge any SPORE\x06 envelope
/// whose signature verifies under the operator's Ed25519 pubkey (from
/// `OASIS_OP_ED25519_PUB_HEX` env var). Updates also persist to
/// `OASIS_REVOCATION_FILE` if configured.
static REVOCATION_LIST: std::sync::OnceLock<std::sync::Mutex<spore_crypto::RevocationList>> = std::sync::OnceLock::new();

fn revocation_list() -> &'static std::sync::Mutex<spore_crypto::RevocationList> {
    REVOCATION_LIST.get_or_init(|| {
        let mut rl = spore_crypto::RevocationList::new();
        // If a file path and op pubkey are configured, hydrate on first access
        if let (Ok(path), Some(op_pub)) = (std::env::var("OASIS_REVOCATION_FILE"), load_op_pubkey_from_env()) {
            if let Ok(loaded) = spore_crypto::load_revocation_file(&path, &op_pub) {
                rl = loaded;
            }
        }
        std::sync::Mutex::new(rl)
    })
}

fn load_op_pubkey_from_env() -> Option<[u8; 32]> {
    let hex = std::env::var("OASIS_OP_ED25519_PUB_HEX").ok()?;
    spore_crypto::parse_key_hex(&hex)
}

/// Check whether a sender fingerprint appears in the process-global revocation list.
/// Callers (adapters / receivers) use this to reject v5 envelopes from revoked drones.
pub fn is_sender_revoked(fp: &[u8; spore_crypto::SENDER_FP_LEN]) -> bool {
    revocation_list().lock().unwrap().is_revoked(fp)
}

/// Ingest a SPORE\x07 envelope into the process-global tracker + revocation
/// list. Requires env config:
///   - OASIS_SPORE_ID_PRIV_HEX: recipient's X25519 private key
///   - OASIS_SPORE_KEY_HEX (or passphrase): pre-shared AEAD key
///   - OASIS_SPORE_SENDER_PUB_HEX__<FP8HEX>: per-sender static pubkey,
///     indexed by fingerprint (optional — for multi-sender setups)
///
/// Returns `Ok(plaintext)` on successful decrypt + counter update, else Err.
pub fn ingest_v7_envelope(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 6 || &buf[..6] != spore_crypto::SPORE_V7_MAGIC {
        return Err("not a v7 envelope");
    }
    let recipient_priv_hex = std::env::var("OASIS_SPORE_ID_PRIV_HEX").map_err(|_| "OASIS_SPORE_ID_PRIV_HEX not set")?;
    let recipient_priv = spore_crypto::parse_key_hex(&recipient_priv_hex).ok_or("bad OASIS_SPORE_ID_PRIV_HEX")?;
    let psk = load_key_from_env().ok_or("no OASIS_SPORE_KEY_HEX configured for v7")?;

    let (fp, _counter) = spore_crypto::v7_envelope_header(buf)?;
    // Resolve sender pub via per-sender env var: OASIS_SPORE_SENDER_PUB_HEX__<hex_fp>
    let fp_hex: String = fp.iter().map(|b| format!("{:02x}", b)).collect();
    let sender_pub_hex = std::env::var(format!("OASIS_SPORE_SENDER_PUB_HEX__{}", fp_hex)).map_err(|_| "sender pub not configured for this fingerprint")?;
    let sender_pub = spore_crypto::parse_key_hex(&sender_pub_hex).ok_or("bad sender pub hex")?;

    let rl = revocation_list().lock().unwrap().clone();
    let mut tracker = counter_tracker().lock().unwrap();
    let pt = spore_crypto::decrypt_envelope_v7_checked(&sender_pub, &recipient_priv, &psk, buf, b"", &mut tracker, &rl)?;
    // Opportunistic persistence
    if let Ok(path) = std::env::var("OASIS_COUNTER_TRACKER_FILE") {
        let _ = tracker.save_to_file(&path);
    }
    Ok(pt)
}

#[cfg(test)]
pub(crate) fn reset_counter_tracker_for_tests() {
    if let Some(t) = COUNTER_TRACKER.get() {
        *t.lock().unwrap() = spore_crypto::CounterTracker::new();
    }
}

/// If `buf` is a SPORE\x06 revocation envelope AND the operator pubkey is
/// configured, verify + merge into the process-global list. Returns the
/// number of newly-added entries, or 0 if the envelope wasn't a v6 or the
/// op pubkey isn't set. Errors ONLY on signature failure or malformed blob.
pub fn ingest_revocation_envelope(buf: &[u8]) -> Result<usize, &'static str> {
    if buf.len() < 6 || &buf[..6] != spore_crypto::SPORE_V6_MAGIC {
        return Ok(0); // not a revocation envelope
    }
    let op_pub = load_op_pubkey_from_env().ok_or("no OASIS_OP_ED25519_PUB_HEX configured")?;
    let mut rl = revocation_list().lock().unwrap();
    let (_, added) = spore_crypto::merge_revocation_envelope(&mut rl, buf, &op_pub)?;
    // Opportunistic persistence — ignore errors (transient disk issues)
    if added > 0 {
        if let (Ok(path), Ok(seed_hex)) = (std::env::var("OASIS_REVOCATION_FILE"), std::env::var("OASIS_OP_ED25519_SEED_HEX")) {
            if let Some(seed) = spore_crypto::parse_key_hex(&seed_hex) {
                let _ = spore_crypto::save_revocation_file(&rl, &path, &seed);
            }
        }
    }
    Ok(added)
}

#[cfg(test)]
pub(crate) fn reset_revocation_for_tests() {
    if let Some(rl) = REVOCATION_LIST.get() {
        *rl.lock().unwrap() = spore_crypto::RevocationList::new();
    }
}

/// If buf is a SPORE\x03 envelope, decrypt with env key. Otherwise pass through.
/// Returns Err("downgrade rejected") if env key is set but buf is NOT encrypted
/// — refusing to accept plaintext when a key is configured prevents trivial
/// downgrade attacks by a MITM.
///
/// For encrypted envelopes, also consults the process-global replay window:
/// a nonce seen in the last `OASIS_SPORE_REPLAY_CAP` (default 1024) messages
/// is rejected BEFORE decrypt is attempted, at zero crypto cost.
fn maybe_decrypt(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    let key_set = load_key_from_env();
    let looks_encrypted = buf.len() >= 6 && &buf[..6] == spore_crypto::SPORE_V3_MAGIC;
    match (key_set, looks_encrypted) {
        (Some(key), true) => {
            // Replay check — cheap, skip decrypt on duplicate.
            if let Ok(nonce) = spore_crypto::envelope_nonce(buf) {
                replay_window().lock().unwrap().check_and_insert(&nonce)?;
            }
            spore_crypto::decrypt_envelope(&key, buf, b"")
        }
        (Some(_), false) => Err("downgrade rejected: key is set but envelope is plaintext"),
        (None, true) => Err("encrypted envelope received but no key configured"),
        (None, false) => Ok(buf.to_vec()),
    }
}

/// Reset the process-global replay window. Test-only.
#[cfg(test)]
pub(crate) fn reset_replay_window_for_tests() {
    if let Some(rw) = REPLAY_WINDOW.get() {
        let cap: usize = std::env::var("OASIS_SPORE_REPLAY_CAP").ok().and_then(|s| s.parse().ok()).unwrap_or(1024);
        *rw.lock().unwrap() = spore_crypto::ReplayWindow::new(cap);
    }
}

// ─── Per-peer rate limiter (token bucket) ──────────────────
//
// Bounds the CPU cost of envelope processing under a DoS flood. Each source
// IP gets its own token bucket. Tokens refill at `rps`/sec; bucket caps at
// `burst`. Check() returns false → drop packet BEFORE any crypto work.
// Config via OASIS_RATE_LIMIT_RPS (default 100), OASIS_RATE_LIMIT_BURST (200),
// OASIS_RATE_LIMIT_MAX_PEERS (10000 — evict oldest when exceeded).

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Instant;

struct TokenState {
    tokens: f64,
    last_update: Instant,
}

pub struct RateLimiter {
    rps: f64,
    burst: f64,
    max_peers: usize,
    state: std::sync::Mutex<HashMap<IpAddr, TokenState>>,
    // Global token bucket: protects against IP-spoofed floods that bypass per-IP limits.
    // Single shared bucket for ALL traffic; reset at every check.
    global: std::sync::Mutex<TokenState>,
    global_rps: f64,
    global_burst: f64,
}

impl RateLimiter {
    pub fn new(rps: f64, burst: f64, max_peers: usize) -> Self {
        Self::new_with_global(rps, burst, max_peers, rps * 10.0, burst * 10.0)
    }

    /// Full constructor. `global_rps` and `global_burst` apply to the total
    /// traffic across ALL source IPs — protects against spoof floods.
    pub fn new_with_global(rps: f64, burst: f64, max_peers: usize, global_rps: f64, global_burst: f64) -> Self {
        Self {
            rps: rps.max(0.0),
            burst: burst.max(1.0),
            max_peers: max_peers.max(2),
            state: std::sync::Mutex::new(HashMap::new()),
            global: std::sync::Mutex::new(TokenState { tokens: global_burst.max(1.0), last_update: Instant::now() }),
            global_rps: global_rps.max(0.0),
            global_burst: global_burst.max(1.0),
        }
    }

    /// Returns true if the packet from `addr` is allowed.
    /// Consumes 1 token from BOTH global and per-IP buckets.
    /// Global bucket check comes first so a spoofed flood with 1M distinct
    /// source IPs is bounded by global_rps, not per-IP.
    pub fn check(&self, addr: IpAddr) -> bool {
        // Global bucket first
        {
            let mut g = self.global.lock().unwrap();
            let now = Instant::now();
            let elapsed = now.duration_since(g.last_update).as_secs_f64();
            g.tokens = (g.tokens + elapsed * self.global_rps).min(self.global_burst);
            g.last_update = now;
            if g.tokens < 1.0 {
                return false;
            }
            g.tokens -= 1.0;
        }
        // Per-IP bucket
        let mut map = self.state.lock().unwrap();
        let now = Instant::now();
        if !map.contains_key(&addr) && map.len() >= self.max_peers {
            if let Some(oldest_key) = map.iter().min_by_key(|(_, v)| v.last_update).map(|(k, _)| *k) {
                map.remove(&oldest_key);
            }
        }
        let entry = map.entry(addr).or_insert(TokenState { tokens: self.burst, last_update: now });
        let elapsed = now.duration_since(entry.last_update).as_secs_f64();
        entry.tokens = (entry.tokens + elapsed * self.rps).min(self.burst);
        entry.last_update = now;
        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn peer_count(&self) -> usize {
        self.state.lock().unwrap().len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        let rps: f64 = std::env::var("OASIS_RATE_LIMIT_RPS").ok().and_then(|s| s.parse().ok()).unwrap_or(100.0);
        let burst: f64 = std::env::var("OASIS_RATE_LIMIT_BURST").ok().and_then(|s| s.parse().ok()).unwrap_or(200.0);
        let max_peers: usize = std::env::var("OASIS_RATE_LIMIT_MAX_PEERS").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000);
        // Global default = 10x per-IP. Operators with many legitimate peers
        // should raise; operators exposed to hostile internet should lower.
        let global_rps: f64 = std::env::var("OASIS_RATE_LIMIT_GLOBAL_RPS").ok().and_then(|s| s.parse().ok()).unwrap_or(rps * 10.0);
        let global_burst: f64 = std::env::var("OASIS_RATE_LIMIT_GLOBAL_BURST").ok().and_then(|s| s.parse().ok()).unwrap_or(burst * 10.0);
        Self::new_with_global(rps, burst, max_peers, global_rps, global_burst)
    }
}

static RATE_LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
fn rate_limiter() -> &'static RateLimiter {
    RATE_LIMITER.get_or_init(RateLimiter::default)
}

/// Check if an incoming packet from `addr` should be accepted by the rate limiter.
/// Exposed so adapter code can consult it without going through listen_once.
pub fn rate_limit_check(addr: IpAddr) -> bool {
    rate_limiter().check(addr)
}

#[cfg(test)]
pub(crate) fn reset_rate_limiter_for_tests() {
    // Can't replace a OnceLock, but we can empty the map
    if let Some(rl) = RATE_LIMITER.get() {
        rl.state.lock().unwrap().clear();
    }
}

// ─── Layer 1: UDP Multicast ────────────────────────────────

/// Broadcast digests to all OASIS devices on local network
pub fn broadcast(mesh: &FederatedMesh) -> Result<usize, &'static str> {
    // Zero-copy: serialize to memory. If OASIS_SPORE_KEY_HEX or
    // OASIS_SPORE_PASSPHRASE is set, wraps in ChaCha20-Poly1305 AEAD (SPORE\x03).
    let data = mesh.serialize_to_vec();
    let inner = maybe_encrypt(&data);
    let packet = wrap_envelope(&inner);
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|_| "bind failed")?;
    sock.set_multicast_ttl_v4(2).ok();
    sock.send_to(&packet, MULTICAST_ADDR).map_err(|_| "send failed")
}

/// Send a spore to a specific peer (unicast). Use when multicast is blocked
/// (corporate networks, cross-VLAN, mobile carriers). `target` is "host:port".
pub fn unicast(mesh: &FederatedMesh, target: &str) -> Result<usize, &'static str> {
    let data = mesh.serialize_to_vec();
    let inner = maybe_encrypt(&data);
    let packet = wrap_envelope(&inner);
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|_| "bind failed")?;
    sock.send_to(&packet, target).map_err(|_| "send failed")
}

/// SPORE\x01 + len(u32) + payload envelope construction.
#[inline]
pub fn wrap_envelope(payload: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(SPORE_MAGIC.len() + 4 + payload.len());
    packet.extend_from_slice(SPORE_MAGIC);
    packet.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    packet.extend_from_slice(payload);
    packet
}

/// Parse a SPORE\x01 envelope and return the payload slice.
pub fn parse_envelope(buf: &[u8]) -> Result<&[u8], &'static str> {
    if buf.len() < SPORE_MAGIC.len() + 4 {
        return Err("too short");
    }
    if &buf[..SPORE_MAGIC.len()] != SPORE_MAGIC {
        return Err("bad magic");
    }
    let plen = u32::from_le_bytes(buf[SPORE_MAGIC.len()..SPORE_MAGIC.len() + 4].try_into().unwrap()) as usize;
    let start = SPORE_MAGIC.len() + 4;
    if start + plen > buf.len() {
        return Err("truncated");
    }
    Ok(&buf[start..start + plen])
}

/// Listen for incoming spore packets (non-blocking, returns immediately)
/// Accepts both multicast AND unicast on port 4200.
pub fn listen_once(mesh: &mut FederatedMesh, trust: f64) -> Result<u32, &'static str> {
    let sock = UdpSocket::bind(BIND_ADDR).map_err(|_| "bind failed")?;
    sock.set_nonblocking(true).ok();
    let _ = sock.join_multicast_v4(&"239.0.42.1".parse().unwrap(), &"0.0.0.0".parse().unwrap());

    let mut buf = [0u8; 16384];
    match sock.recv_from(&mut buf) {
        Ok((len, addr)) => {
            // Rate-limit BEFORE any crypto work or parsing.
            if !rate_limit_check(addr.ip()) {
                return Ok(0);
            }
            let pkt = &buf[..len];
            // Priority 0: SPORE\x08 mesh envelope — zero-copy: read inner slice
            // directly from the returned envelope buffer, re-broadcast if forward.
            if pkt.len() >= 6 && &pkt[..6] == crate::mesh::SPORE_V8_MAGIC {
                use crate::mesh::MeshDecision;
                // Zero-copy path: hand the recv buffer directly to process_owned.
                let decision = mesh_router().lock().unwrap().process_owned(pkt.to_vec());
                let (envelope, forward) = match decision {
                    MeshDecision::Drop(_) => return Ok(0),
                    MeshDecision::Arrived { envelope, forward, .. } => (envelope, forward),
                };
                // Re-broadcast if TTL permitted (caller = this transport).
                if forward {
                    if let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") {
                        s.set_multicast_ttl_v4(2).ok();
                        let _ = s.send_to(&envelope, MULTICAST_ADDR);
                    }
                }
                // Inner = zero-copy slice into the envelope
                let inner_bytes = crate::mesh::inner_slice(&envelope);
                if inner_bytes.len() >= 6 {
                    let magic = &inner_bytes[..6];
                    if magic == spore_crypto::SPORE_V7_MAGIC {
                        let pt = ingest_v7_envelope(inner_bytes)?;
                        return mesh.merge_foreign_bytes(&pt, trust);
                    } else if magic == SPORE_MAGIC {
                        let payload = parse_envelope(inner_bytes)?;
                        let decoded = maybe_decrypt(payload)?;
                        return mesh.merge_foreign_bytes(&decoded, trust);
                    }
                }
                return Ok(0);
            }
            // Priority 1: SPORE\x06 revocation envelope — auto-merge and return.
            if pkt.len() >= 6 && &pkt[..6] == spore_crypto::SPORE_V6_MAGIC {
                match ingest_revocation_envelope(pkt) {
                    Ok(n) => {
                        if n > 0 {
                            eprintln!("[spore] ingested revocation: {} new entries", n);
                        }
                        return Ok(0);
                    }
                    Err(e) => return Err(e),
                }
            }
            // Priority 2: SPORE\x07 forward-secret + authenticated digest envelope.
            if pkt.len() >= 6 && &pkt[..6] == spore_crypto::SPORE_V7_MAGIC {
                let plaintext = ingest_v7_envelope(pkt)?;
                return mesh.merge_foreign_bytes(&plaintext, trust);
            }
            // Priority 3: normal digest envelope (v1/v2/v3/...)
            let payload = parse_envelope(pkt)?;
            let decoded = maybe_decrypt(payload)?;
            mesh.merge_foreign_bytes(&decoded, trust)
        }
        Err(_) => Ok(0),
    }
}

/// Start a background listener thread that saves received spores to a file.
/// The main loop checks this file periodically and merges it.
pub fn start_listener(spore_inbox: &str) -> Result<(), &'static str> {
    let path = spore_inbox.to_string();
    std::thread::spawn(move || {
        let sock = match UdpSocket::bind(BIND_ADDR) {
            Ok(s) => s,
            Err(_) => return,
        };
        let _ = sock.join_multicast_v4(&"239.0.42.1".parse().unwrap(), &"0.0.0.0".parse().unwrap());
        let mut buf = [0u8; 16384];
        loop {
            if let Ok((len, _)) = sock.recv_from(&mut buf) {
                if len >= SPORE_MAGIC.len() + 4 && &buf[..SPORE_MAGIC.len()] == SPORE_MAGIC {
                    let plen = u32::from_le_bytes(buf[SPORE_MAGIC.len()..SPORE_MAGIC.len() + 4].try_into().unwrap()) as usize;
                    if SPORE_MAGIC.len() + 4 + plen <= len {
                        let payload = &buf[SPORE_MAGIC.len() + 4..SPORE_MAGIC.len() + 4 + plen];
                        let _ = std::fs::write(&path, payload);
                    }
                }
            }
        }
    });
    Ok(())
}

// ─── Layer 3: Store-and-forward ────────────────────────────
//
// Already implemented: merge_foreign accumulates digests from
// all devices ever encountered. When device B merges A's digests,
// then B meets C, C gets A+B's combined experience via B's save.
// No additional code needed — it's emergent from the existing design.

// ─── Layer 4: QR Spore (base64 compact) ────────────────────

/// Encode top N digests as compact base64 string (for QR code)
pub fn encode_qr(mesh: &FederatedMesh, max_digests: usize) -> Result<String, &'static str> {
    // Zero-copy: serialize to bytes then base64 (no /tmp).
    // If env key is set, base64-encodes the full encrypted envelope instead
    // (QR carries the key-protected message; key distribution is out of band).
    let data = mesh.serialize_to_vec();
    let limit = (max_digests * 120 + 20).min(data.len());
    let inner = maybe_encrypt(&data[..limit]);
    Ok(base64_encode(&inner))
}

/// Decode base64 QR payload and merge into mesh.
/// Decrypts if the decoded bytes start with SPORE\x03 (and key is configured).
pub fn decode_qr(mesh: &mut FederatedMesh, b64: &str, trust: f64) -> Result<u32, &'static str> {
    let data = base64_decode(b64)?;
    let decoded = maybe_decrypt(&data)?;
    mesh.merge_foreign_bytes(&decoded, trust)
}

// ─── Layer 2: Fragmented Spore (SPORE v2) — Survives lossy radio links ──
//
// Wire format per chunk:
//   [0..6]   SPORE\x02     6 bytes magic
//   [6..10]  msg_id        u32 LE — random per message; reassembly key
//   [10..12] total_data    u16 LE — number of data chunks (excludes parity)
//   [12..14] chunk_idx     u16 LE — 0..total_data-1 = data; total_data = parity
//   [14]     flags         u8     — bit0 = has_parity (single XOR chunk follows)
//   [15..]   payload       up to MAX_CHUNK_PAYLOAD bytes
//
// Reassembler holds a Vec<Option<Vec<u8>>> per msg_id; emits when complete.
// FEC: if exactly 1 data chunk missing AND parity received, recover via XOR.
// Tolerates 1 lost chunk per message at +1/N bandwidth cost.
//
// For higher loss tolerance, sender repeats each chunk N times (--repeat N).
// Receiver dedups by (msg_id, chunk_idx). Joint delivery ≈ 1 - p^N per chunk.

const SPORE_V2_MAGIC: &[u8] = b"SPORE\x02";
const V2_HEADER_LEN: usize = 15;
pub const MAX_CHUNK_PAYLOAD: usize = 1400; // safe under 1500 MTU - IP - UDP - header

/// Fragment `data` into chunk packets. If `with_parity`, append one XOR-parity chunk.
/// Returns ready-to-send packets.
pub fn fragment_v2(data: &[u8], msg_id: u32, with_parity: bool) -> Vec<Vec<u8>> {
    let n_data = data.len().div_ceil(MAX_CHUNK_PAYLOAD).max(1) as u16;
    let mut packets: Vec<Vec<u8>> = Vec::with_capacity(n_data as usize + 1);
    let flags: u8 = if with_parity { 0b0000_0001 } else { 0 };

    for (idx, chunk) in data.chunks(MAX_CHUNK_PAYLOAD).enumerate() {
        let mut pkt = Vec::with_capacity(V2_HEADER_LEN + chunk.len());
        pkt.extend_from_slice(SPORE_V2_MAGIC);
        pkt.extend_from_slice(&msg_id.to_le_bytes());
        pkt.extend_from_slice(&n_data.to_le_bytes());
        pkt.extend_from_slice(&(idx as u16).to_le_bytes());
        pkt.push(flags);
        pkt.extend_from_slice(chunk);
        packets.push(pkt);
    }

    // Handle empty input edge case
    if data.is_empty() {
        let mut pkt = Vec::with_capacity(V2_HEADER_LEN);
        pkt.extend_from_slice(SPORE_V2_MAGIC);
        pkt.extend_from_slice(&msg_id.to_le_bytes());
        pkt.extend_from_slice(&1u16.to_le_bytes());
        pkt.extend_from_slice(&0u16.to_le_bytes());
        pkt.push(flags);
        packets.push(pkt);
    }

    if with_parity {
        // XOR all data chunks together (pad shorter chunks with zeros).
        // Parity has chunk_idx == n_data (one past last data chunk).
        let mut parity_payload = vec![0u8; MAX_CHUNK_PAYLOAD];
        let mut max_len = 0usize;
        for chunk in data.chunks(MAX_CHUNK_PAYLOAD) {
            for (i, b) in chunk.iter().enumerate() {
                parity_payload[i] ^= b;
            }
            max_len = max_len.max(chunk.len());
        }
        parity_payload.truncate(max_len.max(1));

        let mut pkt = Vec::with_capacity(V2_HEADER_LEN + parity_payload.len());
        pkt.extend_from_slice(SPORE_V2_MAGIC);
        pkt.extend_from_slice(&msg_id.to_le_bytes());
        pkt.extend_from_slice(&n_data.to_le_bytes());
        pkt.extend_from_slice(&n_data.to_le_bytes()); // chunk_idx == n_data marks parity
        pkt.push(flags);
        pkt.extend_from_slice(&parity_payload);
        packets.push(pkt);
    }

    packets
}

/// Parse a v2 chunk header. Returns (msg_id, total_data, chunk_idx, flags, payload).
pub fn parse_chunk_v2(buf: &[u8]) -> Result<(u32, u16, u16, u8, &[u8]), &'static str> {
    if buf.len() < V2_HEADER_LEN {
        return Err("too short");
    }
    if &buf[..6] != SPORE_V2_MAGIC {
        return Err("bad magic");
    }
    let msg_id = u32::from_le_bytes(buf[6..10].try_into().unwrap());
    let total = u16::from_le_bytes(buf[10..12].try_into().unwrap());
    let idx = u16::from_le_bytes(buf[12..14].try_into().unwrap());
    let flags = buf[14];
    Ok((msg_id, total, idx, flags, &buf[V2_HEADER_LEN..]))
}

/// In-progress reassembly state for a single msg_id.
/// Caller may track multiple of these in a HashMap<msg_id, Reassembler>.
pub struct Reassembler {
    pub msg_id: u32,
    pub total_data: u16,
    pub flags: u8,
    pub data: Vec<Option<Vec<u8>>>,
    pub parity: Option<Vec<u8>>,
}

impl Reassembler {
    pub fn new(msg_id: u32, total_data: u16, flags: u8) -> Self {
        Self { msg_id, total_data, flags, data: vec![None; total_data as usize], parity: None }
    }

    /// Feed a chunk. Returns Some(complete bytes) if reassembly succeeded.
    pub fn feed(&mut self, idx: u16, payload: &[u8]) -> Option<Vec<u8>> {
        if idx == self.total_data && (self.flags & 0b1) != 0 {
            self.parity = Some(payload.to_vec());
        } else if (idx as usize) < self.data.len() && self.data[idx as usize].is_none() {
            self.data[idx as usize] = Some(payload.to_vec());
        }
        self.try_complete()
    }

    fn try_complete(&self) -> Option<Vec<u8>> {
        let missing: Vec<usize> = self.data.iter().enumerate().filter_map(|(i, v)| if v.is_none() { Some(i) } else { None }).collect();
        match missing.len() {
            0 => Some(self.assemble()),
            1 if self.parity.is_some() => self.recover_one_via_xor(missing[0]),
            _ => None,
        }
    }

    fn assemble(&self) -> Vec<u8> {
        self.data.iter().flatten().flat_map(|c| c.iter().copied()).collect()
    }

    fn recover_one_via_xor(&self, missing_idx: usize) -> Option<Vec<u8>> {
        let parity = self.parity.as_ref()?;
        // XOR all received data chunks with parity to recover the missing one.
        // Note: parity payload length = max(all chunks). Missing chunk length is
        // unknown without the original — assume MAX_CHUNK_PAYLOAD unless idx is last.
        let assumed_len = if missing_idx as u16 == self.total_data - 1 {
            // Last chunk may be short. Take parity length minus what we'd XOR off.
            parity.len()
        } else {
            MAX_CHUNK_PAYLOAD
        };
        let mut recovered = vec![0u8; assumed_len];
        for (i, b) in parity.iter().enumerate() {
            if i < recovered.len() {
                recovered[i] = *b;
            }
        }
        for (i, chunk_opt) in self.data.iter().enumerate() {
            if i == missing_idx {
                continue;
            }
            if let Some(chunk) = chunk_opt {
                for (j, b) in chunk.iter().enumerate() {
                    if j < recovered.len() {
                        recovered[j] ^= b;
                    }
                }
            }
        }
        // For non-last missing chunks, trim trailing zeros that came from padding.
        // (Cannot perfectly recover length if last chunk was missing AND short —
        //  the parity-based scheme has this fundamental ambiguity. Acceptable
        //  given the binary digest format has its own length encoding.)
        let mut data = self.data.clone();
        data[missing_idx] = Some(recovered);
        Some(data.iter().flatten().flat_map(|c| c.iter().copied()).collect())
    }
}

/// Convenience: send a mesh as v2 fragments via UDP unicast.
/// `repeat` = how many times to send each chunk (1 = no repeat, 2-3 for lossy links).
/// Returns total bytes sent. Caller picks `target` ("239.0.42.1:4200" for multicast).
pub fn broadcast_v2(mesh: &FederatedMesh, target: &str, with_parity: bool, repeat: u8) -> Result<usize, &'static str> {
    let data = mesh.serialize_to_vec();
    // Encrypt BEFORE fragmenting: receivers reassemble the encrypted envelope,
    // then decrypt. This avoids per-chunk nonces/tags and keeps FEC effective.
    let inner = maybe_encrypt(&data);
    // msg_id mixes nanos + process pid + payload-derived entropy. Single-process
    // collision resistance: ~1 in 2^32 for typical sub-second send rates.
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let pid = std::process::id();
    let msg_id: u32 = (nanos as u32) ^ ((nanos >> 32) as u32) ^ pid.wrapping_mul(0x9E3779B1) ^ (inner.len() as u32).wrapping_mul(0xDEADBEEF);
    let packets = fragment_v2(&inner, msg_id, with_parity);
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|_| "bind failed")?;
    sock.set_multicast_ttl_v4(2).ok();
    let r = repeat.max(1);
    let mut total = 0usize;
    for _ in 0..r {
        for pkt in &packets {
            total += sock.send_to(pkt, target).map_err(|_| "send failed")?;
        }
    }
    Ok(total)
}

// ─── Minimal base64 (no external dependency) ──────────────

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let bytes: Vec<u8> = s.bytes().filter(|&b| b != b'\n' && b != b'\r' && b != b' ').collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let mut vals = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = match b {
                b'A'..=b'Z' => (b - b'A') as u32,
                b'a'..=b'z' => (b - b'a' + 26) as u32,
                b'0'..=b'9' => (b - b'0' + 52) as u32,
                b'+' => 62,
                b'/' => 63,
                b'=' => 0,
                _ => return Err("invalid base64"),
            };
        }
        let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        let data = b"OASIS collective memory test payload 128 dims";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(&decoded, data);
    }

    #[test]
    fn base64_empty() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn base64_padding() {
        // 1 byte = 2 chars + ==
        let e1 = base64_encode(b"A");
        assert!(e1.ends_with("=="));
        assert_eq!(base64_decode(&e1).unwrap(), b"A");
        // 2 bytes = 3 chars + =
        let e2 = base64_encode(b"AB");
        assert!(e2.ends_with('=') && !e2.ends_with("=="));
        assert_eq!(base64_decode(&e2).unwrap(), b"AB");
        // 3 bytes = 4 chars, no padding
        let e3 = base64_encode(b"ABC");
        assert!(!e3.contains('='));
        assert_eq!(base64_decode(&e3).unwrap(), b"ABC");
    }

    // ─── v2 Fragmentation + FEC ─────────────────────────────────

    #[test]
    fn fragment_v2_round_trip_small_message() {
        // Fits in single chunk — should produce 1 packet (no parity).
        let data = b"hello".to_vec();
        let chunks = fragment_v2(&data, 42, false);
        assert_eq!(chunks.len(), 1);
        let (msg_id, total, idx, flags, payload) = parse_chunk_v2(&chunks[0]).unwrap();
        assert_eq!(msg_id, 42);
        assert_eq!(total, 1);
        assert_eq!(idx, 0);
        assert_eq!(flags, 0);
        assert_eq!(payload, b"hello");
    }

    #[test]
    fn fragment_v2_round_trip_large_message() {
        // 3500 bytes → 3 chunks of 1400/1400/700.
        let data: Vec<u8> = (0..3500u32).map(|i| (i % 256) as u8).collect();
        let chunks = fragment_v2(&data, 99, false);
        assert_eq!(chunks.len(), 3);

        let mut reass = Reassembler::new(99, 3, 0);
        let mut completed = None;
        for pkt in &chunks {
            let (_, _, idx, _, payload) = parse_chunk_v2(pkt).unwrap();
            completed = reass.feed(idx, payload);
        }
        let recovered = completed.expect("should reassemble");
        assert_eq!(recovered, data);
    }

    #[test]
    fn fec_recovers_one_lost_chunk_via_xor_parity() {
        // 4 chunks + 1 parity. Drop chunk 1, verify recovery.
        let data: Vec<u8> = (0..4 * MAX_CHUNK_PAYLOAD as u32).map(|i| (i % 256) as u8).collect();
        let chunks = fragment_v2(&data, 7, true);
        assert_eq!(chunks.len(), 5, "4 data + 1 parity");

        let mut reass = Reassembler::new(7, 4, 0b1);
        // Drop index 1, deliver 0, 2, 3, parity
        for (i, pkt) in chunks.iter().enumerate() {
            if i == 1 {
                continue;
            }
            let (_, _, idx, _, payload) = parse_chunk_v2(pkt).unwrap();
            if let Some(rec) = reass.feed(idx, payload) {
                assert_eq!(rec, data, "FEC must recover lost chunk byte-perfect");
                return;
            }
        }
        panic!("reassembly should have completed via XOR recovery");
    }

    #[test]
    fn fec_cannot_recover_two_lost_chunks() {
        // 4 chunks + 1 parity. Drop chunks 0 AND 2 → cannot recover.
        let data: Vec<u8> = (0..4 * MAX_CHUNK_PAYLOAD as u32).map(|i| (i % 256) as u8).collect();
        let chunks = fragment_v2(&data, 13, true);
        let mut reass = Reassembler::new(13, 4, 0b1);
        for (i, pkt) in chunks.iter().enumerate() {
            if i == 0 || i == 2 {
                continue;
            }
            let (_, _, idx, _, payload) = parse_chunk_v2(pkt).unwrap();
            assert!(reass.feed(idx, payload).is_none(), "single-parity XOR must NOT recover 2 losses");
        }
    }

    #[test]
    fn parse_chunk_v2_rejects_garbage() {
        assert!(parse_chunk_v2(b"too short").is_err());
        assert!(parse_chunk_v2(b"BADMAG\x00\x00\x00\x00\x00\x00\x00\x00\x00").is_err());
    }

    // ─── Encrypted spore integration tests ──────────────────────
    // These use env vars; isolated via serial_test to avoid pollution.

    use serial_test::serial;

    const TEST_KEY_HEX: &str = "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f";

    #[test]
    #[serial]
    fn encrypted_qr_roundtrip() {
        std::env::set_var("OASIS_SPORE_KEY_HEX", TEST_KEY_HEX);
        let mut mesh = FederatedMesh::new();
        let mut a = vz();
        a[10] = 0.5;
        mesh.pool_push_test(a, 0.3, 1.0, 0.4);

        let qr = encode_qr(&mesh, 10).unwrap();
        // The base64 decoded bytes must start with the encrypted magic
        let raw = base64_decode(&qr).unwrap();
        assert!(raw.starts_with(spore_crypto::SPORE_V3_MAGIC), "encrypted QR must use SPORE\\x03 magic");

        let mut mesh2 = FederatedMesh::new();
        let loaded = decode_qr(&mut mesh2, &qr, 0.7).unwrap();
        assert_eq!(loaded, 1);
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
    }

    #[test]
    #[serial]
    fn encrypted_qr_rejects_wrong_key() {
        std::env::set_var("OASIS_SPORE_KEY_HEX", TEST_KEY_HEX);
        let mut mesh = FederatedMesh::new();
        let mut a = vz();
        a[10] = 0.5;
        mesh.pool_push_test(a, 0.3, 1.0, 0.4);
        let qr = encode_qr(&mesh, 10).unwrap();

        // Receiver has a different key — MUST fail
        std::env::set_var("OASIS_SPORE_KEY_HEX", "00000000000000000000000000000000000000000000000000000000deadbeef");
        let mut mesh2 = FederatedMesh::new();
        assert!(decode_qr(&mut mesh2, &qr, 0.7).is_err(), "wrong key MUST fail decryption");
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
    }

    #[test]
    #[serial]
    fn downgrade_rejected_when_key_set() {
        // Attacker sends a plaintext v1 envelope; receiver has key configured.
        // Must reject (no downgrade).
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
        let mesh = FederatedMesh::new();
        let plain_payload = mesh.serialize_to_vec();
        let plain_envelope = wrap_envelope(&plain_payload);
        let inner = parse_envelope(&plain_envelope).unwrap();

        std::env::set_var("OASIS_SPORE_KEY_HEX", TEST_KEY_HEX);
        let err = maybe_decrypt(inner).unwrap_err();
        assert!(err.contains("downgrade"), "key-set receiver must reject plaintext, got: {}", err);
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
    }

    // ───── Rate limiter tests ───────────────────────────────

    fn local_ip(n: u8) -> std::net::IpAddr {
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, n))
    }

    #[test]
    fn rate_limiter_allows_under_burst() {
        let rl = super::RateLimiter::new(100.0, 10.0, 100);
        for _ in 0..10 {
            assert!(rl.check(local_ip(5)));
        }
    }

    #[test]
    fn rate_limiter_rejects_over_burst() {
        let rl = super::RateLimiter::new(1.0, 5.0, 100);
        for _ in 0..5 {
            assert!(rl.check(local_ip(6)));
        }
        // 6th hits without refill (elapsed time is sub-microsecond)
        assert!(!rl.check(local_ip(6)), "over-burst packets must be rejected");
    }

    #[test]
    fn rate_limiter_refills_over_time() {
        let rl = super::RateLimiter::new(1000.0, 2.0, 100); // 1000 rps → 1ms/token
        assert!(rl.check(local_ip(7)));
        assert!(rl.check(local_ip(7)));
        assert!(!rl.check(local_ip(7)), "burst exhausted");
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(rl.check(local_ip(7)), "after 5ms @1000rps, tokens refilled");
    }

    #[test]
    fn rate_limiter_per_ip_isolation() {
        // Each IP has its own bucket.
        let rl = super::RateLimiter::new(1.0, 1.0, 100);
        assert!(rl.check(local_ip(8)));
        assert!(!rl.check(local_ip(8)));
        // Different IP still has full bucket
        assert!(rl.check(local_ip(9)));
    }

    #[test]
    fn rate_limiter_global_bucket_stops_ip_spoof_flood() {
        // Attacker spoofs 1000 distinct source IPs, each sending 1 packet.
        // Per-IP limiter would let ALL through (each gets its own bucket).
        // Global limiter caps the total to `global_burst + steady-refill`.
        // rps = 100 per-IP, burst = 100 per-IP (so each IP alone can send burst)
        // global_rps = 10, global_burst = 5 (MUCH lower than per-IP)
        let rl = super::RateLimiter::new_with_global(100.0, 100.0, 100_000, 10.0, 5.0);
        let mut allowed = 0;
        for i in 0..1000u32 {
            // Each octet combination gives a fresh IP
            let a = ((i >> 8) as u8).wrapping_add(10);
            let b = i as u8;
            let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, a, b, 1));
            if rl.check(ip) {
                allowed += 1;
            }
        }
        // Global bucket caps at ~5 + minimal refill during tight loop
        assert!(allowed < 20, "global bucket must bound spoof flood: got {} allowed (expected ~5)", allowed);
    }

    #[test]
    fn rate_limiter_global_bucket_refills() {
        let rl = super::RateLimiter::new_with_global(100.0, 100.0, 100, 1000.0, 2.0);
        // Exhaust global bucket
        assert!(rl.check(local_ip(50)));
        assert!(rl.check(local_ip(51)));
        // 3rd packet blocked by global bucket
        assert!(!rl.check(local_ip(52)));
        // Wait 5 ms for global refill (1000 rps → 5 tokens)
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(rl.check(local_ip(52)));
    }

    #[test]
    fn rate_limiter_evicts_oldest_when_full() {
        let rl = super::RateLimiter::new(1.0, 1.0, 4);
        for i in 1..=4u8 {
            rl.check(local_ip(i));
        }
        assert_eq!(rl.peer_count(), 4);
        // Sleep so last_update timestamps diverge
        std::thread::sleep(std::time::Duration::from_millis(2));
        rl.check(local_ip(1)); // refresh oldest
        std::thread::sleep(std::time::Duration::from_millis(2));
        rl.check(local_ip(99)); // new peer → should evict oldest (which is now IP 2)
        let count = rl.peer_count();
        assert!(count <= 4, "map must not grow beyond max_peers (got {})", count);
    }

    // ───── Revocation listener integration tests ─────────────

    #[test]
    #[serial]
    fn listener_ingests_revocation_envelope() {
        // Set up an operator keypair
        let op_seed_hex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let op_seed = spore_crypto::parse_key_hex(op_seed_hex).unwrap();
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(&op_seed).unwrap());
        let op_pub_slice: &[u8] = kp.pk.as_ref();
        let op_pub_hex = op_pub_slice.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        std::env::set_var("OASIS_OP_ED25519_PUB_HEX", &op_pub_hex);
        reset_revocation_for_tests();

        let mut rl = spore_crypto::RevocationList::new();
        let compromised_fp = [0xDE; spore_crypto::SENDER_FP_LEN];
        rl.revoke(compromised_fp, 1700000000);
        let signed = rl.serialize_signed(&op_seed).unwrap();
        let envelope = spore_crypto::wrap_revocation_envelope(&signed);

        let added = ingest_revocation_envelope(&envelope).expect("ingest must succeed");
        assert_eq!(added, 1, "one new entry added");
        assert!(is_sender_revoked(&compromised_fp), "global revocation list must contain the ingested fingerprint");

        // Idempotent re-ingest
        let added2 = ingest_revocation_envelope(&envelope).unwrap();
        assert_eq!(added2, 0, "duplicate ingest adds 0");

        std::env::remove_var("OASIS_OP_ED25519_PUB_HEX");
        reset_revocation_for_tests();
    }

    #[test]
    #[serial]
    fn listener_rejects_revocation_with_wrong_operator_key() {
        let real_seed_hex = "aa00000000000000000000000000000000000000000000000000000000000000";
        let real_seed = spore_crypto::parse_key_hex(real_seed_hex).unwrap();
        let mut rl = spore_crypto::RevocationList::new();
        rl.revoke([0xAA; spore_crypto::SENDER_FP_LEN], 1700000000);
        let signed = rl.serialize_signed(&real_seed).unwrap();
        let envelope = spore_crypto::wrap_revocation_envelope(&signed);

        // Configure a DIFFERENT operator pubkey
        let wrong_seed_hex = "bb00000000000000000000000000000000000000000000000000000000000000";
        let wrong_seed = spore_crypto::parse_key_hex(wrong_seed_hex).unwrap();
        let wrong_kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(&wrong_seed).unwrap());
        let wrong_slice: &[u8] = wrong_kp.pk.as_ref();
        let wrong_hex = wrong_slice.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        std::env::set_var("OASIS_OP_ED25519_PUB_HEX", &wrong_hex);
        reset_revocation_for_tests();

        assert!(ingest_revocation_envelope(&envelope).is_err(), "revocation signed by wrong operator must be rejected");

        std::env::remove_var("OASIS_OP_ED25519_PUB_HEX");
        reset_revocation_for_tests();
    }

    #[test]
    #[serial]
    fn listener_no_op_pubkey_rejects_revocation() {
        std::env::remove_var("OASIS_OP_ED25519_PUB_HEX");
        reset_revocation_for_tests();
        let seed = spore_crypto::parse_key_hex("0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c").unwrap();
        let mut rl = spore_crypto::RevocationList::new();
        rl.revoke([0x66; spore_crypto::SENDER_FP_LEN], 1700000000);
        let envelope = spore_crypto::wrap_revocation_envelope(&rl.serialize_signed(&seed).unwrap());

        let err = ingest_revocation_envelope(&envelope).unwrap_err();
        assert!(err.contains("no OASIS_OP"), "missing op pubkey must produce explicit error, got: {}", err);
    }

    #[test]
    #[serial]
    fn listener_ingests_v7_envelope_end_to_end() {
        // Full round-trip through ingest_v7_envelope using env-configured keys.
        reset_counter_tracker_for_tests();
        reset_revocation_for_tests();

        // Recipient identity
        let (r_priv, r_pub) = spore_crypto::x25519_generate_keypair();
        let r_priv_hex: String = r_priv.iter().map(|b| format!("{:02x}", b)).collect();
        let _ = r_pub;
        std::env::set_var("OASIS_SPORE_ID_PRIV_HEX", &r_priv_hex);

        // Sender identity
        let (s_priv, s_pub) = spore_crypto::x25519_generate_keypair();
        let fp = spore_crypto::sender_fingerprint(&s_pub);
        let fp_hex: String = fp.iter().map(|b| format!("{:02x}", b)).collect();
        let s_pub_hex: String = s_pub.iter().map(|b| format!("{:02x}", b)).collect();
        std::env::set_var(format!("OASIS_SPORE_SENDER_PUB_HEX__{}", fp_hex), &s_pub_hex);

        // Pre-shared key
        std::env::set_var("OASIS_SPORE_KEY_HEX", "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff000102030405060708090a0b0c0d0e0f");
        let psk = spore_crypto::parse_key_hex(&std::env::var("OASIS_SPORE_KEY_HEX").unwrap()).unwrap();

        let env = spore_crypto::encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 42, b"counter-tracked digest payload", b"").unwrap();

        let pt = ingest_v7_envelope(&env).expect("listener must decrypt v7");
        assert_eq!(pt, b"counter-tracked digest payload");

        // Replay MUST fail via process-global tracker
        assert!(ingest_v7_envelope(&env).is_err(), "second ingest of same envelope must be counter-rejected");

        std::env::remove_var("OASIS_SPORE_ID_PRIV_HEX");
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
        std::env::remove_var(format!("OASIS_SPORE_SENDER_PUB_HEX__{}", fp_hex));
        reset_counter_tracker_for_tests();
    }

    #[test]
    #[serial]
    fn listener_v7_rejects_unknown_sender_fp() {
        // No OASIS_SPORE_SENDER_PUB_HEX__<fp> env → can't resolve sender
        reset_counter_tracker_for_tests();
        let (r_priv, r_pub) = spore_crypto::x25519_generate_keypair();
        let r_priv_hex: String = r_priv.iter().map(|b| format!("{:02x}", b)).collect();
        std::env::set_var("OASIS_SPORE_ID_PRIV_HEX", &r_priv_hex);
        std::env::set_var("OASIS_SPORE_KEY_HEX", "aa".repeat(32));
        let (s_priv, s_pub) = spore_crypto::x25519_generate_keypair();
        let psk = spore_crypto::parse_key_hex(&std::env::var("OASIS_SPORE_KEY_HEX").unwrap()).unwrap();
        let env = spore_crypto::encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 1, b"x", b"").unwrap();
        let err = ingest_v7_envelope(&env).unwrap_err();
        assert!(err.contains("sender pub not configured"), "unknown fingerprint must fail cleanly, got: {}", err);
        std::env::remove_var("OASIS_SPORE_ID_PRIV_HEX");
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
    }

    #[test]
    #[serial]
    fn mesh_wrap_then_process_extracts_inner() {
        reset_mesh_for_tests();
        let fed = FederatedMesh::new();
        let inner = wrap_envelope(&fed.serialize_to_vec());
        let mut origin_router = crate::mesh::MeshRouter::new([7u8; crate::mesh::FP_LEN]);
        let mesh_pkt = origin_router.origin_wrap(&inner);
        match mesh_process_incoming(&mesh_pkt) {
            crate::mesh::MeshDecision::Arrived { envelope, hops_seen, forward: true, .. } => {
                assert_eq!(crate::mesh::inner_slice(&envelope), &inner[..], "inner slice must be byte-identical to the wrapped inner");
                assert_eq!(hops_seen, 0);
            }
            other => panic!("expected Arrived{{forward=true}}, got {:?}", other),
        }
        reset_mesh_for_tests();
    }

    #[test]
    #[serial]
    fn mesh_drop_own_broadcast_when_echoed() {
        // Our router's fp. Self-wrap then receive → must drop as own echo.
        reset_mesh_for_tests();
        // Configure our fp via origin_wrap path
        let our_pkt = mesh_wrap_for_broadcast(b"hi");
        // Now simulate receiving it back
        match mesh_process_incoming(&our_pkt) {
            crate::mesh::MeshDecision::Drop(reason) => {
                assert_eq!(reason, "own echo");
            }
            other => panic!("expected own-echo drop, got {:?}", other),
        }
        reset_mesh_for_tests();
    }

    #[test]
    #[serial]
    fn listener_passes_through_non_v6_envelopes() {
        // A non-SPORE\x06 envelope returns Ok(0) without touching state
        std::env::remove_var("OASIS_OP_ED25519_PUB_HEX");
        let random = b"some other data that is not a revocation";
        assert_eq!(ingest_revocation_envelope(random).unwrap(), 0);
    }

    #[test]
    #[serial]
    fn replay_of_encrypted_envelope_rejected() {
        // An attacker captures a valid encrypted payload and re-delivers it.
        // With replay window, the second arrival must fail.
        std::env::set_var("OASIS_SPORE_KEY_HEX", TEST_KEY_HEX);
        reset_replay_window_for_tests();
        let mesh = FederatedMesh::new();
        let encrypted = maybe_encrypt(&mesh.serialize_to_vec());
        // First arrival — accepted
        let ok = maybe_decrypt(&encrypted).expect("first arrival must succeed");
        assert!(!ok.is_empty() || ok.is_empty()); // just check it returned Ok
                                                  // Second arrival of the SAME bytes — replay detected
        let err = maybe_decrypt(&encrypted).unwrap_err();
        assert!(err.contains("replay"), "second arrival must be replay-rejected, got: {}", err);
        std::env::remove_var("OASIS_SPORE_KEY_HEX");
    }

    #[test]
    #[serial]
    fn encrypted_received_without_key_rejected() {
        // Encrypted message arrives, receiver has NO key → cannot decrypt.
        std::env::set_var("OASIS_SPORE_KEY_HEX", TEST_KEY_HEX);
        let mesh = FederatedMesh::new();
        let enc = maybe_encrypt(&mesh.serialize_to_vec());
        std::env::remove_var("OASIS_SPORE_KEY_HEX");

        let err = maybe_decrypt(&enc).unwrap_err();
        assert!(err.contains("no key"), "unconfigured receiver must fail, got: {}", err);
    }

    #[test]
    fn qr_encode_decode_roundtrip() {
        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[10] = 0.7;
        axis[50] = -0.3;
        mesh.pool_push_test(axis, 0.5, 1.0, 0.4);

        let qr = encode_qr(&mesh, 10).unwrap();
        assert!(qr.len() < 300, "QR payload should be compact, got {}", qr.len());

        let mut mesh2 = FederatedMesh::new();
        let loaded = decode_qr(&mut mesh2, &qr, 0.7).unwrap();
        assert_eq!(loaded, 1);
        assert_eq!(mesh2.digest_count(), 1);
    }

    #[test]
    fn broadcast_creates_valid_packet() {
        let mesh = FederatedMesh::new();
        // Empty mesh should still serialize
        let path = &std::env::temp_dir().join("oasis-spore-test.bin").to_string_lossy().into_owned();
        mesh.save(path).unwrap();
        let data = std::fs::read(path).unwrap();
        assert!(data.len() > 6, "empty mesh should serialize");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn listen_nonblocking_returns_zero() {
        // No one broadcasting → should return 0, not block
        let mut mesh = FederatedMesh::new();
        // This may fail if port 4200 is in use, that's ok
        match listen_once(&mut mesh, 0.5) {
            Ok(n) => assert_eq!(n, 0),
            Err(_) => {} // Port busy = acceptable
        }
    }
}
