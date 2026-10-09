//! `oasis_sma_probe` — read-only SunSpec discovery against a Modbus TCP device.
//!
//! Rule 1 of `prompts/OASIS_SMA_REAL_TEST.md`: the write register's address must be
//! **found on the device**, by walking the SunSpec model chain the way SMA's *Technical
//! Information — SunSpec Modbus* (`SunSpecModbus-TI-en-11`) §4 prescribes — "All
//! information models start with an ID register and a length register … information models
//! can be found and used" — and never copied from a forum. This tool does exactly that.
//!
//! **It only ever sends FC03 (read holding registers). It cannot write.** So it is safe to
//! point at a live inverter the moment its Modbus server is on, and it is how the register
//! map for the gateway gets its one address without guessing.
//!
//! ```text
//! oasis_sma_probe --addr 169.254.12.3:502 --unit 126
//! oasis_sma_probe --addr 169.254.12.3:502 --unit 126 --power   # read AC power once
//! ```
//!
//! It reports: the SunSpec base it found, every model in the chain (id, register, length),
//! the common-model identity (manufacturer / model / version / serial — the serial is
//! printed with only its last 4 characters masked, per rule 10), and, if model 123 or 704
//! is present, the block dumped so the `WMaxLimPct` point and its scale factor can be
//! confirmed against the SunSpec model definition before any write is ever planned.
//!
//! ⚠️ It does **not** decide the write address on its own: it locates and prints the block,
//! and the address goes into the plan for the user to approve. Reading the value the point
//! actually holds is the check that the located offset is the right one.

use std::time::Duration;

use oasis_rt::mbtcp_net::{connect_timeout, modbus_exchange};

/// The SunSpec identifier `"SunS"` as a 32-bit value, stored in the two registers at the
/// base of the map. Finding it is how a device confirms it speaks SunSpec at that base.
const SUNS: u32 = 0x5375_6e53;

/// Candidate base register addresses (0-based Modbus addressing). SunSpec devices place
/// the map at one of these well-known bases; SMA commonly uses 40000.
const BASES: [u16; 3] = [40000, 50000, 0];

/// A model header found in the chain: its SunSpec id, the register where its **block
/// body** begins (just after the 2-register header), and the block length in registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Model {
    id: u16,
    /// Register address of the first data register of the block (header excluded).
    body_reg: u16,
    len: u16,
}

/// Walk the model chain over registers already read into `regs`, starting at the word
/// index of the SunSpec marker. `base_reg` is the Modbus address that `regs[0]` maps to,
/// so a model's `body_reg` can be reported as a real device address.
///
/// Pure and total: it stops at the end marker `0xFFFF`, at a zero-length model, or when the
/// next header would run past what was read — never panics, never loops forever. This is
/// the part worth a unit test, and it has one.
fn walk_models(regs: &[u16], marker_idx: usize, base_reg: u16) -> Vec<Model> {
    let mut out = Vec::new();
    // The marker is 2 registers; the first model header follows it.
    let mut i = marker_idx + 2;
    while i + 1 < regs.len() {
        let id = regs[i];
        let len = regs[i + 1];
        if id == 0xFFFF {
            break; // SunSpec end model
        }
        let body = i + 2;
        // A model whose body would run past what we read: stop rather than invent.
        if body > regs.len() {
            break;
        }
        let body_reg = base_reg.wrapping_add(body as u16);
        out.push(Model { id, body_reg, len });
        // Guard against a zero/garbage length that would not advance the cursor.
        let step = 2usize + len as usize;
        if step < 2 || len == 0 && id != 0 {
            // len 0 is legal only for a few models; to stay total, advance by the header.
            i = body;
            if len == 0 {
                continue;
            }
        }
        i += step;
    }
    out
}

