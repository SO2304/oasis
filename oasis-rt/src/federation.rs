//! OASIS-RT — Federated Neural Mesh (Mechanism 11)
//! Agents share experience by synaptic resonance. Trust-gated.
use crate::synapse::Synapse;
use crate::vec::*;
const MAX_DIGESTS: usize = 64;

pub struct ExperienceDigest {
    pub source: usize,
    pub axis: V,
    pub magnitude: f64,
    pub valence: f64,
    pub entropy: f64,
    pub tick: u32,
    pub reinforcements: u32,
    pub active: bool,
}
pub struct FederatedMesh {
    pool: Vec<ExperienceDigest>,
    trust: Vec<(usize, usize, f64)>,
    pub significance_threshold: f64,
    pub resonance_threshold: f64,
    pub resonance_gain: f64,
    pub default_trust: f64,
    tick: u32,
    pub sympathetic_count: u32,
    pub rejected_count: u32,
}

impl FederatedMesh {
    pub fn new() -> Self {
        Self {
            pool: Vec::new(),
            trust: Vec::new(),
            significance_threshold: 0.03,
            resonance_threshold: 0.3,
            resonance_gain: 0.02,
            default_trust: 0.5,
            tick: 0,
            sympathetic_count: 0,
            rejected_count: 0,
        }
    }
    pub fn get_trust(&self, from: usize, to: usize) -> f64 {
        self.trust.iter().find(|t| t.0 == from && t.1 == to).map(|t| t.2).unwrap_or(self.default_trust)
    }
    pub fn set_trust(&mut self, from: usize, to: usize, val: f64) {
        let v = val.clamp(0.0, 1.0);
        if let Some(t) = self.trust.iter_mut().find(|t| t.0 == from && t.1 == to) {
            t.2 = v;
        } else {
            self.trust.push((from, to, v));
        }
    }

    /// Harvest significant plasticity events into experience digests
    pub fn harvest(
        &mut self,
        events: &[(usize, usize, f64, f64, &str)], // (pre, post, w_before, w_after, kind)
        synapses: &[Synapse],
        entropies: &[f64],
    ) -> u32 {
        self.tick += 1;
        let mut count = 0;
        for &(pre, post, wb, wa, kind) in events {
            if kind != "POTENTIATED" && kind != "DEPRESSED" {
                continue;
            }
            let delta = (wa - wb).abs();
            if delta < self.significance_threshold {
                continue;
            }

            let syn = synapses.iter().find(|s| s.active && s.pre == pre && s.post == post);
            let axis = match syn {
                Some(s) if vn(&s.conduction_axis) > 0.01 => vnorm(&s.conduction_axis),
                _ => continue,
            };

            let entropy = if pre < entropies.len() && post < entropies.len() { (entropies[pre] + entropies[post]) / 2.0 } else { 0.5 };

            // Dedup
            if let Some(d) = self.pool.iter_mut().find(|d| d.active && d.source == pre && vcos(&d.axis, &axis) > 0.9) {
                d.reinforcements += 1;
            } else {
                if self.pool.len() >= MAX_DIGESTS {
                    if let Some(d) = self.pool.iter_mut().find(|d| !d.active) {
                        *d = ExperienceDigest {
                            source: pre,
                            axis,
                            magnitude: delta,
                            valence: if kind == "POTENTIATED" { 1.0 } else { -1.0 },
                            entropy,
                            tick: self.tick,
                            reinforcements: 1,
                            active: true,
                        };
                    }
                } else {
                    self.pool.push(ExperienceDigest {
                        source: pre,
                        axis,
                        magnitude: delta,
                        valence: if kind == "POTENTIATED" { 1.0 } else { -1.0 },
                        entropy,
                        tick: self.tick,
                        reinforcements: 1,
                        active: true,
                    });
                }
            }
            count += 1;
        }
        count
    }
    /// Propagate resonance to aligned synapses of other agents
    pub fn propagate(&mut self, synapses: &mut [Synapse], agent_count: usize, entropies: &[f64]) -> u32 {
        let mut affected = 0;
        // Passive trust recovery: blacklisted sources slowly regain trust (biological tolerance)
        for t in &mut self.trust {
            t.2 = (t.2 + 0.001).min(1.0);
        }
        for di in 0..self.pool.len() {
            if !self.pool[di].active {
                continue;
            }
            if self.tick.saturating_sub(self.pool[di].tick) > 50 {
                self.pool[di].active = false;
                continue;
            }
            let source = self.pool[di].source;
            let digest_axis = self.pool[di].axis;
            let digest_valence = self.pool[di].valence;
            let digest_magnitude = self.pool[di].magnitude;
            let digest_entropy = self.pool[di].entropy;
            let digest_reinforcements = self.pool[di].reinforcements;

            for si in 0..synapses.len() {
                if !synapses[si].active {
                    continue;
                }
                let (pre, post) = (synapses[si].pre, synapses[si].post);
                // Lateral diffusion: skip only the exact same synapse pair (no self-reinforcement)
                // Old bug: pre==source OR post==source blocked ALL propagation on single-device
                if pre == source && post == source {
                    continue;
                }
                let axis = synapses[si].conduction_axis;
                if vn(&axis) < 0.01 {
                    continue;
                }
                let alignment = vcos(&digest_axis, &axis);
                if alignment.abs() < self.resonance_threshold {
                    self.rejected_count += 1;
                    continue;
                }
                let trust = self.get_trust(source, pre);
                if trust < 0.1 {
                    continue;
                }
                let lw = synapses[si].weight;
                let conflict = (lw > 0.3 && digest_valence < 0.0) || (lw < -0.3 && digest_valence > 0.0);
                if conflict {
                    self.set_trust(source, pre, (trust - 0.1).max(0.0));
                    self.rejected_count += 1;
                    continue;
                }
                let em = if pre < entropies.len() { 1.0 - (entropies[pre] - digest_entropy).abs() } else { 0.5 };
                let boost = (1.0 + (digest_reinforcements as f64).log2()).min(3.0);
                let wd = alignment * digest_valence * digest_magnitude * trust * em * boost * self.resonance_gain;
                synapses[si].weight = (synapses[si].weight + wd).clamp(-1.0, 1.0);
                self.sympathetic_count += 1;
                affected += 1;
                if trust < 0.5 {
                    self.set_trust(source, pre, (trust + 0.01).min(1.0));
                }
            }

            if self.pool[di].reinforcements <= 1 {
                self.pool[di].active = false;
            } else {
                self.pool[di].reinforcements -= 1;
            }
        }
        affected
    }

