//! OASIS-RT — minimal MAVLink v2 frame parser for the 5 messages needed to bridge
//! PX4/ArduPilot → OASIS `drone_bridge.exe` stdin protocol.
//!
//! This is NOT a full MAVLink stack. It parses the minimum set of messages and
//! encodes one outbound (SET_POSITION_TARGET_LOCAL_NED). For production use,
//! replace with `mavlink` crate (~3k deps) or `rust-mavlink` codegen.
//!
//! Scope:
//! - MAVLink v2 magic 0xFD, 10-byte header + payload + 2-byte CRC-16/MCRF4XX
//! - Parsing: HEARTBEAT (#0), ATTITUDE (#30), GLOBAL_POSITION_INT (#33),
//!   DISTANCE_SENSOR (#132), SYS_STATUS (#1)
//! - Encoding: SET_POSITION_TARGET_LOCAL_NED (#84)
//!
//! All numeric types match MAVLink XML field types (little-endian).
//! CRC verification: currently OMITTED for simplicity (noted as TODO).
//! Payload truncation / extensions: not supported (MAVLink 2 optional feature).

#![allow(dead_code)]

const MAV_V2_MAGIC: u8 = 0xFD;
const MAVLINK_IFLAG_SIGNED: u8 = 0x01;

// ─── MAVLink v2 signing (spec: SHA256 of secret_key || frame || link_id || timestamp) ───

/// Compute 6-byte MAVLink v2 signature over the given frame (header+payload+CRC) + 7-byte suffix
/// (link_id + 6-byte timestamp). Returns `None` if secret key missing.
pub fn compute_mav_signature(secret_key_32: &[u8; 32], frame_without_sig: &[u8], link_id: u8, timestamp: u64) -> [u8; 6] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(secret_key_32);
    hasher.update(frame_without_sig);
    hasher.update([link_id]);
    // Timestamp is 48 bits (6 bytes) little-endian in MAVLink spec (10us increments since 2015-01-01)
    let ts_bytes = timestamp.to_le_bytes();
    hasher.update(&ts_bytes[..6]);
    let digest = hasher.finalize();
    let mut sig = [0u8; 6];
    sig.copy_from_slice(&digest[..6]);
    sig
}

/// Append 13-byte signature tail to an existing MAVLink v2 frame.
/// Frame's `incompat_flags` byte (index 2) must already have MAVLINK_IFLAG_SIGNED set.
/// Returns new frame with tail appended.
pub fn sign_frame(frame: Vec<u8>, secret_key_32: &[u8; 32], link_id: u8, timestamp: u64) -> Vec<u8> {
    let mut out = frame;
    let sig = compute_mav_signature(secret_key_32, &out, link_id, timestamp);
    out.push(link_id);
    let ts_bytes = timestamp.to_le_bytes();
    out.extend_from_slice(&ts_bytes[..6]);
    out.extend_from_slice(&sig);
    out
}

/// Verify a signed MAVLink v2 frame. Expects 13-byte signature tail after standard frame.
/// Returns true if signature matches.
pub fn verify_signature(frame_full: &[u8], secret_key_32: &[u8; 32]) -> bool {
    if frame_full.len() < 12 + 13 { return false; }
    let sig_start = frame_full.len() - 13;
    let msg = &frame_full[..sig_start];
    let link_id = frame_full[sig_start];
    let mut ts_arr = [0u8; 8];
    ts_arr[..6].copy_from_slice(&frame_full[sig_start + 1..sig_start + 7]);
    let timestamp = u64::from_le_bytes(ts_arr);
    let received_sig = &frame_full[sig_start + 7..];
    let expected = compute_mav_signature(secret_key_32, msg, link_id, timestamp);
    received_sig == expected
}

/// Extract (link_id, timestamp) from a signed frame without verifying signature.
pub fn extract_link_and_timestamp(frame_full: &[u8]) -> Option<(u8, u64)> {
    if frame_full.len() < 12 + 13 { return None; }
    let sig_start = frame_full.len() - 13;
    let link_id = frame_full[sig_start];
    let mut ts_arr = [0u8; 8];
    ts_arr[..6].copy_from_slice(&frame_full[sig_start + 1..sig_start + 7]);
    let timestamp = u64::from_le_bytes(ts_arr);
    Some((link_id, timestamp))
}

// ─── Replay protection per MAVLink v2 signing spec ──────────────────────────
//
// The spec requires: for each (link_id), the receiver tracks the highest timestamp
// seen and REJECTS frames whose timestamp ≤ that highwater. This prevents replay
// of captured signed frames.
//
// Timestamps are 48-bit, 10-µs ticks since 2015-01-01 00:00:00 UTC. Monotonically
// increasing by design on the sender's clock. Per-link_id isolation lets multiple
// senders (e.g. phone + drone + ground station) all send without stepping on each
// other's timestamp sequence.

/// Per-link_id replay protection state. One instance tracks all peer link IDs.
/// Optional allowlist: if set, rejects frames from link_ids not in the list.
pub struct ReplayState {
    /// 256 possible link_ids × u64 highwater. Init to 0 = accept first frame.
    /// 2 KB total footprint; fine for any deployment.
    highwater: [u64; 256],
    /// Optional allowlist bitmap: if `allowed_mask[link_id/64] & (1 << link_id%64) == 0` → reject.
    /// `None` disables allowlist (accept any link_id).
    allowed_mask: Option<[u64; 4]>,
}

impl ReplayState {
    pub const fn new() -> Self {
        Self { highwater: [0u64; 256], allowed_mask: None }
    }

    /// Restrict accepted link_ids to the given list. Calling with `&[]` disables
    /// the allowlist (accept all). Idempotent; replaces previous allowlist.
    pub fn set_allowlist(&mut self, allowed: &[u8]) {
        if allowed.is_empty() {
            self.allowed_mask = None;
            return;
        }
        let mut mask = [0u64; 4];
        for &id in allowed {
            mask[(id as usize) / 64] |= 1u64 << ((id as usize) % 64);
        }
        self.allowed_mask = Some(mask);
    }

    /// Returns true if link_id is permitted (no allowlist set = always true).
    #[inline]
    pub fn is_link_allowed(&self, link_id: u8) -> bool {
        match self.allowed_mask {
            None => true,
            Some(mask) => (mask[(link_id as usize) / 64] >> ((link_id as usize) % 64)) & 1 != 0,
        }
    }

    /// Check + update: returns `true` if timestamp is ACCEPTABLE (link allowed AND
    /// strictly > stored highwater), `false` if it's a replay or disallowed link.
    /// On accept, updates internal highwater.
    pub fn check_and_update(&mut self, link_id: u8, timestamp: u64) -> bool {
        if !self.is_link_allowed(link_id) { return false; }
        let idx = link_id as usize;
        if timestamp > self.highwater[idx] {
            self.highwater[idx] = timestamp;
            true
        } else {
            false
        }
    }

    /// Read-only query: is this timestamp fresh for this link_id?
    pub fn is_fresh(&self, link_id: u8, timestamp: u64) -> bool {
        self.is_link_allowed(link_id) && timestamp > self.highwater[link_id as usize]
    }

    // ─── Persistence (save/load across restarts) ──────────────────────────
    //
    // Format: 8B magic "OASISRS1" + 4×u64 allowed_mask (or all-zero+flag) + 256×u64 highwater.
    // Total 8 + 32 + 2048 = 2088 bytes.

    const RS_MAGIC: &'static [u8] = b"OASISRS1";

    pub fn save(&self, path: &str) -> Result<(), &'static str> {
        let mut buf = Vec::with_capacity(Self::RS_MAGIC.len() + 1 + 32 + 2048);
        buf.extend_from_slice(Self::RS_MAGIC);
        let (flag, mask) = match self.allowed_mask {
            Some(m) => (1u8, m),
            None => (0u8, [0u64; 4]),
        };
        buf.push(flag);
        for w in &mask { buf.extend_from_slice(&w.to_le_bytes()); }
        for w in &self.highwater { buf.extend_from_slice(&w.to_le_bytes()); }
        // Atomic write: write to .tmp, then rename. Prevents torn writes from
        // mid-write crash (Windows doesn't guarantee atomic fs::write).
        let tmp = format!("{}.tmp", path);
        std::fs::write(&tmp, &buf).map_err(|_| "tmp write failed")?;
        std::fs::rename(&tmp, path).map_err(|_| "rename failed")
    }

    pub fn load(path: &str) -> Result<Self, &'static str> {
        let data = std::fs::read(path).map_err(|_| "read failed")?;
        let expected_len = Self::RS_MAGIC.len() + 1 + 32 + 2048;
        if data.len() != expected_len { return Err("bad length"); }
        if &data[..Self::RS_MAGIC.len()] != Self::RS_MAGIC { return Err("bad magic"); }
        let mut pos = Self::RS_MAGIC.len();
        let flag = data[pos]; pos += 1;
        let mut mask = [0u64; 4];
        for w in mask.iter_mut() {
            *w = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
        }
        let mut highwater = [0u64; 256];
        for w in highwater.iter_mut() {
            *w = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
        }
        Ok(Self {
            highwater,
            allowed_mask: if flag == 1 { Some(mask) } else { None },
        })
    }
}