/// Read 16-bit registers as a NUL/space-trimmed ASCII string (SunSpec strings are
/// big-endian register pairs of two chars each).
fn ascii_from_regs(regs: &[u16]) -> String {
    let mut bytes = Vec::with_capacity(regs.len() * 2);
    for &r in regs {
        bytes.push((r >> 8) as u8);
        bytes.push((r & 0xff) as u8);
    }
    let s: String = bytes.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { ' ' }).collect();
    s.trim().to_string()
}

/// Mask all but the last 4 characters of a serial — actually, mask the LAST 4, per rule 10
/// ("masque le numéro de série, 4 derniers caractères"), keeping the prefix that identifies
/// the product family and hiding the unit-unique tail.
fn mask_serial(s: &str) -> String {
    let n = s.len();
    if n <= 4 {
        return "xxxx".to_string();
    }
    format!("{}{}", &s[..n - 4], "xxxx")
}

/// Build a plain Modbus TCP FC03 (read holding registers) request frame.
/// MBAP: tid, protocol 0, length, unit; PDU: fc 0x03, start, count.
fn fc03(tid: u16, unit: u8, start: u16, count: u16) -> [u8; 12] {
    [(tid >> 8) as u8, tid as u8, 0, 0, 0, 6, unit, 0x03, (start >> 8) as u8, start as u8, (count >> 8) as u8, count as u8]
}

/// Parse the data payload of an FC03 response into register words. Returns `None` on an
/// exception, a short frame, or a byte count that does not match.
fn parse_fc03(resp: &[u8]) -> Option<Vec<u16>> {
    // MBAP(7) + fc(1) + bytecount(1) + data
    if resp.len() < 9 {
        return None;
    }
    let fc = resp[7];
    if fc & 0x80 != 0 {
        return None; // Modbus exception
    }
    if fc != 0x03 {
        return None;
    }
    let bc = resp[8] as usize;
    if resp.len() < 9 + bc || bc % 2 != 0 {
        return None;
    }
    let mut regs = Vec::with_capacity(bc / 2);
    let mut i = 9;
    while i + 1 < 9 + bc {
        regs.push(u16::from_be_bytes([resp[i], resp[i + 1]]));
        i += 2;
    }
    Some(regs)
}

fn read_regs(addr: &str, unit: u8, start: u16, count: u16, tid: u16) -> Result<Vec<u16>, String> {
    let to = Duration::from_millis(3000);
    let mut sock = connect_timeout(addr, to).map_err(|e| format!("connect {addr}: {:?}", e.kind()))?;
    let req = fc03(tid, unit, start, count);
    let resp = modbus_exchange(&mut sock, &req).map_err(|e| format!("exchange: {:?}", e.kind()))?;
    parse_fc03(&resp).ok_or_else(|| {
        let code = resp.get(8).copied().unwrap_or(0);
        format!("FC03 refused or malformed at reg {start} (unit {unit}); byte8=0x{code:02x}")
    })
}

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == flag).and_then(|i| a.get(i + 1)).cloned()
}