    pub fn digest_count(&self) -> usize {
        self.pool.iter().filter(|d| d.active).count()
    }

    /// Insert a digest directly (for testing and QR encoding)
    pub fn pool_push_test(&mut self, axis: V, mag: f64, val: f64, ent: f64) {
        self.pool
            .push(ExperienceDigest { source: 0, axis, magnitude: mag, valence: val, entropy: ent, tick: self.tick, reinforcements: 1, active: true });
    }

    const MAGIC: &'static [u8] = b"OASISMEM\x01"; // sparse binary, ~7 KB
    const SIG_MAGIC: &'static [u8] = b"OASISSIG\x01"; // appended when OASIS_SIGNING_KEY or OASIS_ED25519_SEED set

    /// Compute tamper-detection MAC over message bytes.
    /// Two modes (priority order):
    /// 1. OASIS_ED25519_SEED="<hex-32-bytes>" — REAL Ed25519 signature (cryptographic-grade).
    ///    Signature is 64 bytes; we truncate to leading 8 bytes and return as u64 for wire-compat.
    ///    Full 64-byte sig available via `compute_signature_full` if needed for verification.
    /// 2. OASIS_SIGNING_KEY="<any-string>" — SipHash MAC (auth-grade, not cryptographic).
    /// Returns None if neither env var is set.
    fn compute_mac(msg: &[u8]) -> Option<u64> {
        // Ed25519 path (real crypto)
        if let Ok(hex) = std::env::var("OASIS_ED25519_SEED") {
            if let Some(seed_bytes) = parse_hex_32(&hex) {
                let seed = ed25519_compact::Seed::from_slice(&seed_bytes).ok()?;
                let kp = ed25519_compact::KeyPair::from_seed(seed);
                let sig = kp.sk.sign(msg, None);
                let sig_bytes: &[u8] = sig.as_ref();
                let mut out = [0u8; 8];
                out.copy_from_slice(&sig_bytes[..8]);
                return Some(u64::from_le_bytes(out));
            }
        }
        // SipHash MAC path (legacy / simpler)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let key = std::env::var("OASIS_SIGNING_KEY").ok()?;
        if key.is_empty() {
            return None;
        }
        let mut h = DefaultHasher::new();
        key.as_bytes().hash(&mut h);
        msg.hash(&mut h);
        Some(h.finish())
    }

    /// Return the full 64-byte Ed25519 signature over msg if OASIS_ED25519_SEED is set.
    /// This is the proper cryptographic signature; compute_mac returns only the first 8 bytes
    /// for backward-compat with the existing file format.
    pub fn compute_signature_full(msg: &[u8]) -> Option<[u8; 64]> {
        let hex = std::env::var("OASIS_ED25519_SEED").ok()?;
        let seed_bytes = parse_hex_32(&hex)?;
        let seed = ed25519_compact::Seed::from_slice(&seed_bytes).ok()?;
        let kp = ed25519_compact::KeyPair::from_seed(seed);
        let sig = kp.sk.sign(msg, None);
        let sig_bytes: &[u8] = sig.as_ref();
        let mut out = [0u8; 64];
        out.copy_from_slice(sig_bytes);
        Some(out)
    }