impl Default for ReplayState {
    fn default() -> Self { Self::new() }
}

/// CRC-16/MCRF4XX (MAVLink "X.25" CRC): poly 0x1021, init 0xFFFF, refin=true, refout=true.
/// Reference: MAVLink C `crc_accumulate()` — byte-at-a-time, shift-trick form.
/// Check value for input "123456789": 0x6F91.
#[inline]
fn crc_accumulate(data: u8, crc: u16) -> u16 {
    let tmp = data ^ (crc as u8);
    let tmp = tmp ^ (tmp.wrapping_shl(4));
    let tmp16 = tmp as u16;
    (crc >> 8) ^ (tmp16 << 8) ^ (tmp16 << 3) ^ (tmp16 >> 4)
}

fn crc_over(bytes: &[u8], extra: u8) -> u16 {
    let mut crc = 0xFFFFu16;
    for &b in bytes { crc = crc_accumulate(b, crc); }
    crc = crc_accumulate(extra, crc);
    crc
}

/// MAVLink CRC_EXTRA bytes per message ID — derived from each message's field signature
/// (standard values from mavlink/message_definitions/common.xml).
/// Only the 6 messages we parse/encode are populated; others → None → CRC unchecked.
fn crc_extra_for(msgid: u32) -> Option<u8> {
    match msgid {
        0   => Some(50),   // HEARTBEAT
        1   => Some(124),  // SYS_STATUS
        11  => Some(89),   // SET_MODE
        22  => Some(220),  // PARAM_VALUE
        23  => Some(168),  // PARAM_SET
        30  => Some(39),   // ATTITUDE                    (pymavlink default)
        31  => Some(246),  // ATTITUDE_QUATERNION         (PX4 native)
        32  => Some(185),  // LOCAL_POSITION_NED          (PX4 native)
        33  => Some(104),  // GLOBAL_POSITION_INT         (pymavlink default)
        74  => Some(20),   // VFR_HUD                     (PX4 native)
        76  => Some(152),  // COMMAND_LONG
        77  => Some(143),  // COMMAND_ACK
        84  => Some(143),  // SET_POSITION_TARGET_LOCAL_NED
        132 => Some(85),   // DISTANCE_SENSOR
        _   => None,
    }
}

#[derive(Debug, Clone)]
pub struct MavHeader {
    pub magic: u8,
    pub len: u8,
    pub incompat_flags: u8,
    pub compat_flags: u8,
    pub seq: u8,
    pub sysid: u8,
    pub compid: u8,
    pub msgid: u32, // 3 bytes in wire
}

#[derive(Debug, Clone)]
pub enum MavMsg {
    Heartbeat { type_: u8, autopilot: u8, base_mode: u8, custom_mode: u32, system_status: u8 },
    Attitude { time_boot_ms: u32, roll: f32, pitch: f32, yaw: f32, rollspeed: f32, pitchspeed: f32, yawspeed: f32 },
    /// PX4 native attitude: quaternion q1..q4 (w,x,y,z) + body-rates.
    AttitudeQuaternion { time_boot_ms: u32, q1: f32, q2: f32, q3: f32, q4: f32, rollspeed: f32, pitchspeed: f32, yawspeed: f32 },
    GlobalPositionInt { time_boot_ms: u32, lat: i32, lon: i32, alt: i32, relative_alt: i32, vx: i16, vy: i16, vz: i16, hdg: u16 },
    /// PX4 native position in local NED frame (meters).
    LocalPositionNed { time_boot_ms: u32, x: f32, y: f32, z: f32, vx: f32, vy: f32, vz: f32 },
    /// Consolidated attitude + speed (common PX4 stream).
    VfrHud { airspeed: f32, groundspeed: f32, heading: i16, throttle: u16, alt: f32, climb: f32 },
    DistanceSensor { time_boot_ms: u32, min_distance: u16, max_distance: u16, current_distance: u16, type_: u8, id: u8, orientation: u8, covariance: u8 },
    /// Response to a COMMAND_LONG. Result codes: 0=ACCEPTED, 1=TEMPORARILY_REJECTED,
    /// 2=DENIED, 3=UNSUPPORTED, 4=FAILED, 5=IN_PROGRESS, 6=CANCELLED.
    CommandAck { command: u16, result: u8 },
    /// PARAM_VALUE response (after PARAM_SET or PARAM_REQUEST). Contains the param
    /// identifier (up to 16 ASCII chars) and its new value.
    ParamValue { param_id: [u8; 16], param_value: f32, param_type: u8, param_index: u16, param_count: u16 },
    SysStatus { sensors_present: u32, sensors_enabled: u32, sensors_health: u32, load: u16, voltage_battery: u16, current_battery: i16, battery_remaining: i8, drop_rate_comm: u16, errors_comm: u16, errors_count1: u16, errors_count2: u16, errors_count3: u16, errors_count4: u16 },
    Unknown { msgid: u32, payload: Vec<u8> },
}

/// Parse a signed MAVLink v2 frame WITH replay protection.
/// Requires caller to maintain a `ReplayState`. Rejects frames whose timestamp
/// is ≤ the highwater for their link_id (per MAVLink spec).
/// Unsigned frames pass through (no replay check possible) UNLESS
/// OASIS_MAVLINK_REQUIRE_SIGNED=1 is set — then unsigned frames are rejected.
pub fn parse_frame_checked(bytes: &[u8], replay: &mut ReplayState) -> Option<(MavHeader, MavMsg)> {
    if bytes.is_empty() { return None; }
    let is_signed = bytes.len() > 2 && (bytes[2] & MAVLINK_IFLAG_SIGNED) != 0;

    // Require-signed mode: reject any unsigned frame before even parsing.
    if !is_signed {
        let require: bool = std::env::var("OASIS_MAVLINK_REQUIRE_SIGNED")
            .ok().map(|s| s == "1").unwrap_or(false);
        if require { return None; }
    }

    let (h, m) = parse_frame(bytes)?;
    // Only signed frames carry replay-protection data.
    if is_signed {
        let len = bytes[1] as usize;
        let frame_end = 10 + len + 2 + 13;
        if bytes.len() >= frame_end {
            if let Some((link_id, ts)) = extract_link_and_timestamp(&bytes[..frame_end]) {
                if !replay.check_and_update(link_id, ts) {
                    return None;  // replay or disallowed link rejected
                }
            }
        }
    }
    Some((h, m))
}

/// Parse one MAVLink v2 frame. Returns (header, message) or None if invalid.
/// Verifies CRC-16/MCRF4XX for known msgids. If incompat_flags & SIGNED and
/// OASIS_MAVLINK_SECRET env var is set (64 hex chars), also verifies 13-byte
/// signature tail. Does NOT perform replay detection — use `parse_frame_checked`
/// with a ReplayState for defense-grade use.
/// Unknown msgids bypass CRC check but still return Unknown variant.
pub fn parse_frame(bytes: &[u8]) -> Option<(MavHeader, MavMsg)> {
    if bytes.len() < 12 { return None; }
    if bytes[0] != MAV_V2_MAGIC { return None; }
    let len = bytes[1] as usize;
    let incompat_flags = bytes[2];
    let signed = (incompat_flags & MAVLINK_IFLAG_SIGNED) != 0;
    let total_needed = 10 + len + 2 + if signed { 13 } else { 0 };
    if bytes.len() < total_needed { return None; }

    // If signed and we have a key, verify signature
    if signed {
        if let Some(key) = load_signing_key() {
            if !verify_signature(&bytes[..total_needed], &key) {
                return None;
            }
        }
    }

    let header = MavHeader {
        magic: bytes[0],
        len: bytes[1],
        incompat_flags: bytes[2],
        compat_flags: bytes[3],
        seq: bytes[4],
        sysid: bytes[5],
        compid: bytes[6],
        msgid: u32::from_le_bytes([bytes[7], bytes[8], bytes[9], 0]),
    };

    // Verify CRC-16/MCRF4XX if we know the CRC_EXTRA byte for this msgid.
    if let Some(extra) = crc_extra_for(header.msgid) {
        let crc_region = &bytes[1..10 + len];
        let computed = crc_over(crc_region, extra);
        let frame_crc = u16::from_le_bytes([bytes[10 + len], bytes[10 + len + 1]]);
        if computed != frame_crc { return None; }
    }

    let payload = &bytes[10..10 + len];
    let msg = decode_payload(header.msgid, payload);
    Some((header, msg))
}

