//! OASIS-RT — Transport abstraction for federation digest exchange.
//!
//! The current federation uses filesystem for peer-to-peer exchange (see federation.rs).
//! This trait formalizes the transport layer so LoRa / BLE / TCP / QR can plug in
//! without changing federation logic.
//!
//! Design constraints:
//! - No async runtime (keep it synchronous-first for edge / MCU)
//! - No std::net dependency in the trait itself (file + in-memory work without it)
//! - Digest bytes are opaque to transport — signing happens at federation layer
//!
//! Usage (current codebase still uses direct file I/O; this is the migration path):
//! ```no_run
//! use oasis_rt::transport::{Transport, FileTransport, InMemoryTransport};
//! let t = FileTransport::new("C:/dev/oasis/webots/recon10v6_shared/");
//! t.send("d00", b"digest-bytes").unwrap();
//! let peers = t.list_peers("d00").unwrap();
//! for p in peers { let _ = t.recv(&p); }
//! ```

use std::io;

/// Opaque transport for peer-to-peer digest exchange.
pub trait Transport {
    /// Send a digest blob identified by the sender's name.
    /// Idempotent: repeated calls with the same name replace the previous blob.
    fn send(&self, sender: &str, blob: &[u8]) -> io::Result<()>;

    /// Receive the latest blob from a specific peer. Returns Err if peer unknown.
    fn recv(&self, peer: &str) -> io::Result<Vec<u8>>;

    /// Enumerate peer names visible to this transport (excludes `own`).
    fn list_peers(&self, own: &str) -> io::Result<Vec<String>>;
}

// ───────── FileTransport ─────────

/// Filesystem-backed transport. Wraps the current save/load pattern.
pub struct FileTransport {
    root: String,
}

impl FileTransport {
    pub fn new(root: impl Into<String>) -> Self {
        let root = root.into();
        let _ = std::fs::create_dir_all(&root);
        Self { root }
    }

    fn path(&self, name: &str) -> String {
        format!("{}{}_fed.bin", self.root, name)
    }
}

impl Transport for FileTransport {
    fn send(&self, sender: &str, blob: &[u8]) -> io::Result<()> {
        std::fs::write(self.path(sender), blob)
    }

    fn recv(&self, peer: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.path(peer))
    }

    fn list_peers(&self, own: &str) -> io::Result<Vec<String>> {
        let mut peers = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            if let Some(stem) = fname_str.strip_suffix("_fed.bin") {
                if stem != own { peers.push(stem.to_string()); }
            }
        }
        Ok(peers)
    }
}

// ───────── InMemoryTransport (for tests) ─────────

use std::sync::Mutex;
use std::collections::HashMap;

/// In-memory transport. All instances sharing the same `Box<Mutex<HashMap>>` see each other.
pub struct InMemoryTransport {
    store: std::sync::Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl InMemoryTransport {
    pub fn new() -> Self {
        Self { store: std::sync::Arc::new(Mutex::new(HashMap::new())) }
    }
    pub fn clone_handle(&self) -> Self {
        Self { store: self.store.clone() }
    }
}

impl Default for InMemoryTransport {
    fn default() -> Self { Self::new() }
}

impl Transport for InMemoryTransport {
    fn send(&self, sender: &str, blob: &[u8]) -> io::Result<()> {
        self.store.lock().unwrap().insert(sender.to_string(), blob.to_vec());
        Ok(())
    }
    fn recv(&self, peer: &str) -> io::Result<Vec<u8>> {
        self.store.lock().unwrap().get(peer).cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "peer not present"))
    }
    fn list_peers(&self, own: &str) -> io::Result<Vec<String>> {
        Ok(self.store.lock().unwrap().keys()
            .filter(|k| k.as_str() != own)
            .cloned().collect())
    }
}

// ───────── LoRa transport — generic over Read+Write IO ─────────
//
// Wire format (LoRa-friendly: short, self-delimited, single-fragment ≤ 200 B):
//   [0..4]  LORA  ASCII magic
//   [4]     ver   = 0x01
//   [5..7]  len   u16 LE — payload length
//   [7]     crc8  CRC-8/CCITT-FALSE over magic..len (header integrity)
//   [8..]   payload (≤ MAX_LORA_PAYLOAD bytes)
//
// Sender wraps `serialize_to_vec()` output; if blob > MAX_LORA_PAYLOAD,
// caller must fragment first (use spore::fragment_v2).
//
// Bring-up plan for real RFM95 / E32 module (host code):
//   1. Open COM port via `serialport` crate (not depended on here).
//   2. AT-mode E32: send `AT+SET=...` to configure SF/BW/freq.
//   3. After config, write framed bytes; module transmits on RF.
//   4. Receiving side: read framed bytes from serial RX, call parse_lora_frame.
//
// Duty-cycle (EU 868 MHz region 1%): sender must throttle. Not enforced here —
// caller responsibility. US 915 MHz (FCC FHSS) less strict.

pub const MAX_LORA_PAYLOAD: usize = 200; // safe under SX1276 max 255 - 8 header
const LORA_MAGIC: &[u8; 4] = b"LORA";
const LORA_VER: u8 = 0x01;
const LORA_HEADER_LEN: usize = 8;

/// CRC-8/CCITT-FALSE: poly=0x07, init=0x00.
fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if crc & 0x80 != 0 { crc = (crc << 1) ^ 0x07; } else { crc <<= 1; }
        }
    }
    crc
}