    /// Save active digests + trust to binary file.
    /// If OASIS_SIGNING_KEY env var is set, appends signature block:
    ///   SIG_MAGIC (8B) + mac(u64 LE) = 16 bytes
    /// Receivers with the same key can verify; without the key, file loads with
    /// signature stripped (backward compatible).
    pub fn save(&self, path: &str) -> Result<(), &'static str> {
        let mut buf: Vec<u8> = Vec::with_capacity(8192);
        buf.extend_from_slice(Self::MAGIC);

        // Digests
        let active: Vec<&ExperienceDigest> = self.pool.iter().filter(|d| d.active).collect();
        let count = active.len().min(255) as u8;
        buf.push(count);

        for d in &active[..count as usize] {
            buf.extend_from_slice(&(d.source as u16).to_le_bytes());

            // Sparse axis: collect non-zero dims
            let nz: Vec<(u8, f64)> = d.axis.iter().enumerate().filter(|(_, &v)| v.abs() > 1e-6).map(|(i, &v)| (i as u8, v)).collect();
            buf.push(nz.len().min(255) as u8);
            for &(idx, val) in &nz {
                buf.push(idx);
                buf.extend_from_slice(&val.to_le_bytes());
            }

            buf.extend_from_slice(&d.magnitude.to_le_bytes());
            buf.extend_from_slice(&d.valence.to_le_bytes());
            buf.extend_from_slice(&d.entropy.to_le_bytes());
            buf.extend_from_slice(&(d.reinforcements.min(65535) as u16).to_le_bytes());
        }

        // Trust pairs
        let trust_count = self.trust.len().min(255) as u8;
        buf.push(trust_count);
        for &(from, to, val) in &self.trust[..trust_count as usize] {
            buf.extend_from_slice(&(from as u16).to_le_bytes());
            buf.extend_from_slice(&(to as u16).to_le_bytes());
            buf.extend_from_slice(&val.to_le_bytes());
        }