fn main() -> std::process::ExitCode {
    let addr = match arg("--addr") {
        Some(a) => a,
        None => {
            eprintln!("usage: oasis_sma_probe --addr <ip:port> --unit <n> [--power]");
            return std::process::ExitCode::from(2);
        }
    };
    let unit: u8 = arg("--unit").and_then(|s| s.parse().ok()).unwrap_or(126);
    let want_power = std::env::args().any(|a| a == "--power");

    println!("# oasis_sma_probe (READ-ONLY, FC03 only)  addr={addr} unit={unit}");

    // 1. Find the SunSpec base.
    let mut found: Option<(u16, Vec<u16>)> = None;
    for &base in &BASES {
        match read_regs(&addr, unit, base, 2, 1) {
            Ok(regs) if regs.len() == 2 => {
                let marker = ((regs[0] as u32) << 16) | regs[1] as u32;
                if marker == SUNS {
                    println!("SunSpec marker 'SunS' found at base register {base}");
                    found = Some((base, regs));
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("  (base {base}: {e})"),
        }
    }
    let Some((base, _)) = found else {
        eprintln!("No SunSpec marker at bases {BASES:?}. Is the Modbus server on and the unit id right?");
        return std::process::ExitCode::from(1);
    };

    // 2. Read a window of the map and walk the model chain. FC03 reads at most 125 regs;
    // read a few windows and concatenate so the chain past the common model is seen.
    let mut regs: Vec<u16> = Vec::new();
    for w in 0..4u16 {
        match read_regs(&addr, unit, base + w * 120, 120, 100 + w) {
            Ok(mut r) => regs.append(&mut r),
            Err(e) => {
                eprintln!("  (window {w} stopped: {e})");
                break;
            }
        }
    }
    let models = walk_models(&regs, 0, base);
    println!("\nModels in the chain:");
    for m in &models {
        let name = match m.id {
            1 => " (Common)",
            101 => " (Inverter single phase)",
            103 => " (Inverter three phase)",
            120 => " (Nameplate)",
            121 => " (Settings)",
            122 => " (Extended measurement)",
            123 => " (Immediate controls — WMaxLimPct lives here)",
            701 => " (DER AC measurement)",
            702 => " (DER capacity)",
            704 => " (DER AC controls — WMaxLimPct lives here)",
            _ => "",
        };
        println!("  model {:>4}  body@reg {:>6}  len {:>3}{}", m.id, m.body_reg, m.len, name);
    }

    // 3. Common model (1) identity.
    if let Some(cm) = models.iter().find(|m| m.id == 1) {
        let b = cm.body_reg;
        let id_mfg = read_regs(&addr, unit, b, 16, 200).ok();
        let id_mdl = read_regs(&addr, unit, b + 16, 16, 201).ok();
        let id_ver = read_regs(&addr, unit, b + 40, 8, 202).ok();
        let id_ser = read_regs(&addr, unit, b + 48, 16, 203).ok();
        println!("\nIdentity (common model 1):");
        if let Some(r) = id_mfg {
            println!("  manufacturer : {}", ascii_from_regs(&r));
        }
        if let Some(r) = id_mdl {
            println!("  model        : {}", ascii_from_regs(&r));
        }
        if let Some(r) = id_ver {
            println!("  version/fw   : {}", ascii_from_regs(&r));
        }
        if let Some(r) = id_ser {
            println!("  serial       : {} (masked)", mask_serial(&ascii_from_regs(&r)));
        }
    }

    // 4. Dump the controls block so WMaxLimPct can be confirmed against the SunSpec model
    // definition. No write address is decided here — only read and shown.
    for id in [123u16, 704] {
        if let Some(m) = models.iter().find(|m| m.id == id) {
            if let Ok(block) = read_regs(&addr, unit, m.body_reg, m.len.min(120), 210 + id) {
                println!("\nControls block, model {id}, body@reg {} (len {}):", m.body_reg, m.len);
                for (off, &v) in block.iter().enumerate() {
                    println!("  +{:>2}  reg {:>6}  = {:>6}  (0x{:04x})", off, m.body_reg + off as u16, v, v);
                }
                println!("  ^ confirm WMaxLimPct and WMaxLimPct_SF offsets here against the SunSpec model {id} definition before planning any write.");
            }
        }
    }

    // 5. AC power, for the baseline — only on request, still read-only.
    if want_power {
        for m in models.iter().filter(|m| matches!(m.id, 101 | 103 | 701)) {
            if let Ok(block) = read_regs(&addr, unit, m.body_reg, m.len.min(120), 220) {
                println!("\nModel {} block read ({} regs) for the AC-power baseline — map the W point from the definition.", m.id, block.len());
            }
        }
    }

    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    // A hand-built SunSpec map: marker, common(1) stub, controls(123), end. Register words
    // only — no socket. This is what makes the chain walker testable without a device.
    fn sample() -> Vec<u16> {
        let mut r = vec![0x5375, 0x6e53]; // "SunS"
                                          // model 1, len 4
        r.extend_from_slice(&[1, 4, 0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD]);
        // model 123, len 3
        r.extend_from_slice(&[123, 3, 0x0000, 0x03E8, 0x0001]);
        // end
        r.extend_from_slice(&[0xFFFF, 0]);
        r
    }

    #[test]
    fn walks_the_chain_and_reports_real_register_addresses() {
        let regs = sample();
        let models = walk_models(&regs, 0, 40000);
        assert_eq!(models.len(), 2, "common + controls");
        assert_eq!(models[0].id, 1);
        assert_eq!(models[0].body_reg, 40000 + 4, "marker(2) + header(2)");
        assert_eq!(models[0].len, 4);
        assert_eq!(models[1].id, 123);
        // after marker(2)+hdr(2)+body(4) = 8, next hdr at idx 8, body at 10
        assert_eq!(models[1].body_reg, 40000 + 10);
        assert_eq!(models[1].len, 3);
    }

    #[test]
    fn stops_at_the_end_marker_not_past_it() {
        let regs = sample();
        let models = walk_models(&regs, 0, 40000);
        assert!(models.iter().all(|m| m.id != 0xFFFF), "the end marker is not a model");
    }

    #[test]
    fn walker_is_total_on_a_truncated_chain() {
        // A header that claims a body longer than what was read must not panic or loop.
        let regs = vec![0x5375, 0x6e53, 1, 200, 0x1111, 0x2222];
        let models = walk_models(&regs, 0, 40000);
        // The one model is recorded; the walk stops rather than reading past the buffer.
        assert!(models.len() <= 1);
    }

    #[test]
    fn walker_is_total_on_garbage() {
        for seed in [0u16, 0xFFFF, 1, 2, 7, 999] {
            let regs = vec![seed; 13];
            let _ = walk_models(&regs, 0, 40000); // must simply return
        }
    }

    #[test]
    fn fc03_response_parsed_and_exceptions_rejected() {
        // 2 registers: byte count 4.
        let ok = [0, 1, 0, 0, 0, 5, 126, 0x03, 4, 0x12, 0x34, 0x56, 0x78];
        assert_eq!(parse_fc03(&ok), Some(vec![0x1234, 0x5678]));
        // exception (fc | 0x80)
        let exc = [0, 1, 0, 0, 0, 3, 126, 0x83, 0x02];
        assert_eq!(parse_fc03(&exc), None);
        // short frame
        assert_eq!(parse_fc03(&[0, 1, 0, 0]), None);
        // byte count longer than the frame
        assert_eq!(parse_fc03(&[0, 1, 0, 0, 0, 5, 126, 0x03, 40, 1, 2]), None);
    }

    #[test]
    fn serial_masks_the_last_four() {
        assert_eq!(mask_serial("1234567890"), "123456xxxx");
        assert_eq!(mask_serial("ABCD"), "xxxx");
        assert_eq!(mask_serial("AB"), "xxxx");
    }

    #[test]
    fn ascii_trims_and_keeps_printable() {
        // 'S''M' 'A' ' '
        let regs = [0x534D, 0x4120, 0x0000];
        assert_eq!(ascii_from_regs(&regs), "SMA");
    }

    #[test]
    fn fc03_request_is_well_formed() {
        let r = fc03(7, 126, 40000, 2);
        assert_eq!(&r[0..2], &7u16.to_be_bytes()); // tid
        assert_eq!(&r[2..4], &[0, 0]); // protocol
        assert_eq!(&r[4..6], &[0, 6]); // length
        assert_eq!(r[6], 126); // unit
        assert_eq!(r[7], 0x03); // fc
        assert_eq!(&r[8..10], &40000u16.to_be_bytes());
        assert_eq!(&r[10..12], &2u16.to_be_bytes());
    }
}