/// Build a LoRa frame ready to push out a serial port to a SX127x / E32 module.
pub fn pack_lora_frame(payload: &[u8]) -> Result<Vec<u8>, &'static str> {
    if payload.len() > MAX_LORA_PAYLOAD { return Err("payload exceeds MAX_LORA_PAYLOAD"); }
    let mut frame = Vec::with_capacity(LORA_HEADER_LEN + payload.len());
    frame.extend_from_slice(LORA_MAGIC);
    frame.push(LORA_VER);
    let len = payload.len() as u16;
    frame.extend_from_slice(&len.to_le_bytes());
    let header_crc = crc8(&frame);
    frame.push(header_crc);
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Parse a LoRa frame. Returns the payload slice on success.
pub fn parse_lora_frame(frame: &[u8]) -> Result<&[u8], &'static str> {
    if frame.len() < LORA_HEADER_LEN { return Err("frame too short"); }
    if &frame[..4] != LORA_MAGIC { return Err("bad magic"); }
    if frame[4] != LORA_VER { return Err("unsupported version"); }
    let len = u16::from_le_bytes([frame[5], frame[6]]) as usize;
    let expected_crc = crc8(&frame[..7]);
    if frame[7] != expected_crc { return Err("header crc mismatch"); }
    if frame.len() < LORA_HEADER_LEN + len { return Err("payload truncated"); }
    Ok(&frame[LORA_HEADER_LEN..LORA_HEADER_LEN + len])
}

/// Generic LoRa transport: writes frames to any `Write`, reads from any `Read`.
/// Use `LoRaTransport::<File>::new(File::open("/dev/ttyUSB0")?)` once the
/// `serialport` crate is wired in at the application layer.
pub struct LoRaTransport<W: io::Write> {
    writer: Mutex<W>,
}

impl<W: io::Write> LoRaTransport<W> {
    pub fn new(writer: W) -> Self { Self { writer: Mutex::new(writer) } }

    /// Send a LoRa-framed blob. Errors if blob exceeds MAX_LORA_PAYLOAD.
    pub fn send_framed(&self, blob: &[u8]) -> io::Result<()> {
        let frame = pack_lora_frame(blob)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let mut w = self.writer.lock().unwrap();
        w.write_all(&frame)?;
        w.flush()
    }
}

impl<W: io::Write + Send> Transport for LoRaTransport<W> {
    fn send(&self, _sender: &str, blob: &[u8]) -> io::Result<()> {
        self.send_framed(blob)
    }
    /// Read-side requires a paired Read transport — see `parse_lora_frame`
    /// which is exposed for receiver-side integration with serial RX.
    fn recv(&self, _peer: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::new(io::ErrorKind::Unsupported,
            "LoRaTransport is write-side; read via parse_lora_frame on serial RX"))
    }
    fn list_peers(&self, _own: &str) -> io::Result<Vec<String>> { Ok(Vec::new()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_roundtrip() {
        let t = InMemoryTransport::new();
        t.send("d00", b"hello").unwrap();
        assert_eq!(t.recv("d00").unwrap(), b"hello");
        assert_eq!(t.list_peers("d01").unwrap(), vec!["d00".to_string()]);
        assert!(t.list_peers("d00").unwrap().is_empty());
    }

    #[test]
    fn in_memory_shared_handle_sees_peer_writes() {
        let t1 = InMemoryTransport::new();
        let t2 = t1.clone_handle();
        t1.send("a", b"from-a").unwrap();
        t2.send("b", b"from-b").unwrap();
        let peers_from_a = t1.list_peers("a").unwrap();
        assert!(peers_from_a.contains(&"b".to_string()));
        let data = t1.recv("b").unwrap();
        assert_eq!(data, b"from-b");
    }

    #[test]
    fn file_transport_roundtrip() {
        let tmp = std::env::temp_dir().join("oasis_transport_test/");
        let tmp_s = tmp.to_string_lossy().to_string();
        let t = FileTransport::new(&tmp_s);
        t.send("dX", b"payload").unwrap();
        let got = t.recv("dX").unwrap();
        assert_eq!(got, b"payload");
        let peers = t.list_peers("dY").unwrap();
        assert!(peers.contains(&"dX".to_string()));
        let _ = std::fs::remove_dir_all(&tmp_s);
    }

    #[test]
    fn lora_pack_parse_roundtrip() {
        let payload = b"hello LoRa over serial radio";
        let frame = pack_lora_frame(payload).unwrap();
        assert!(frame.starts_with(LORA_MAGIC));
        assert_eq!(frame[4], LORA_VER);
        let parsed = parse_lora_frame(&frame).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn lora_rejects_bad_magic() {
        let mut frame = pack_lora_frame(b"x").unwrap();
        frame[0] = b'Z';
        assert_eq!(parse_lora_frame(&frame).unwrap_err(), "bad magic");
    }

    #[test]
    fn lora_rejects_bad_header_crc() {
        let mut frame = pack_lora_frame(b"x").unwrap();
        frame[7] ^= 0xFF;
        assert_eq!(parse_lora_frame(&frame).unwrap_err(), "header crc mismatch");
    }

    #[test]
    fn lora_rejects_oversize_payload() {
        let big = vec![0u8; MAX_LORA_PAYLOAD + 1];
        assert!(pack_lora_frame(&big).is_err());
    }

    #[test]
    fn lora_transport_writes_to_buffer() {
        // Write LoRa frames into a Vec<u8> as a stand-in for a serial port.
        let buf: Vec<u8> = Vec::new();
        let t = LoRaTransport::new(buf);
        t.send_framed(b"digest1").unwrap();
        t.send_framed(b"digest2").unwrap();
        let writer = t.writer.into_inner().unwrap();
        // Should contain two framed messages back-to-back
        let f1 = parse_lora_frame(&writer).unwrap();
        assert_eq!(f1, b"digest1");
        // Second frame starts after first frame
        let f1_total_len = LORA_HEADER_LEN + b"digest1".len();
        let f2 = parse_lora_frame(&writer[f1_total_len..]).unwrap();
        assert_eq!(f2, b"digest2");
    }
}