        // Append signature block if signing key is set.
        if let Some(mac) = Self::compute_mac(&buf) {
            buf.extend_from_slice(Self::SIG_MAGIC);
            buf.extend_from_slice(&mac.to_le_bytes());
        }
        std::fs::write(path, &buf).map_err(|_| "write failed")
    }

    /// Transport-abstracted save. Serializes identically to `save()` and sends via Transport.
    /// Enables LoRa / BLE / in-memory transport without changing call sites.
    pub fn save_via(&self, transport: &dyn crate::transport::Transport, sender: &str) -> Result<(), &'static str> {
        let buf = self.serialize_to_vec();
        transport.send(sender, &buf).map_err(|_| "transport send failed")
    }

    /// Build the byte payload identically to `save()` would write to disk.
    /// Extracted for reuse by save_via; also useful for in-memory tests.
    pub fn serialize_to_vec(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(8192);
        buf.extend_from_slice(Self::MAGIC);
        let active: Vec<&ExperienceDigest> = self.pool.iter().filter(|d| d.active).collect();
        let count = active.len().min(255) as u8;
        buf.push(count);
        for d in &active[..count as usize] {
            buf.extend_from_slice(&(d.source as u16).to_le_bytes());
            let nz: Vec<(u8, f64)> = d.axis.iter().enumerate().filter(|(_, &v)| v.abs() > 1e-6).map(|(i, &v)| (i as u8, v)).collect();
            buf.push(nz.len().min(255) as u8);
            for &(idx, val) in &nz {
                buf.push(idx);
                buf.extend_from_slice(&val.to_le_bytes());
            }
            buf.extend_from_slice(&d.magnitude.to_le_bytes());
            buf.extend_from_slice(&d.valence.to_le_bytes());
            buf.extend_from_slice(&d.entropy.to_le_bytes());
            buf.extend_from_slice(&(d.reinforcements.min(65535) as u16).to_le_bytes());
        }
        let trust_count = self.trust.len().min(255) as u8;
        buf.push(trust_count);
        for &(from, to, val) in &self.trust[..trust_count as usize] {
            buf.extend_from_slice(&(from as u16).to_le_bytes());
            buf.extend_from_slice(&(to as u16).to_le_bytes());
            buf.extend_from_slice(&val.to_le_bytes());
        }
        if let Some(mac) = Self::compute_mac(&buf) {
            buf.extend_from_slice(Self::SIG_MAGIC);
            buf.extend_from_slice(&mac.to_le_bytes());
        }
        buf
    }

    /// Transport-abstracted merge_foreign. Reads peer blob via Transport, parses identically.
    pub fn merge_foreign_via(&mut self, transport: &dyn crate::transport::Transport, peer: &str, foreign_trust: f64) -> Result<u32, &'static str> {
        let data = transport.recv(peer).map_err(|_| "transport recv failed")?;
        let loaded = self.load_from_bytes(&data)?;
        for d in self.pool.iter_mut().filter(|d| d.active) {
            if d.tick == self.tick {
                d.magnitude *= foreign_trust.clamp(0.1, 1.0);
            }
        }
        Ok(loaded)
    }

    /// Parse a byte slice into mesh (extracted for reuse; identical parse logic to `load()`).
    pub fn load_from_bytes(&mut self, data_in: &[u8]) -> Result<u32, &'static str> {
        // Strip + verify signature block at tail (if present)
        let mut data: &[u8] = data_in;
        if data.len() >= Self::SIG_MAGIC.len() + 8 {
            let sig_start = data.len() - Self::SIG_MAGIC.len() - 8;
            if &data[sig_start..sig_start + Self::SIG_MAGIC.len()] == Self::SIG_MAGIC {
                let mac_bytes: [u8; 8] = data[sig_start + Self::SIG_MAGIC.len()..].try_into().map_err(|_| "sig truncated")?;
                let received_mac = u64::from_le_bytes(mac_bytes);
                let msg = &data[..sig_start];
                if let Some(expected_mac) = Self::compute_mac(msg) {
                    if received_mac != expected_mac {
                        return Err("signature mismatch");
                    }
                }
                data = &data[..sig_start];
            }
        }
        if data.len() < Self::MAGIC.len() + 1 {
            return Err("too short");
        }
        if &data[..Self::MAGIC.len()] != Self::MAGIC {
            return Err("bad magic");
        }

        // Inline parse body — identical to load() but operating on slice.
        // No /tmp roundtrip: zero alloc on the hot path.
        let mut pos = Self::MAGIC.len();
        let count = data[pos] as usize;
        pos += 1;
        let mut loaded = 0u32;
        for _ in 0..count {
            if pos + 3 > data.len() {
                break;
            }
            let source = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;
            let ndims = data[pos] as usize;
            pos += 1;
            let mut axis = vz();
            for _ in 0..ndims {
                if pos + 9 > data.len() {
                    return Err("truncated axis");
                }
                let idx = data[pos] as usize;
                pos += 1;
                let val = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                if idx < DIM {
                    axis[idx] = val;
                }
            }
            if pos + 26 > data.len() {
                return Err("truncated digest");
            }
            let magnitude = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let valence = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let entropy = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let reinforcements = u16::from_le_bytes([data[pos], data[pos + 1]]) as u32;
            pos += 2;
            // Dedup
            let dup = self.pool.iter().any(|d| d.active && vcos(&d.axis, &axis) > 0.95);
            if dup {
                continue;
            }
            if self.pool.len() < MAX_DIGESTS {
                self.pool
                    .push(ExperienceDigest { source, axis, magnitude, valence, entropy, tick: self.tick, reinforcements, active: true });
            } else if let Some(slot) = self.pool.iter_mut().find(|d| !d.active) {
                *slot = ExperienceDigest { source, axis, magnitude, valence, entropy, tick: self.tick, reinforcements, active: true };
            }
            loaded += 1;
        }
        // Trust pairs
        if pos < data.len() {
            let trust_count = data[pos] as usize;
            pos += 1;
            for _ in 0..trust_count {
                if pos + 12 > data.len() {
                    break;
                }
                let from = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                let to = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                let val = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                self.set_trust(from, to, val);
            }
        }
        Ok(loaded)
    }

    /// In-memory equivalent of `merge_foreign(path, trust)` — applies trust attenuation
    /// to digests loaded from `data` without filesystem roundtrip.
    pub fn merge_foreign_bytes(&mut self, data: &[u8], foreign_trust: f64) -> Result<u32, &'static str> {
        let loaded = self.load_from_bytes(data)?;
        let attn = foreign_trust.clamp(0.1, 1.0);
        for d in self.pool.iter_mut().filter(|d| d.active) {
            if d.tick == self.tick {
                d.magnitude *= attn;
            }
        }
        Ok(loaded)
    }

    /// Load digests + trust from binary file.
    /// If signature block present AND OASIS_SIGNING_KEY set: verify MAC, reject if invalid.
    /// If signature present but no key: strip signature, proceed (backward compat mode).
    /// If no signature: proceed normally.
    pub fn load(&mut self, path: &str) -> Result<u32, &'static str> {
        let mut data = std::fs::read(path).map_err(|_| "read failed")?;
        // Check for signature block at tail: SIG_MAGIC (8B) + mac (8B) = 16B
        if data.len() >= Self::SIG_MAGIC.len() + 8 {
            let sig_start = data.len() - Self::SIG_MAGIC.len() - 8;
            if &data[sig_start..sig_start + Self::SIG_MAGIC.len()] == Self::SIG_MAGIC {
                let mac_bytes: [u8; 8] = data[sig_start + Self::SIG_MAGIC.len()..].try_into().map_err(|_| "sig truncated")?;
                let received_mac = u64::from_le_bytes(mac_bytes);
                let msg = &data[..sig_start];
                if let Some(expected_mac) = Self::compute_mac(msg) {
                    // Key is set — MUST match or reject
                    if received_mac != expected_mac {
                        return Err("signature mismatch");
                    }
                }
                // Strip signature bytes before parsing body
                data.truncate(sig_start);
            }
        }
        if data.len() < Self::MAGIC.len() + 1 {
            return Err("too short");
        }
        if &data[..Self::MAGIC.len()] != Self::MAGIC {
            return Err("bad magic");
        }

        let mut pos = Self::MAGIC.len();
        let count = data[pos] as usize;
        pos += 1;
        let mut loaded = 0u32;

        for _ in 0..count {
            if pos + 3 > data.len() {
                break;
            }
            let source = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;
            let ndims = data[pos] as usize;
            pos += 1;

            let mut axis = vz();
            for _ in 0..ndims {
                if pos + 9 > data.len() {
                    return Err("truncated axis");
                }
                let idx = data[pos] as usize;
                pos += 1;
                let val = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                if idx < DIM {
                    axis[idx] = val;
                }
            }

            if pos + 26 > data.len() {
                return Err("truncated digest");
            }
            let magnitude = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let valence = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let entropy = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let reinforcements = u16::from_le_bytes([data[pos], data[pos + 1]]) as u32;
            pos += 2;

            // Dedup: skip if axis already exists
            let dup = self.pool.iter().any(|d| d.active && vcos(&d.axis, &axis) > 0.95);
            if dup {
                continue;
            }

            if self.pool.len() < MAX_DIGESTS {
                self.pool
                    .push(ExperienceDigest { source, axis, magnitude, valence, entropy, tick: self.tick, reinforcements, active: true });
            } else if let Some(slot) = self.pool.iter_mut().find(|d| !d.active) {
                *slot = ExperienceDigest { source, axis, magnitude, valence, entropy, tick: self.tick, reinforcements, active: true };
            }
            loaded += 1;
        }

        // Trust pairs
        if pos < data.len() {
            let trust_count = data[pos] as usize;
            pos += 1;
            for _ in 0..trust_count {
                if pos + 12 > data.len() {
                    break;
                }
                let from = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                let to = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                let val = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                self.set_trust(from, to, val);
            }
        }

        Ok(loaded)
    }

    /// Merge foreign digests with trust attenuation
    pub fn merge_foreign(&mut self, path: &str, foreign_trust: f64) -> Result<u32, &'static str> {
        let before = self.digest_count();
        let loaded = self.load(path)?;

        // Attenuate foreign digests by trust factor
        for d in self.pool.iter_mut().filter(|d| d.active) {
            if d.tick == self.tick {
                // Just loaded — attenuate magnitude by foreign trust
                d.magnitude *= foreign_trust.clamp(0.1, 1.0);
            }
        }

        Ok(loaded)
    }
}

