//! OASIS — the network layer the Modbus TCP gateway was missing (`prompts/OASIS_PILOT_GATEWAY.md`
//! phase A).
//!
//! `modbus_tcp` is pure: it parses, decides and builds frames, and an audit found exactly
//! what that implies — **there was no socket anywhere in `oasis-rt`**, so the TCP layer
//! existed only in memory and no pilot was possible. This module adds the transport, and
//! nothing else: the gate, the order format and the frame builders are untouched.
//!
//! ```text
//! HMI ──Modbus TCP, clear──▶ agent ──v0B over TCP──▶ gateway ──Modbus TCP──▶ PLC
//!                             (operator side)         (machine side)
//! ```
//!
//! **Framing between agent and gateway**: `len u32 BE | v0B envelope`, with `len` bounded
//! by [`MAX_LINK_FRAME`]. A length prefix is needed because TCP is a stream, and the bound
//! is needed because the length comes from the network — an unbounded prefix is an
//! allocation an attacker chooses.
//!
//! Blocking threads, no async runtime: one connection per order, a read timeout on every
//! socket, and no shared mutable state between connections except the gateway's own
//! counters behind a mutex. The prompt asks for a measured justification before an async
//! runtime, and at one order per HMI write there is nothing to measure.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A v0B envelope carrying an order is 153 B, a sealed one 203 B, a fragmented authority
/// message more. 4 KiB is far above anything this link carries and far below anything that
/// would hurt to allocate.
pub const MAX_LINK_FRAME: usize = 4096;
/// Length prefix: `u32` big-endian, like Modbus itself.
pub const LEN_PREFIX: usize = 4;

/// Write one length-prefixed frame.
pub fn write_frame(sock: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_LINK_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "frame too large"));
    }
    let mut out = Vec::with_capacity(LEN_PREFIX + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    sock.write_all(&out)?;
    sock.flush()
}

/// Read one length-prefixed frame.
///
/// The length is checked **before** any allocation: a declared length over
/// [`MAX_LINK_FRAME`] is refused without reading the body, so a hostile peer cannot make
/// this side reserve memory of its choosing.
pub fn read_frame(sock: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; LEN_PREFIX];
    sock.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > MAX_LINK_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "declared length out of range"));
    }
    let mut body = vec![0u8; len];
    sock.read_exact(&mut body)?;
    Ok(body)
}

/// Connect with a timeout on every operation. A socket without a read timeout turns a
/// silent peer into a hung gateway, and the prompt is explicit that the HMI must never be
/// left in silence.
pub fn connect_timeout(addr: &str, timeout: Duration) -> io::Result<TcpStream> {
    let sa = addr.to_socket_addrs()?.next().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no address"))?;
    let sock = TcpStream::connect_timeout(&sa, timeout)?;
    sock.set_read_timeout(Some(timeout))?;
    sock.set_write_timeout(Some(timeout))?;
    sock.set_nodelay(true)?;
    Ok(sock)
}

/// One Modbus TCP exchange: send `req`, read one response, return it.
///
/// Reads the MBAP header first and then exactly the length it declares, so a short or a
/// chatty peer cannot desynchronise the stream.
pub fn modbus_exchange(sock: &mut TcpStream, req: &[u8]) -> io::Result<Vec<u8>> {
    sock.write_all(req)?;
    sock.flush()?;
    let mut head = [0u8; 6];
    sock.read_exact(&mut head)?;
    let rest = u16::from_be_bytes([head[4], head[5]]) as usize;
    if rest == 0 || rest > 260 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MBAP length out of range"));
    }
    let mut out = Vec::with_capacity(6 + rest);
    out.extend_from_slice(&head);
    let mut body = vec![0u8; rest];
    sock.read_exact(&mut body)?;
    out.extend_from_slice(&body);
    Ok(out)
}

/// Read one Modbus TCP request from a connected client, same discipline as above.
pub fn modbus_read_request(sock: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut head = [0u8; 6];
    sock.read_exact(&mut head)?;
    let rest = u16::from_be_bytes([head[4], head[5]]) as usize;
    if rest == 0 || rest > 260 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MBAP length out of range"));
    }
    let mut out = Vec::with_capacity(6 + rest);
    out.extend_from_slice(&head);
    let mut body = vec![0u8; rest];
    sock.read_exact(&mut body)?;
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod tests;