/// Load signing key from OASIS_MAVLINK_SECRET env var (64 hex chars = 32 bytes).
/// Returns None if unset or malformed.
fn load_signing_key() -> Option<[u8; 32]> {
    let hex = std::env::var("OASIS_MAVLINK_SECRET").ok()?;
    if hex.len() != 64 { return None; }
    let mut key = [0u8; 32];
    for i in 0..32 {
        let hi = hex_digit(hex.as_bytes()[i * 2])?;
        let lo = hex_digit(hex.as_bytes()[i * 2 + 1])?;
        key[i] = (hi << 4) | lo;
    }
    Some(key)
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Encode a COMMAND_LONG frame (msgid 76, payload 33 B).
/// Used to send commands like ARM, TAKEOFF, DISARM, RETURN_TO_LAUNCH, etc.
/// `command` = MAV_CMD constant (400 = COMPONENT_ARM_DISARM).
/// `param1` = typically 1.0 to arm, 0.0 to disarm.
pub fn encode_command_long(
    seq: u8, sysid: u8, compid: u8,
    target_sys: u8, target_comp: u8,
    command: u16, confirmation: u8,
    param1: f32, param2: f32, param3: f32,
    param4: f32, param5: f32, param6: f32, param7: f32,
) -> Vec<u8> {
    let mut payload = vec![0u8; 33];
    payload[0..4].copy_from_slice(&param1.to_le_bytes());
    payload[4..8].copy_from_slice(&param2.to_le_bytes());
    payload[8..12].copy_from_slice(&param3.to_le_bytes());
    payload[12..16].copy_from_slice(&param4.to_le_bytes());
    payload[16..20].copy_from_slice(&param5.to_le_bytes());
    payload[20..24].copy_from_slice(&param6.to_le_bytes());
    payload[24..28].copy_from_slice(&param7.to_le_bytes());
    payload[28..30].copy_from_slice(&command.to_le_bytes());
    payload[30] = target_sys;
    payload[31] = target_comp;
    payload[32] = confirmation;

    let msgid: u32 = 76;
    let mut frame = Vec::with_capacity(12 + 33);
    frame.push(MAV_V2_MAGIC);
    frame.push(33);
    frame.push(0); frame.push(0);
    frame.push(seq); frame.push(sysid); frame.push(compid);
    frame.push((msgid & 0xFF) as u8);
    frame.push(((msgid >> 8) & 0xFF) as u8);
    frame.push(((msgid >> 16) & 0xFF) as u8);
    frame.extend_from_slice(&payload);
    let extra = crc_extra_for(msgid).unwrap();
    let crc = crc_over(&frame[1..], extra);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Encode a SET_MODE frame (msgid 11, payload 6 B).
/// For PX4 OFFBOARD: base_mode=0x01 (MAV_MODE_FLAG_CUSTOM_MODE_ENABLED),
///                   custom_mode=0x00060000 (main_mode=6 = OFFBOARD).
pub fn encode_set_mode(seq: u8, sysid: u8, compid: u8, target_sys: u8, base_mode: u8, custom_mode: u32) -> Vec<u8> {
    let mut payload = vec![0u8; 6];
    payload[0..4].copy_from_slice(&custom_mode.to_le_bytes());
    payload[4] = target_sys;
    payload[5] = base_mode;

    let msgid: u32 = 11;
    let mut frame = Vec::with_capacity(12 + 6);
    frame.push(MAV_V2_MAGIC);
    frame.push(6);
    frame.push(0); frame.push(0);
    frame.push(seq); frame.push(sysid); frame.push(compid);
    frame.push((msgid & 0xFF) as u8);
    frame.push(((msgid >> 8) & 0xFF) as u8);
    frame.push(((msgid >> 16) & 0xFF) as u8);
    frame.extend_from_slice(&payload);
    let extra = crc_extra_for(msgid).unwrap();
    let crc = crc_over(&frame[1..], extra);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Encode a PARAM_SET frame (msgid 23, payload 23 B) to update a PX4 parameter.
/// `param_id` must be ≤ 16 ASCII bytes (e.g. "MPC_XY_VEL_MAX"). Type 9 = MAV_PARAM_TYPE_REAL32.
/// PX4 will respond with PARAM_VALUE (msgid 22) confirming the new value.
pub fn encode_param_set(seq: u8, sysid: u8, compid: u8, target_sys: u8, target_comp: u8, param_id: &str, param_value: f32) -> Vec<u8> {
    let mut payload = vec![0u8; 23];
    payload[0..4].copy_from_slice(&param_value.to_le_bytes());
    payload[4] = target_sys;
    payload[5] = target_comp;
    // param_id: 16 bytes, null-padded ASCII
    let id_bytes = param_id.as_bytes();
    let n = id_bytes.len().min(16);
    payload[6..6 + n].copy_from_slice(&id_bytes[..n]);
    payload[22] = 9;  // MAV_PARAM_TYPE_REAL32

    let msgid: u32 = 23;
    let mut frame = Vec::with_capacity(12 + 23);
    frame.push(MAV_V2_MAGIC);
    frame.push(23);
    frame.push(0); frame.push(0);
    frame.push(seq); frame.push(sysid); frame.push(compid);
    frame.push((msgid & 0xFF) as u8);
    frame.push(((msgid >> 8) & 0xFF) as u8);
    frame.push(((msgid >> 16) & 0xFF) as u8);
    frame.extend_from_slice(&payload);
    let extra = crc_extra_for(msgid).expect("PARAM_SET has CRC_EXTRA");
    let crc = crc_over(&frame[1..], extra);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Encode a HEARTBEAT frame. Required to keep PX4/GCS from flagging the link as offline.
/// Sends every 1 second in typical deployments.
pub fn encode_heartbeat(seq: u8, sysid: u8, compid: u8) -> Vec<u8> {
    let mut payload = vec![0u8; 9];
    // custom_mode (u32) = 0
    payload[4] = 2;   // type = MAV_TYPE_QUADROTOR
    payload[5] = 12;  // autopilot = MAV_AUTOPILOT_PX4
    payload[6] = 0;   // base_mode
    payload[7] = 4;   // system_status = MAV_STATE_ACTIVE
    payload[8] = 3;   // mavlink_version

    let msgid: u32 = 0;
    let mut frame = Vec::with_capacity(12 + 9);
    frame.push(MAV_V2_MAGIC);
    frame.push(9);
    frame.push(0); frame.push(0); // incompat, compat
    frame.push(seq); frame.push(sysid); frame.push(compid);
    frame.push((msgid & 0xFF) as u8);
    frame.push(((msgid >> 8) & 0xFF) as u8);
    frame.push(((msgid >> 16) & 0xFF) as u8);
    frame.extend_from_slice(&payload);
    let extra = crc_extra_for(msgid).unwrap();
    let crc = crc_over(&frame[1..], extra);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Zero-pad a payload up to expected length. MAVLink v2 truncates trailing zero bytes
/// on the wire; receivers must restore them before decoding.
fn pad_payload(p: &[u8], target_len: usize) -> std::borrow::Cow<'_, [u8]> {
    if p.len() >= target_len {
        std::borrow::Cow::Borrowed(&p[..target_len])
    } else {
        let mut padded = p.to_vec();
        padded.resize(target_len, 0);
        std::borrow::Cow::Owned(padded)
    }
}

fn decode_payload(msgid: u32, p: &[u8]) -> MavMsg {
    match msgid {
        0 => { // HEARTBEAT (9 bytes)
            let p = pad_payload(p, 9);
            let custom_mode = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
            MavMsg::Heartbeat {
                custom_mode,
                type_: p[4], autopilot: p[5], base_mode: p[6],
                system_status: p[7],
            }
        }
        30 => { // ATTITUDE (28 bytes)
            let p = pad_payload(p, 28);
            MavMsg::Attitude {
                time_boot_ms: u32::from_le_bytes(p[0..4].try_into().unwrap()),
                roll: f32::from_le_bytes(p[4..8].try_into().unwrap()),
                pitch: f32::from_le_bytes(p[8..12].try_into().unwrap()),
                yaw: f32::from_le_bytes(p[12..16].try_into().unwrap()),
                rollspeed: f32::from_le_bytes(p[16..20].try_into().unwrap()),
                pitchspeed: f32::from_le_bytes(p[20..24].try_into().unwrap()),
                yawspeed: f32::from_le_bytes(p[24..28].try_into().unwrap()),
            }
        }
        33 => { // GLOBAL_POSITION_INT (28 bytes)
            let p = pad_payload(p, 28);
            MavMsg::GlobalPositionInt {
                time_boot_ms: u32::from_le_bytes(p[0..4].try_into().unwrap()),
                lat: i32::from_le_bytes(p[4..8].try_into().unwrap()),
                lon: i32::from_le_bytes(p[8..12].try_into().unwrap()),
                alt: i32::from_le_bytes(p[12..16].try_into().unwrap()),
                relative_alt: i32::from_le_bytes(p[16..20].try_into().unwrap()),
                vx: i16::from_le_bytes(p[20..22].try_into().unwrap()),
                vy: i16::from_le_bytes(p[22..24].try_into().unwrap()),
                vz: i16::from_le_bytes(p[24..26].try_into().unwrap()),
                hdg: u16::from_le_bytes(p[26..28].try_into().unwrap()),
            }
        }
        31 => { // ATTITUDE_QUATERNION (48 bytes; last 16 B are optional repr_offset_q, truncated often)
            let p = pad_payload(p, 32); // need only first 32 B for core fields
            MavMsg::AttitudeQuaternion {
                time_boot_ms: u32::from_le_bytes(p[0..4].try_into().unwrap()),
                q1: f32::from_le_bytes(p[4..8].try_into().unwrap()),
                q2: f32::from_le_bytes(p[8..12].try_into().unwrap()),
                q3: f32::from_le_bytes(p[12..16].try_into().unwrap()),
                q4: f32::from_le_bytes(p[16..20].try_into().unwrap()),
                rollspeed: f32::from_le_bytes(p[20..24].try_into().unwrap()),
                pitchspeed: f32::from_le_bytes(p[24..28].try_into().unwrap()),
                yawspeed: f32::from_le_bytes(p[28..32].try_into().unwrap()),
            }
        }
        32 => { // LOCAL_POSITION_NED (28 bytes)
            let p = pad_payload(p, 28);
            MavMsg::LocalPositionNed {
                time_boot_ms: u32::from_le_bytes(p[0..4].try_into().unwrap()),
                x: f32::from_le_bytes(p[4..8].try_into().unwrap()),
                y: f32::from_le_bytes(p[8..12].try_into().unwrap()),
                z: f32::from_le_bytes(p[12..16].try_into().unwrap()),
                vx: f32::from_le_bytes(p[16..20].try_into().unwrap()),
                vy: f32::from_le_bytes(p[20..24].try_into().unwrap()),
                vz: f32::from_le_bytes(p[24..28].try_into().unwrap()),
            }
        }
        74 => { // VFR_HUD (20 bytes)
            let p = pad_payload(p, 20);
            MavMsg::VfrHud {
                airspeed: f32::from_le_bytes(p[0..4].try_into().unwrap()),
                groundspeed: f32::from_le_bytes(p[4..8].try_into().unwrap()),
                alt: f32::from_le_bytes(p[8..12].try_into().unwrap()),
                climb: f32::from_le_bytes(p[12..16].try_into().unwrap()),
                heading: i16::from_le_bytes(p[16..18].try_into().unwrap()),
                throttle: u16::from_le_bytes(p[18..20].try_into().unwrap()),
            }
        }
        77 => { // COMMAND_ACK (3 bytes minimal — extensions in v2 can add more)
            let p = pad_payload(p, 3);
            MavMsg::CommandAck {
                command: u16::from_le_bytes(p[0..2].try_into().unwrap()),
                result: p[2],
            }
        }
        22 => { // PARAM_VALUE (25 bytes)
            let p = pad_payload(p, 25);
            let mut param_id = [0u8; 16];
            param_id.copy_from_slice(&p[8..24]);
            MavMsg::ParamValue {
                param_value: f32::from_le_bytes(p[0..4].try_into().unwrap()),
                param_count: u16::from_le_bytes(p[4..6].try_into().unwrap()),
                param_index: u16::from_le_bytes(p[6..8].try_into().unwrap()),
                param_id,
                param_type: p[24],
            }
        }
        132 => { // DISTANCE_SENSOR (14 bytes)
            let p = pad_payload(p, 14);
            MavMsg::DistanceSensor {
                time_boot_ms: u32::from_le_bytes(p[0..4].try_into().unwrap()),
                min_distance: u16::from_le_bytes(p[4..6].try_into().unwrap()),
                max_distance: u16::from_le_bytes(p[6..8].try_into().unwrap()),
                current_distance: u16::from_le_bytes(p[8..10].try_into().unwrap()),
                type_: p[10], id: p[11], orientation: p[12], covariance: p[13],
            }
        }
        1 => { // SYS_STATUS (full 31 bytes; we read first 12 for health)
            let p = pad_payload(p, 12);
            MavMsg::SysStatus {
                sensors_present: u32::from_le_bytes(p[0..4].try_into().unwrap()),
                sensors_enabled: u32::from_le_bytes(p[4..8].try_into().unwrap()),
                sensors_health: u32::from_le_bytes(p[8..12].try_into().unwrap()),
                load: 0, voltage_battery: 0, current_battery: 0,
                battery_remaining: 0, drop_rate_comm: 0, errors_comm: 0,
                errors_count1: 0, errors_count2: 0, errors_count3: 0, errors_count4: 0,
            }
        }
        _ => MavMsg::Unknown { msgid, payload: p.to_vec() },
    }
}

/// Encode a SET_POSITION_TARGET_LOCAL_NED (#84) command frame with valid CRC-16/MCRF4XX.
/// Returns bytes ready to send over UDP. PX4/ArduPilot will accept this frame.
pub fn encode_set_position_target(seq: u8, sysid: u8, compid: u8, target_sys: u8, target_comp: u8, vx: f32, vy: f32, vz: f32) -> Vec<u8> {
    // Payload is 53 bytes; we fill velocity fields and set type_mask to velocity-only
    let mut payload = vec![0u8; 53];
    payload[16..20].copy_from_slice(&vx.to_le_bytes());
    payload[20..24].copy_from_slice(&vy.to_le_bytes());
    payload[24..28].copy_from_slice(&vz.to_le_bytes());
    // type_mask (u16) at offset 48: bits for ignore_pos+ignore_accel+ignore_yaw = 0x01C7
    payload[48..50].copy_from_slice(&0x01C7u16.to_le_bytes());
    payload[50] = target_sys;
    payload[51] = target_comp;
    payload[52] = 1; // coordinate frame MAV_FRAME_LOCAL_NED

    let msgid: u32 = 84;
    let mut frame = Vec::with_capacity(12 + 53);
    frame.push(MAV_V2_MAGIC);
    frame.push(53u8);
    frame.push(0); frame.push(0); // incompat, compat
    frame.push(seq); frame.push(sysid); frame.push(compid);
    frame.push((msgid & 0xFF) as u8);
    frame.push(((msgid >> 8) & 0xFF) as u8);
    frame.push(((msgid >> 16) & 0xFF) as u8);
    frame.extend_from_slice(&payload);

    // Compute CRC over bytes[1..10+53] = len + incompat + compat + seq + sysid + compid + msgid(3) + payload
    // then accumulate CRC_EXTRA for msgid 84 = 143.
    let extra = crc_extra_for(msgid).expect("msgid 84 has CRC_EXTRA");
    let crc = crc_over(&frame[1..], extra);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Small helper exposed for test frame builders: compute CRC for a fully-built frame
/// (except for its 2 trailing CRC bytes) + return as [lo, hi].
pub fn compute_frame_crc(header_plus_payload: &[u8], msgid: u32) -> [u8; 2] {
    let extra = crc_extra_for(msgid).unwrap_or(0);
    let crc = crc_over(&header_plus_payload[1..], extra); // skip magic byte
    [(crc & 0xFF) as u8, (crc >> 8) as u8]
}

/// Encode a SET_POSITION_TARGET_LOCAL_NED frame with POSITION setpoint (x, y, z in NED meters).
/// type_mask = 0x0DF8 (ignore velocity, acceleration, yaw, yaw_rate). PX4 will drive to this
/// position using its internal position controller (handles accel/damping).
///
/// NED: z positive = DOWN from origin. For target altitude +alt (above origin), pass z = -alt.
/// Example: drone should hover at (3, 5) in xy + altitude 2m → pass x=3.0, y=5.0, z=-2.0.
pub fn encode_position_setpoint(seq: u8, sysid: u8, compid: u8, target_sys: u8, target_comp: u8, x: f32, y: f32, z: f32) -> Vec<u8> {
    let mut payload = vec![0u8; 53];
    // time_boot_ms (0..4) = 0
    payload[4..8].copy_from_slice(&x.to_le_bytes());
    payload[8..12].copy_from_slice(&y.to_le_bytes());
    payload[12..16].copy_from_slice(&z.to_le_bytes());
    // velocity (16..28), accel (28..40), yaw+yaw_rate (40..48) all zero
    // type_mask (48..50): ignore vel (0x0008|0x0010|0x0020), accel (0x0040|0x0080|0x0100),
    // yaw (0x0400), yaw_rate (0x0800) → 0x0DF8. USE position x,y,z.
    payload[48..50].copy_from_slice(&0x0DF8u16.to_le_bytes());
    payload[50] = target_sys;
    payload[51] = target_comp;
    payload[52] = 1;  // MAV_FRAME_LOCAL_NED

    let msgid: u32 = 84;
    let mut frame = Vec::with_capacity(12 + 53);
    frame.push(MAV_V2_MAGIC);
    frame.push(53);
    frame.push(0); frame.push(0);
    frame.push(seq); frame.push(sysid); frame.push(compid);
    frame.push((msgid & 0xFF) as u8);
    frame.push(((msgid >> 8) & 0xFF) as u8);
    frame.push(((msgid >> 16) & 0xFF) as u8);
    frame.extend_from_slice(&payload);
    let extra = crc_extra_for(msgid).expect("msgid 84 has CRC_EXTRA");
    let crc = crc_over(&frame[1..], extra);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Translate MAVLink messages into OASIS JSON tick input line.
/// Accumulates partial state and emits a full line when position/attitude are both fresh.
pub struct MavToOasisAccumulator {
    pub tick: u32,
    pub x: f64, pub y: f64, pub alt: f64,
    pub roll: f64, pub pitch: f64, pub gz: f64,
    pub rf: f64, pub rl: f64, pub rr: f64, pub rb: f64,
    pub vx: f64, pub vy: f64,
    pub imu_alive: bool, pub gps_alive: bool, pub sonar_alive: bool,
    has_attitude: bool, has_position: bool,
    /// Once LOCAL_POSITION_NED arrives, we lock position source to local frame.
    /// GLOBAL_POSITION_INT (lat/lon in 1e7 degrees) would otherwise overwrite with
    /// values in the wrong scale. PX4 streams both; local takes priority.
    local_position_seen: bool,
}

impl Default for MavToOasisAccumulator {
    fn default() -> Self {
        Self {
            tick: 0, x: 0.0, y: 0.0, alt: 1.3, roll: 0.0, pitch: 0.0, gz: 0.0,
            rf: 2.0, rl: 2.0, rr: 2.0, rb: 2.0, vx: 0.0, vy: 0.0,
            imu_alive: true, gps_alive: true, sonar_alive: true,
            has_attitude: false, has_position: false,
            local_position_seen: false,
        }
    }
}

impl MavToOasisAccumulator {
    pub fn apply(&mut self, msg: &MavMsg) {
        match msg {
            MavMsg::Attitude { roll, pitch, yawspeed, .. } => {
                self.roll = *roll as f64;
                self.pitch = *pitch as f64;
                self.gz = *yawspeed as f64;
                self.has_attitude = true;
            }
            // PX4 native: convert quaternion (w,x,y,z) → roll/pitch via ZYX Euler.
            MavMsg::AttitudeQuaternion { q1, q2, q3, q4, yawspeed, .. } => {
                // q1=w q2=x q3=y q4=z (MAVLink convention)
                let (w, x, y, z) = (*q1 as f64, *q2 as f64, *q3 as f64, *q4 as f64);
                // roll = atan2(2(wx+yz), 1-2(x²+y²))
                let sinr = 2.0 * (w * x + y * z);
                let cosr = 1.0 - 2.0 * (x * x + y * y);
                self.roll = sinr.atan2(cosr);
                // pitch = asin(2(wy-zx))  (clamp for numerical safety)
                let sinp = 2.0 * (w * y - z * x);
                self.pitch = if sinp.abs() >= 1.0 {
                    std::f64::consts::FRAC_PI_2.copysign(sinp)
                } else { sinp.asin() };
                self.gz = *yawspeed as f64;
                self.has_attitude = true;
            }
            // VFR_HUD provides a consolidated altitude + speed; keeps accumulator fresh
            // even when explicit position isn't streamed.
            // PX4 sends VFR_HUD.alt = MSL (e.g., 491m at Zurich home). If we have already
            // locked onto LOCAL_POSITION_NED, do NOT overwrite alt — it would re-introduce
            // the MSL leak that breaks the altitude controller.
            MavMsg::VfrHud { alt, climb, groundspeed, .. } => {
                if !self.local_position_seen {
                    self.alt = *alt as f64;
                }
                if self.vx.abs() < 1e-6 && self.vy.abs() < 1e-6 {
                    self.vx = *groundspeed as f64;
                }
                let _ = climb;
            }
            MavMsg::GlobalPositionInt { lat, lon, alt, vx, vy, .. } => {
                // Skip if LocalPositionNed has been seen — local frame is authoritative.
                // GLOBAL_POSITION_INT is in lat/lon*1e7 degrees which is meaningless as
                // local x,y (would make drone think it's at 47.40 m instead of 1.0 m).
                if !self.local_position_seen {
                    self.x = *lat as f64 / 1.0e7;
                    self.y = *lon as f64 / 1.0e7;
                    self.alt = *alt as f64 / 1000.0;
                    self.vx = *vx as f64 / 100.0;
                    self.vy = *vy as f64 / 100.0;
                    self.has_position = true;
                }
            }
            // PX4 native: local NED frame, already in meters & m/s.
            MavMsg::LocalPositionNed { x, y, z, vx, vy, .. } => {
                self.x = *x as f64;
                self.y = *y as f64;
                self.alt = -(*z as f64);  // NED z-down → OASIS altitude (z-up)
                self.vx = *vx as f64;
                self.vy = *vy as f64;
                self.has_position = true;
                self.local_position_seen = true;  // lock position source to local
            }
            MavMsg::DistanceSensor { current_distance, orientation, .. } => {
                let m = *current_distance as f64 / 100.0;
                match orientation {
                    0 => self.rf = m,
                    2 => self.rr = m,
                    4 => self.rb = m,
                    6 => self.rl = m,
                    _ => {}
                }
            }
            MavMsg::SysStatus { sensors_health, .. } => {
                const SENSOR_3D_GYRO: u32 = 0x01;
                const SENSOR_GPS: u32 = 0x20;
                const SENSOR_LASER: u32 = 0x8000;
                self.imu_alive = sensors_health & SENSOR_3D_GYRO != 0;
                self.gps_alive = sensors_health & SENSOR_GPS != 0;
                self.sonar_alive = sensors_health & SENSOR_LASER != 0;
            }
            _ => {}
        }
    }

    pub fn ready_to_emit(&self) -> bool { self.has_attitude && self.has_position }

    pub fn to_oasis_json(&mut self) -> String {
        self.tick += 1;
        let s = format!(
            "{{\"tick\":{},\"x\":{:.4},\"y\":{:.4},\"alt\":{:.4},\"roll\":{:.4},\"pitch\":{:.4},\"gz\":{:.4},\"rf\":{:.4},\"rl\":{:.4},\"rr\":{:.4},\"rb\":{:.4},\"vx\":{:.4},\"vy\":{:.4},\"imu_alive\":{},\"gps_alive\":{},\"sonar_alive\":{}}}",
            self.tick, self.x, self.y, self.alt, self.roll, self.pitch, self.gz,
            self.rf, self.rl, self.rr, self.rb, self.vx, self.vy,
            self.imu_alive, self.gps_alive, self.sonar_alive
        );
        // Reset fresh-state flags; keep values (rate-limiting to one emission per matched pair)
        self.has_attitude = false;
        self.has_position = false;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // Helper: build a well-formed frame with correct CRC.
    fn build_frame(msgid: u32, seq: u8, payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.push(0xFD);
        frame.push(payload.len() as u8);
        frame.push(0); frame.push(0);
        frame.push(seq); frame.push(1); frame.push(1);
        frame.push((msgid & 0xFF) as u8);
        frame.push(((msgid >> 8) & 0xFF) as u8);
        frame.push(((msgid >> 16) & 0xFF) as u8);
        frame.extend_from_slice(payload);
        let crc_bytes = compute_frame_crc(&frame, msgid);
        frame.extend_from_slice(&crc_bytes);
        frame
    }

    #[test]
    fn parse_attitude_frame() {
        let mut payload = [0u8; 28];
        payload[0..4].copy_from_slice(&1234u32.to_le_bytes());
        payload[4..8].copy_from_slice(&0.1_f32.to_le_bytes());
        payload[8..12].copy_from_slice(&0.2_f32.to_le_bytes());
        payload[12..16].copy_from_slice(&0.3_f32.to_le_bytes());
        payload[16..20].copy_from_slice(&0.0_f32.to_le_bytes());
        payload[20..24].copy_from_slice(&0.0_f32.to_le_bytes());
        payload[24..28].copy_from_slice(&0.5_f32.to_le_bytes());
        let frame = build_frame(30, 1, &payload);

        let (h, m) = parse_frame(&frame).expect("parse ATTITUDE with valid CRC");
        assert_eq!(h.msgid, 30);
        match m {
            MavMsg::Attitude { roll, pitch, yawspeed, .. } => {
                assert!((roll - 0.1).abs() < 1e-5);
                assert!((pitch - 0.2).abs() < 1e-5);
                assert!((yawspeed - 0.5).abs() < 1e-5);
            }
            _ => panic!("expected Attitude"),
        }
    }

    #[test]
    fn parse_rejects_bad_crc() {
        let payload = [0u8; 28];
        let mut frame = build_frame(30, 2, &payload);
        // Tamper with CRC low byte
        let last = frame.len() - 2;
        frame[last] ^= 0xFF;
        assert!(parse_frame(&frame).is_none(), "frame with corrupted CRC must be rejected");
    }

    #[test]
    fn crc_known_vector() {
        // CRC-16/MCRF4XX check vector for "123456789" (ASCII) with no extra = 0x6F91.
        // With extra=0: accumulate 9 bytes of "123456789", then 1 zero byte.
        let mut crc = 0xFFFFu16;
        for b in b"123456789" { crc = crc_accumulate(*b, crc); }
        // NOTE: standard MCRF4XX check is over "123456789" only (no extra); add extra=0 gives different value.
        // Here we just verify the 9-byte intermediate is stable/documented.
        // Standard library check vector is 0x6F91 for the 9 input bytes alone.
        assert_eq!(crc, 0x6F91, "CRC-16/MCRF4XX check vector");
    }

    #[test]
    fn encode_decode_roundtrip_set_position() {
        // A frame built by encode_set_position_target must parse back cleanly.
        let frame = encode_set_position_target(7, 1, 1, 1, 1, 2.0, -1.5, 0.0);
        let (h, _m) = parse_frame(&frame).expect("self-encoded frame must parse");
        assert_eq!(h.msgid, 84);
        assert_eq!(h.seq, 7);
    }

    #[test]
    fn encode_position_setpoint_roundtrip() {
        // Position setpoint at (3, 5, -2) — drone should fly to (3,5) at altitude 2m
        let frame = encode_position_setpoint(11, 1, 1, 1, 1, 3.0, 5.0, -2.0);
        let (h, _) = parse_frame(&frame).expect("position setpoint must self-parse");
        assert_eq!(h.msgid, 84);
        assert_eq!(h.seq, 11);
        // Verify position fields at payload offset 4..16 (frame offset 14..26)
        let x = f32::from_le_bytes(frame[14..18].try_into().unwrap());
        let y = f32::from_le_bytes(frame[18..22].try_into().unwrap());
        let z = f32::from_le_bytes(frame[22..26].try_into().unwrap());
        assert!((x - 3.0).abs() < 1e-5);
        assert!((y - 5.0).abs() < 1e-5);
        assert!((z - (-2.0)).abs() < 1e-5);
        // type_mask at payload offset 48 (frame offset 58)
        let mask = u16::from_le_bytes([frame[58], frame[59]]);
        assert_eq!(mask, 0x0DF8, "position-only mask must ignore vel+accel+yaw+yaw_rate");
    }

    #[test]
    fn heartbeat_roundtrip() {
        let frame = encode_heartbeat(42, 1, 1);
        let (h, m) = parse_frame(&frame).expect("heartbeat must parse");
        assert_eq!(h.msgid, 0);
        assert_eq!(h.seq, 42);
        match m {
            MavMsg::Heartbeat { type_, autopilot, system_status, .. } => {
                assert_eq!(type_, 2); assert_eq!(autopilot, 12); assert_eq!(system_status, 4);
            }
            _ => panic!("expected Heartbeat"),
        }
    }

    #[test]
    fn signing_roundtrip() {
        let key: [u8; 32] = [42u8; 32];
        let frame = encode_set_position_target(1, 1, 1, 1, 1, 1.0, 2.0, 0.0);
        // Set signed flag
        let mut signed_frame = frame.clone();
        signed_frame[2] |= MAVLINK_IFLAG_SIGNED;
        // Recompute CRC because incompat_flags changed
        let payload_len = signed_frame[1] as usize;
        let crc = crc_over(&signed_frame[1..10 + payload_len], crc_extra_for(84).unwrap());
        let crc_idx = 10 + payload_len;
        signed_frame[crc_idx] = (crc & 0xFF) as u8;
        signed_frame[crc_idx + 1] = (crc >> 8) as u8;
        let full = sign_frame(signed_frame, &key, 1, 1_000_000_000);
        assert!(verify_signature(&full, &key), "self-signed frame must verify");
        // Tamper
        let mut bad = full.clone();
        let idx = bad.len() / 2;
        bad[idx] ^= 0xFF;
        assert!(!verify_signature(&bad, &key), "tampered signed frame must reject");
    }

    /// Build a valid signed frame with given (link_id, timestamp) for replay tests.
    fn build_signed_frame(link_id: u8, ts: u64, sysid: u8, seq: u8, key: &[u8; 32]) -> Vec<u8> {
        let mut payload = [0u8; 28];
        payload[4..8].copy_from_slice(&0.1f32.to_le_bytes());
        let mut frame = Vec::new();
        frame.push(0xFD); frame.push(28); frame.push(MAVLINK_IFLAG_SIGNED); frame.push(0);
        frame.push(seq); frame.push(sysid); frame.push(1);
        frame.push(30); frame.push(0); frame.push(0);
        frame.extend_from_slice(&payload);
        let crc = crc_over(&frame[1..], crc_extra_for(30).unwrap());
        frame.push((crc & 0xFF) as u8);
        frame.push((crc >> 8) as u8);
        sign_frame(frame, key, link_id, ts)
    }

    #[test]
    #[serial]
    fn replay_protection_accepts_fresh() {
        let hex_key = "42".repeat(32);
        std::env::set_var("OASIS_MAVLINK_SECRET", &hex_key);
        let key = [0x42u8; 32];
        let mut state = ReplayState::new();
        let f1 = build_signed_frame(1, 100, 1, 1, &key);
        assert!(parse_frame_checked(&f1, &mut state).is_some(), "first frame must be accepted");
        let f2 = build_signed_frame(1, 200, 1, 2, &key);
        assert!(parse_frame_checked(&f2, &mut state).is_some(), "later timestamp must accept");
        std::env::remove_var("OASIS_MAVLINK_SECRET");
    }

    #[test]
    #[serial]
    fn replay_protection_rejects_replay() {
        let hex_key = "55".repeat(32);
        std::env::set_var("OASIS_MAVLINK_SECRET", &hex_key);
        let key = [0x55u8; 32];
        let mut state = ReplayState::new();
        let f1 = build_signed_frame(1, 500, 1, 1, &key);
        assert!(parse_frame_checked(&f1, &mut state).is_some());
        // Replay same timestamp → reject
        let f1_replay = build_signed_frame(1, 500, 1, 1, &key);
        assert!(parse_frame_checked(&f1_replay, &mut state).is_none(), "replay of same ts must reject");
        // Older timestamp → reject
        let f_old = build_signed_frame(1, 400, 1, 2, &key);
        assert!(parse_frame_checked(&f_old, &mut state).is_none(), "older ts must reject");
        std::env::remove_var("OASIS_MAVLINK_SECRET");
    }

    #[test]
    #[serial]
    fn replay_protection_isolates_link_ids() {
        // Per-link_id isolation: link 1's timestamp doesn't affect link 2.
        let hex_key = "77".repeat(32);
        std::env::set_var("OASIS_MAVLINK_SECRET", &hex_key);
        let key = [0x77u8; 32];
        let mut state = ReplayState::new();
        let a = build_signed_frame(1, 1000, 1, 1, &key);
        assert!(parse_frame_checked(&a, &mut state).is_some());
        // Link 2 sends an earlier timestamp — that's fine, it's a different link.
        let b = build_signed_frame(2, 500, 1, 1, &key);
        assert!(parse_frame_checked(&b, &mut state).is_some(),
                "different link_id must not be affected by link 1 highwater");
        std::env::remove_var("OASIS_MAVLINK_SECRET");
    }

    #[test]
    fn allowlist_rejects_disallowed_link() {
        let mut state = ReplayState::new();
        state.set_allowlist(&[1, 2, 3]);
        assert!(state.is_link_allowed(1));
        assert!(state.is_link_allowed(2));
        assert!(state.is_link_allowed(3));
        assert!(!state.is_link_allowed(4), "link 4 not in allowlist");
        assert!(!state.is_link_allowed(42), "link 42 not in allowlist");
        // check_and_update should reject disallowed link even with fresh timestamp
        assert!(!state.check_and_update(42, u64::MAX));
        // allowed link still accepts
        assert!(state.check_and_update(1, 100));
        // Clearing allowlist makes any link pass
        state.set_allowlist(&[]);
        assert!(state.is_link_allowed(42));
    }

    #[test]
    fn allowlist_boundary_link_ids() {
        // Test bitmap boundary values (0, 63, 64, 127, 128, 255)
        let mut state = ReplayState::new();
        state.set_allowlist(&[0, 63, 64, 127, 128, 255]);
        for id in [0u8, 63, 64, 127, 128, 255] {
            assert!(state.is_link_allowed(id), "id {} must be allowed", id);
        }
        for id in [1u8, 62, 65, 126, 129, 254] {
            assert!(!state.is_link_allowed(id), "id {} must NOT be allowed", id);
        }
    }

    #[test]
    fn replay_state_persistence_roundtrip() {
        let tmp = std::env::temp_dir().join("oasis_replay_rt.bin");
        let tmp_s = tmp.to_str().unwrap();
        let mut state = ReplayState::new();
        state.set_allowlist(&[1, 5, 42]);
        state.check_and_update(1, 1000);
        state.check_and_update(5, 2000);
        state.save(tmp_s).expect("save");
        let loaded = ReplayState::load(tmp_s).expect("load");
        // Highwater preserved
        assert!(loaded.is_fresh(1, 1001), "just-above-1000 still fresh");
        assert!(!loaded.is_fresh(1, 1000), "equal to highwater not fresh");
        assert!(!loaded.is_fresh(1, 500), "below highwater not fresh");
        assert!(loaded.is_fresh(5, 2001));
        // Allowlist preserved
        assert!(loaded.is_link_allowed(1));
        assert!(loaded.is_link_allowed(42));
        assert!(!loaded.is_link_allowed(100));
        let _ = std::fs::remove_file(tmp_s);
    }

    #[test]
    #[serial]
    fn require_signed_rejects_unsigned() {
        std::env::set_var("OASIS_MAVLINK_REQUIRE_SIGNED", "1");
        let mut state = ReplayState::new();
        // Build a valid unsigned ATTITUDE frame
        let mut payload = [0u8; 28];
        payload[4..8].copy_from_slice(&0.1f32.to_le_bytes());
        let mut frame = Vec::new();
        frame.push(0xFD); frame.push(28); frame.push(0); frame.push(0);
        frame.push(1); frame.push(1); frame.push(1);
        frame.push(30); frame.push(0); frame.push(0);
        frame.extend_from_slice(&payload);
        let crc = crc_over(&frame[1..], crc_extra_for(30).unwrap());
        frame.push((crc & 0xFF) as u8);
        frame.push((crc >> 8) as u8);
        // With REQUIRE_SIGNED, parse_frame_checked must reject it.
        assert!(parse_frame_checked(&frame, &mut state).is_none(),
                "unsigned frame must be rejected when REQUIRE_SIGNED=1");
        // Without REQUIRE_SIGNED, the same frame must parse.
        std::env::remove_var("OASIS_MAVLINK_REQUIRE_SIGNED");
        assert!(parse_frame_checked(&frame, &mut state).is_some(),
                "unsigned frame accepted when REQUIRE_SIGNED unset");
    }

    #[test]
    fn atomic_save_leaves_no_tmp_on_success() {
        let tmp = std::env::temp_dir().join("oasis_atomic_save.bin");
        let tmp_s = tmp.to_str().unwrap();
        let tmp_tmp = format!("{}.tmp", tmp_s);
        let _ = std::fs::remove_file(tmp_s);
        let _ = std::fs::remove_file(&tmp_tmp);
        let st = ReplayState::new();
        st.save(tmp_s).expect("save");
        assert!(std::path::Path::new(tmp_s).exists(), "final file exists");
        assert!(!std::path::Path::new(&tmp_tmp).exists(), ".tmp removed after rename");
        let _ = std::fs::remove_file(tmp_s);
    }

    #[test]
    fn replay_state_persistence_no_allowlist() {
        let tmp = std::env::temp_dir().join("oasis_replay_rt2.bin");
        let tmp_s = tmp.to_str().unwrap();
        let mut state = ReplayState::new();  // no allowlist
        state.check_and_update(99, 42);
        state.save(tmp_s).unwrap();
        let loaded = ReplayState::load(tmp_s).unwrap();
        assert!(loaded.is_link_allowed(99));
        assert!(loaded.is_link_allowed(200), "no allowlist → any link OK");
        assert!(!loaded.is_fresh(99, 42), "equal ts not fresh");
        assert!(loaded.is_fresh(99, 43));
        let _ = std::fs::remove_file(tmp_s);
    }

    #[test]
    #[serial]
    fn signing_with_env_var() {
        // Set key in env and verify parse_frame uses it to reject bad signatures.
        let hex_key = "00".repeat(32);
        std::env::set_var("OASIS_MAVLINK_SECRET", &hex_key);
        let key = [0u8; 32];
        // Build a valid signed ATTITUDE frame.
        let mut payload = [0u8; 28];
        payload[4..8].copy_from_slice(&0.1f32.to_le_bytes());
        let mut frame = Vec::new();
        frame.push(0xFD); frame.push(28); frame.push(MAVLINK_IFLAG_SIGNED); frame.push(0);
        frame.push(1); frame.push(1); frame.push(1);
        frame.push(30); frame.push(0); frame.push(0);
        frame.extend_from_slice(&payload);
        let crc = crc_over(&frame[1..], crc_extra_for(30).unwrap());
        frame.push((crc & 0xFF) as u8);
        frame.push((crc >> 8) as u8);
        let signed = sign_frame(frame, &key, 2, 42_000);
        assert!(parse_frame(&signed).is_some(), "valid signed frame must parse when key set");

        // Tamper in body
        let mut bad = signed.clone();
        bad[5] ^= 0xFF;  // change sysid → invalidates signature
        assert!(parse_frame(&bad).is_none(), "tampered signed frame must be rejected");

        std::env::remove_var("OASIS_MAVLINK_SECRET");
    }

    #[test]
    fn encode_set_position_target_shape() {
        let frame = encode_set_position_target(42, 1, 1, 1, 1, 1.5, -0.5, 0.0);
        assert_eq!(frame[0], 0xFD);
        assert_eq!(frame[1], 53);
        assert_eq!(frame[4], 42); // seq
        // msgid 84 at offset 7
        assert_eq!(frame[7], 84);
        assert_eq!(frame[8], 0);
        assert_eq!(frame[9], 0);
        // vx at payload offset 16 → frame offset 10+16 = 26
        let vx = f32::from_le_bytes(frame[26..30].try_into().unwrap());
        assert!((vx - 1.5).abs() < 1e-5);
        let vy = f32::from_le_bytes(frame[30..34].try_into().unwrap());
        assert!((vy - (-0.5)).abs() < 1e-5);
    }

    #[test]
    fn param_set_encodes_correctly() {
        let frame = encode_param_set(3, 255, 190, 1, 1, "MPC_XY_VEL_MAX", 4.0);
        let (h, _) = parse_frame(&frame).expect("PARAM_SET must self-parse");
        assert_eq!(h.msgid, 23);
        assert_eq!(h.len, 23);
        // param_value at payload offset 0 = frame offset 10..14
        let v = f32::from_le_bytes(frame[10..14].try_into().unwrap());
        assert!((v - 4.0).abs() < 1e-5);
        // param_id at payload offset 6 = frame offset 16..32 (16 bytes, null-padded)
        let id_bytes = &frame[16..32];
        let id_str = std::str::from_utf8(&id_bytes[..14]).unwrap();
        assert_eq!(id_str, "MPC_XY_VEL_MAX");
        assert_eq!(frame[14], 1, "target_sys");
        assert_eq!(frame[15], 1, "target_comp");
        assert_eq!(frame[32], 9, "REAL32 type");
    }

    #[test]
    fn param_value_parses_px4_response() {
        // Build a PARAM_VALUE: value=2.0, count=100, index=5, id="MPC_XY_VEL_MAX", type=9
        let mut payload = [0u8; 25];
        payload[0..4].copy_from_slice(&2.0f32.to_le_bytes());
        payload[4..6].copy_from_slice(&100u16.to_le_bytes());
        payload[6..8].copy_from_slice(&5u16.to_le_bytes());
        let id = b"MPC_XY_VEL_MAX";
        payload[8..8 + id.len()].copy_from_slice(id);
        payload[24] = 9;
        let frame = build_frame(22, 1, &payload);
        let (_, m) = parse_frame(&frame).expect("PARAM_VALUE must parse");
        match m {
            MavMsg::ParamValue { param_value, param_id, param_type, .. } => {
                assert!((param_value - 2.0).abs() < 1e-5);
                assert_eq!(param_type, 9);
                // param_id is null-padded; find end
                let end = param_id.iter().position(|&b| b == 0).unwrap_or(16);
                assert_eq!(&param_id[..end], b"MPC_XY_VEL_MAX");
            }
            _ => panic!("expected ParamValue"),
        }
    }

    #[test]
    fn command_long_arm_encodes_correctly() {
        // MAV_CMD_COMPONENT_ARM_DISARM = 400, param1=1.0 → ARM
        let frame = encode_command_long(1, 255, 190, 1, 1, 400, 0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (h, _) = parse_frame(&frame).expect("COMMAND_LONG must self-parse");
        assert_eq!(h.msgid, 76);
        assert_eq!(h.len, 33);
        // Verify target_sys at payload offset 30 (frame offset 10+30=40)
        assert_eq!(frame[40], 1);
        // Verify command (u16) at payload offset 28 (frame offset 38-39)
        let cmd = u16::from_le_bytes([frame[38], frame[39]]);
        assert_eq!(cmd, 400);
    }

    #[test]
    fn set_mode_offboard_encodes_correctly() {
        // PX4 OFFBOARD: base_mode=0x01, custom_mode=0x00060000
        let frame = encode_set_mode(1, 255, 190, 1, 0x01, 0x00060000);
        let (h, _) = parse_frame(&frame).expect("SET_MODE must self-parse");
        assert_eq!(h.msgid, 11);
        assert_eq!(h.len, 6);
        // custom_mode at payload offset 0 (frame offset 10-13)
        let custom = u32::from_le_bytes([frame[10], frame[11], frame[12], frame[13]]);
        assert_eq!(custom, 0x00060000);
        assert_eq!(frame[14], 1, "target_sys");
        assert_eq!(frame[15], 0x01, "base_mode");
    }

    #[test]
    fn parse_command_ack() {
        let mut payload = [0u8; 3];
        payload[0..2].copy_from_slice(&400u16.to_le_bytes());  // COMPONENT_ARM_DISARM
        payload[2] = 2;  // MAV_RESULT_DENIED (pre-arm check failed)
        let frame = build_frame(77, 1, &payload);
        let (_, m) = parse_frame(&frame).expect("COMMAND_ACK must parse");
        match m {
            MavMsg::CommandAck { command, result } => {
                assert_eq!(command, 400);
                assert_eq!(result, 2, "DENIED result");
            }
            _ => panic!("expected CommandAck"),
        }
    }

    #[test]
    fn parse_local_position_ned() {
        let mut payload = [0u8; 28];
        payload[0..4].copy_from_slice(&1000u32.to_le_bytes());
        payload[4..8].copy_from_slice(&1.5f32.to_le_bytes());
        payload[8..12].copy_from_slice(&(-2.0f32).to_le_bytes());
        payload[12..16].copy_from_slice(&(-1.3f32).to_le_bytes());  // NED z-down
        payload[16..20].copy_from_slice(&0.3f32.to_le_bytes());
        let frame = build_frame(32, 1, &payload);
        let (_, m) = parse_frame(&frame).expect("LOCAL_POSITION_NED");
        match m {
            MavMsg::LocalPositionNed { x, y, z, vx, .. } => {
                assert!((x - 1.5).abs() < 1e-5);
                assert!((y + 2.0).abs() < 1e-5);
                assert!((z + 1.3).abs() < 1e-5);
                assert!((vx - 0.3).abs() < 1e-5);
            }
            _ => panic!("expected LocalPositionNed"),
        }
    }

    #[test]
    fn parse_attitude_quaternion_identity() {
        // Identity quaternion (w=1, x=y=z=0) → roll=pitch=yaw=0
        let mut payload = [0u8; 32];
        payload[0..4].copy_from_slice(&500u32.to_le_bytes());
        payload[4..8].copy_from_slice(&1.0f32.to_le_bytes());   // w
        payload[8..12].copy_from_slice(&0.0f32.to_le_bytes());
        payload[12..16].copy_from_slice(&0.0f32.to_le_bytes());
        payload[16..20].copy_from_slice(&0.0f32.to_le_bytes());
        let frame = build_frame(31, 1, &payload);
        let (_, m) = parse_frame(&frame).expect("ATTITUDE_QUATERNION");
        let mut acc = MavToOasisAccumulator::default();
        acc.apply(&m);
        assert!(acc.roll.abs() < 1e-5, "identity quat → roll=0, got {}", acc.roll);
        assert!(acc.pitch.abs() < 1e-5, "identity quat → pitch=0");
        assert!(acc.has_attitude);
    }

    #[test]
    fn vfr_hud_does_not_override_local_alt() {
        // Regression: PX4 sends VFR_HUD.alt = MSL (e.g., 491m) which used to overwrite
        // the LOCAL_POSITION_NED-derived altitude. Adapter then logged junk z values.
        let mut acc = MavToOasisAccumulator::default();
        // Local position arrives first (drone at 2 m up): z = -2 → alt = 2
        acc.apply(&MavMsg::LocalPositionNed {
            time_boot_ms: 100, x: 0.0, y: 0.0, z: -2.0,
            vx: 0.0, vy: 0.0, vz: 0.0,
        });
        assert!((acc.alt - 2.0).abs() < 1e-5, "after LOCAL_POSITION_NED, alt must be 2.0");
        // Then PX4 spams VFR_HUD with MSL altitude (Zurich-ish)
        acc.apply(&MavMsg::VfrHud {
            airspeed: 0.0, groundspeed: 0.0,
            alt: 491.0, climb: 0.0, heading: 0, throttle: 0,
        });
        assert!((acc.alt - 2.0).abs() < 1e-5,
                "VFR_HUD must NOT overwrite alt once local position is locked, got {}", acc.alt);
    }

    #[test]
    fn parse_vfr_hud() {
        let mut payload = [0u8; 20];
        payload[0..4].copy_from_slice(&10.0f32.to_le_bytes());  // airspeed
        payload[4..8].copy_from_slice(&12.5f32.to_le_bytes());  // groundspeed
        payload[8..12].copy_from_slice(&100.0f32.to_le_bytes()); // alt
        payload[12..16].copy_from_slice(&0.5f32.to_le_bytes()); // climb
        payload[16..18].copy_from_slice(&45i16.to_le_bytes());  // heading
        payload[18..20].copy_from_slice(&50u16.to_le_bytes());  // throttle
        let frame = build_frame(74, 1, &payload);
        let (_, m) = parse_frame(&frame).expect("VFR_HUD");
        match m {
            MavMsg::VfrHud { alt, airspeed, groundspeed, heading, throttle, .. } => {
                assert!((alt - 100.0).abs() < 1e-5);
                assert!((airspeed - 10.0).abs() < 1e-5);
                assert!((groundspeed - 12.5).abs() < 1e-5);
                assert_eq!(heading, 45);
                assert_eq!(throttle, 50);
            }
            _ => panic!("expected VfrHud"),
        }
    }

    #[test]
    fn accumulator_fires_on_px4_stream() {
        // PX4 sends ATTITUDE_QUATERNION + LOCAL_POSITION_NED (not ATTITUDE + GLOBAL_POSITION_INT).
        // Accumulator must produce a valid OASIS JSON from PX4's native stream.
        let mut acc = MavToOasisAccumulator::default();
        acc.apply(&MavMsg::AttitudeQuaternion {
            time_boot_ms: 100, q1: 1.0, q2: 0.0, q3: 0.0, q4: 0.0,
            rollspeed: 0.0, pitchspeed: 0.0, yawspeed: 0.2,
        });
        acc.apply(&MavMsg::LocalPositionNed {
            time_boot_ms: 100, x: 1.0, y: 2.0, z: -1.3,
            vx: 0.5, vy: -0.2, vz: 0.0,
        });
        assert!(acc.ready_to_emit(), "PX4 stream must trigger emit");
        let json = acc.to_oasis_json();
        assert!(json.contains("\"x\":1.0000"));
        assert!(json.contains("\"y\":2.0000"));
        assert!(json.contains("\"alt\":1.3000"),
                "NED z=-1.3 → altitude +1.3, got {}", json);
        assert!(json.contains("\"gz\":0.2000"));
        // Verify JSON is parseable
        let _: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    }

    #[test]
    fn accumulator_produces_valid_oasis_json() {
        let mut acc = MavToOasisAccumulator::default();
        acc.apply(&MavMsg::Attitude {
            time_boot_ms: 0, roll: 0.1, pitch: 0.2, yaw: 0.3,
            rollspeed: 0.0, pitchspeed: 0.0, yawspeed: 0.5,
        });
        acc.apply(&MavMsg::GlobalPositionInt {
            time_boot_ms: 0, lat: 12345678, lon: 87654321, alt: 1300, relative_alt: 1300,
            vx: 50, vy: -30, vz: 0, hdg: 0,
        });
        assert!(acc.ready_to_emit());
        let json = acc.to_oasis_json();
        assert!(json.contains("\"tick\":1"));
        assert!(json.contains("\"roll\":0.1000"));
        assert!(json.contains("\"alt\":1.3000"));
        assert!(json.contains("\"vx\":0.5000"));
        // Can parse back
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["tick"], 1);
    }

    #[test]
    fn sys_status_decodes_health_flags() {
        let mut payload = [0u8; 12];
        payload[0..4].copy_from_slice(&0u32.to_le_bytes());
        payload[4..8].copy_from_slice(&0u32.to_le_bytes());
        payload[8..12].copy_from_slice(&(0x01u32 | 0x20u32).to_le_bytes()); // gyro+gps, NOT laser
        let frame = build_frame(1, 0, &payload);

        let (_, m) = parse_frame(&frame).expect("parse SYS_STATUS with valid CRC");
        let mut acc = MavToOasisAccumulator::default();
        acc.apply(&m);
        assert!(acc.imu_alive);
        assert!(acc.gps_alive);
        assert!(!acc.sonar_alive, "laser bit not set → sonar_alive false");
    }
}