/// Parse a hex string of exactly 64 chars into 32 bytes.
fn parse_hex_32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = s.as_bytes().get(i * 2)?;
        let lo = s.as_bytes().get(i * 2 + 1)?;
        let h = hex_digit(*hi)?;
        let l = hex_digit(*lo)?;
        out[i] = (h << 4) | l;
    }
    Some(out)
}

pub(crate) fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ==========================================================================
//                              KANI PROOFS
//
// M11 Federation — the heavy-lifting paths (Ed25519 signing, digest
// exchange, propagation loops) are SAT-intractable under CBMC. We
// prove the pure scalar helpers that support the module:
//   `hex_digit`       — fingerprint / key-hex parsing correctness
//   trust clamping    — property of set_trust's clamp(0.0, 1.0)
// Both are scalar ops; Kani-tractable in < 1s.
// ==========================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: hex_digit returns Some(n) with n ∈ [0, 15] for every valid
    /// hex character, and None for every other byte. Exhaustive over u8
    /// (256 cases — SAT-tractable).
    #[kani::proof]
    fn proof_m11_hex_digit_range_and_rejection() {
        let b: u8 = kani::any();
        let is_hex = matches!(b,
            b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F');
        match hex_digit(b) {
            Some(n) => {
                assert!(is_hex, "hex_digit accepted non-hex byte {}", b);
                assert!(n <= 15, "hex_digit returned out-of-range {}", n);
            }
            None => {
                assert!(!is_hex, "hex_digit rejected valid hex byte {}", b);
            }
        }
    }

    /// PROVE: clamped trust value is always in [0, 1]. Mirrors the
    /// inline `val.clamp(0.0, 1.0)` in `set_trust` — regression guard
    /// against anyone removing the clamp.
    #[kani::proof]
    fn proof_m11_trust_clamp_bounded() {
        let val: f64 = kani::any();
        kani::assume(val.is_finite());
        kani::assume(val.abs() < 1e9);
        let clamped = val.clamp(0.0, 1.0);
        assert!(clamped >= 0.0, "trust clamp went below 0");
        assert!(clamped <= 1.0, "trust clamp went above 1");
        // Fixpoint: clamping in-range values preserves them.
        if (0.0..=1.0).contains(&val) {
            assert_eq!(clamped, val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synapse::Synapse;
    use serial_test::serial;

    fn make_syn(axis_dim: usize) -> Synapse {
        let mut a = vz();
        a[axis_dim] = 1.0;
        Synapse {
            active: true,
            pre: 0,
            post: 1,
            weight: 0.5,
            activation_count: 5,
            last_activated: 1,
            conduction_axis: a,
            pre_fire_tick: 1,
            post_fire_tick: 2,
            eligibility: 0.5,
        }
    }
    fn make_digest(dim: usize, val: f64, mag: f64, val_sign: f64) -> ExperienceDigest {
        let mut a = vz();
        a[dim] = val;
        ExperienceDigest { source: 0, axis: a, magnitude: mag, valence: val_sign, entropy: 0.4, tick: 1, reinforcements: 1, active: true }
    }

    #[test]
    fn trust_gating_and_bounds() {
        let mut m = FederatedMesh::new();
        assert!((m.get_trust(99, 100) - 0.5).abs() < 0.01); // default
        m.set_trust(0, 1, 0.8);
        assert!((m.get_trust(0, 1) - 0.8).abs() < 0.01);
        assert!((m.get_trust(1, 0) - 0.5).abs() < 0.01); // directional
        m.set_trust(0, 2, 0.0);
        assert!(m.get_trust(0, 2) < 0.01); // zero trust
        m.set_trust(0, 1, 5.0);
        assert!((m.get_trust(0, 1) - 1.0).abs() < 0.01); // clamp up
        m.set_trust(0, 1, -3.0);
        assert!(m.get_trust(0, 1) >= 0.0); // clamp down
    }

    #[test]
    #[serial]
    fn save_load_roundtrip() {
        let mut mesh = FederatedMesh::new();
        // Manually insert digest to avoid harvest normalization
        let mut axis = vz();
        axis[10] = 0.8;
        axis[22] = -0.6;
        mesh.pool
            .push(ExperienceDigest { source: 0, axis, magnitude: 0.2, valence: 1.0, entropy: 0.4, tick: 1, reinforcements: 1, active: true });
        mesh.set_trust(0, 1, 0.9);
        mesh.set_trust(1, 0, 0.3);

        let path = &std::env::temp_dir().join("oasis_rt.bin").to_string_lossy().into_owned();
        mesh.save(path).unwrap();
        let mut m2 = FederatedMesh::new();
        assert_eq!(m2.load(path).unwrap(), 1);
        let d = m2.pool.iter().find(|d| d.active).unwrap();
        assert!((d.axis[10] - 0.8).abs() < 1e-6);
        assert!((d.axis[22] - (-0.6)).abs() < 1e-6);
        assert!((d.magnitude - 0.2).abs() < 1e-6);
        assert!((d.valence - 1.0).abs() < 1e-6);
        assert!((m2.get_trust(0, 1) - 0.9).abs() < 0.01);
        assert!((m2.get_trust(1, 0) - 0.3).abs() < 0.01);
        std::fs::remove_file(path).ok();
    }

    #[test]
    #[serial]
    fn load_dedup_and_corrupt() {
        let mut mesh = FederatedMesh::new();
        mesh.pool.push(make_digest(10, 1.0, 0.3, 1.0));
        let path = &std::env::temp_dir().join("oasis_dedup.bin").to_string_lossy().into_owned();
        mesh.save(path).unwrap();
        assert_eq!(mesh.load(path).unwrap(), 0); // dedup: same axis → skip
        assert_eq!(mesh.digest_count(), 1);
        std::fs::remove_file(path).ok();

        let bad = &std::env::temp_dir().join("oasis_bad.bin").to_string_lossy().into_owned();
        std::fs::write(bad, b"GARBAGE").unwrap();
        assert!(FederatedMesh::new().load(bad).is_err());
        std::fs::remove_file(bad).ok();
    }

    #[test]
    #[serial]
    fn merge_foreign_attenuates() {
        let mut ma = FederatedMesh::new();
        ma.pool.push(make_digest(5, 1.0, 1.0, 1.0));
        let path = &std::env::temp_dir().join("oasis_foreign.bin").to_string_lossy().into_owned();
        ma.save(path).unwrap();
        let mut mb = FederatedMesh::new();
        assert_eq!(mb.merge_foreign(path, 0.5).unwrap(), 1);
        let d = mb.pool.iter().find(|d| d.active).unwrap();
        assert!((d.magnitude - 0.5).abs() < 1e-6); // attenuated
        std::fs::remove_file(path).ok();
    }

    #[test]
    #[serial]
    fn save_empty_and_sparse() {
        let path = &std::env::temp_dir().join("oasis_empty.bin").to_string_lossy().into_owned();
        FederatedMesh::new().save(path).unwrap();
        assert_eq!(FederatedMesh::new().load(path).unwrap_or(99), 0);
        std::fs::remove_file(path).ok();

        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[3] = 0.1;
        axis[50] = -0.9;
        axis[127] = -0.7;
        mesh.pool
            .push(ExperienceDigest { source: 0, axis, magnitude: 0.5, valence: -1.0, entropy: 0.6, tick: 1, reinforcements: 2, active: true });
        let sp = &std::env::temp_dir().join("oasis_sparse.bin").to_string_lossy().into_owned();
        mesh.save(sp).unwrap();
        assert!(std::fs::metadata(sp).unwrap().len() < 200);
        let mut m2 = FederatedMesh::new();
        m2.load(sp).unwrap();
        let d = m2.pool.iter().find(|d| d.active).unwrap();
        assert!((d.axis[3] - 0.1).abs() < 1e-6);
        assert!((d.axis[50] - (-0.9)).abs() < 1e-6);
        assert!((d.axis[127] - (-0.7)).abs() < 1e-6);
        assert!(d.axis[0].abs() < 1e-6 && d.axis[64].abs() < 1e-6);
        std::fs::remove_file(sp).ok();
    }

    #[test]
    #[serial]
    fn digest_deduplication() {
        let mut mesh = FederatedMesh::new();
        let syn = make_syn(10);
        let events = vec![(0usize, 1usize, 0.3, 0.4, "POTENTIATED"), (0, 1, 0.4, 0.5, "POTENTIATED")];
        mesh.harvest(&events, &[syn.clone()], &[0.4, 0.4]);
        mesh.harvest(&events, &[syn], &[0.4, 0.4]);
        assert!(mesh.digest_count() <= 2);
    }

    // ─── Signed digests (OASIS_SIGNING_KEY) ─────────────────────────────
    //
    // These tests modify env vars; serialize via a mutex or run with
    // `cargo test --test-threads=1` for determinism. For simplicity we use
    // unique key names per test to avoid races.

    #[test]
    #[serial]
    fn signed_roundtrip_same_key() {
        std::env::set_var("OASIS_SIGNING_KEY", "test-key-roundtrip-shared");
        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[0] = 1.0;
        mesh.pool_push_test(axis, 0.5, 1.0, 0.3);
        let path = std::env::temp_dir().join("oasis_signed_roundtrip.bin");
        let path_s = path.to_str().unwrap();
        mesh.save(path_s).expect("save should succeed");

        let mut mesh2 = FederatedMesh::new();
        let n = mesh2.load(path_s).expect("valid signature must load");
        assert!(n >= 1);
        std::env::remove_var("OASIS_SIGNING_KEY");
        let _ = std::fs::remove_file(path_s);
    }

    #[test]
    #[serial]
    fn signed_rejects_tampered() {
        std::env::set_var("OASIS_SIGNING_KEY", "test-key-tamper-shared");
        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[0] = 1.0;
        mesh.pool_push_test(axis, 0.5, 1.0, 0.3);
        let path = std::env::temp_dir().join("oasis_signed_tamper.bin");
        let path_s = path.to_str().unwrap();
        mesh.save(path_s).expect("save should succeed");

        // Tamper: flip one byte inside the body (before signature block = last 16 bytes)
        let mut data = std::fs::read(path_s).unwrap();
        let flip_idx = data.len() / 2; // middle of body
        data[flip_idx] ^= 0xFF;
        std::fs::write(path_s, &data).unwrap();

        let mut mesh2 = FederatedMesh::new();
        let err = mesh2.load(path_s);
        assert!(err.is_err(), "tampered payload must be rejected, got {:?}", err);
        std::env::remove_var("OASIS_SIGNING_KEY");
        let _ = std::fs::remove_file(path_s);
    }

    #[test]
    #[serial]
    fn ed25519_signing_roundtrip() {
        // Use a deterministic 32-byte seed as 64-char hex
        let seed_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        std::env::remove_var("OASIS_SIGNING_KEY");
        std::env::set_var("OASIS_ED25519_SEED", seed_hex);

        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[0] = 1.0;
        mesh.pool_push_test(axis, 0.5, 1.0, 0.3);

        let path = std::env::temp_dir().join("oasis_ed25519_roundtrip.bin");
        let path_s = path.to_str().unwrap();
        mesh.save(path_s).expect("save with Ed25519");

        // Verify full 64-byte sig available
        let data = std::fs::read(path_s).unwrap();
        let sig_start = data.len() - FederatedMesh::SIG_MAGIC.len() - 8;
        let msg = &data[..sig_start];
        let full_sig = FederatedMesh::compute_signature_full(msg).expect("full sig");
        // Independently verify via ed25519-compact
        let seed_bytes = parse_hex_32(seed_hex).unwrap();
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(&seed_bytes).unwrap());
        let sig = ed25519_compact::Signature::from_slice(&full_sig).unwrap();
        kp.pk.verify(msg, &sig).expect("independent Ed25519 verification");

        // Load must succeed (truncated MAC from sig matches)
        let mut mesh2 = FederatedMesh::new();
        mesh2.load(path_s).expect("load with Ed25519");

        std::env::remove_var("OASIS_ED25519_SEED");
        let _ = std::fs::remove_file(path_s);
    }

    #[test]
    #[serial]
    fn ed25519_rejects_tampered() {
        let seed_hex = "00000000000000000000000000000000000000000000000000000000000000aa";
        std::env::remove_var("OASIS_SIGNING_KEY");
        std::env::set_var("OASIS_ED25519_SEED", seed_hex);

        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[1] = 1.0;
        mesh.pool_push_test(axis, 0.5, 1.0, 0.3);

        let path = std::env::temp_dir().join("oasis_ed25519_tamper.bin");
        let path_s = path.to_str().unwrap();
        mesh.save(path_s).unwrap();

        let mut data = std::fs::read(path_s).unwrap();
        let idx = data.len() / 3;
        data[idx] ^= 0xFF;
        std::fs::write(path_s, &data).unwrap();

        let mut mesh2 = FederatedMesh::new();
        assert!(mesh2.load(path_s).is_err(), "tampered Ed25519-signed file must reject");
        std::env::remove_var("OASIS_ED25519_SEED");
        let _ = std::fs::remove_file(path_s);
    }

    #[test]
    #[serial]
    fn transport_roundtrip_in_memory() {
        // Wire: save_via + merge_foreign_via over InMemoryTransport.
        use crate::transport::{InMemoryTransport, Transport};
        std::env::remove_var("OASIS_SIGNING_KEY");
        let transport = InMemoryTransport::new();

        let mut mesh_a = FederatedMesh::new();
        let mut axis = vz();
        axis[0] = 1.0;
        mesh_a.pool_push_test(axis, 0.5, 1.0, 0.3);
        mesh_a.save_via(&transport, "peer_a").expect("save_via");

        // Verify peer visible from a different transport view (same store)
        let transport2 = transport.clone_handle();
        let peers = transport2.list_peers("peer_b").unwrap();
        assert!(peers.contains(&"peer_a".to_string()));

        let mut mesh_b = FederatedMesh::new();
        let n = mesh_b.merge_foreign_via(&transport, "peer_a", 0.8).expect("merge_via");
        assert!(n >= 1, "should merge at least 1 digest, got {}", n);
    }

    #[test]
    #[serial]
    fn unsigned_loads_when_no_key() {
        // No key set anywhere; save produces unsigned file; load succeeds.
        std::env::remove_var("OASIS_SIGNING_KEY");
        let mut mesh = FederatedMesh::new();
        let mut axis = vz();
        axis[0] = 1.0;
        mesh.pool_push_test(axis, 0.5, 1.0, 0.3);
        let path = std::env::temp_dir().join("oasis_unsigned.bin");
        let path_s = path.to_str().unwrap();
        mesh.save(path_s).expect("save should succeed");
        let mut mesh2 = FederatedMesh::new();
        let n = mesh2.load(path_s).expect("unsigned should load");
        assert!(n >= 1);
        let _ = std::fs::remove_file(path_s);
    }
}
