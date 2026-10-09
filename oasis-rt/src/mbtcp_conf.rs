//! Configuration shared by `oasis_mbtcp_gateway` and `oasis_mbtcp_agent`.
//!
//! A plain `key = value` file, one per line, `#` for comments. Deliberately not TOML: a
//! parser dependency for twelve keys would be a dependency to audit, and the pilot prompt
//! asks for a configuration file, not a format.
//!
//! **No compiled seed.** Phase 1.2 removed keys from the firmware binary and this keeps
//! that: the seed is read from a file named by the config, so swapping an identity is a
//! file operation and never a rebuild.
//!
//! ⚠️ The seed file is as readable as any file on the host. That is the same limit as
//! C14 on the RP2040 (key readable from flash) and it does not close in software.

use std::collections::BTreeMap;
use std::fs;

use crate::enrollment::{perm, Entry, Registry};
use crate::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed, MeshPubRegistry, FP_LEN, MESH_V0B_NETWORK_LEN};
use crate::modbus_gateway::RegRule;

#[derive(Clone)]
pub struct Config {
    /// Where this program listens: the gateway for agent envelopes, the agent for the HMI.
    pub listen: String,
    /// The gateway's PLC, or the agent's gateway.
    pub plc: String,
    pub unit: u8,
    pub timeout_ms: u64,
    pub network_id: [u8; MESH_V0B_NETWORK_LEN],
    pub our_fp: [u8; FP_LEN],
    pub our_seed: MeshEdSeed,
    /// Peers we accept envelopes from, by fingerprint.
    pub registry: MeshPubRegistry,
    /// The peer we send to (the agent's gateway fingerprint), when there is one.
    pub peer_fp: [u8; FP_LEN],
    /// The logical actuator an order addresses, checked by the gate. Distinct from
    /// `unit`, which is the Modbus address on the PLC's own bus.
    pub gateway_id: u16,
    /// Register map: address → allowed range. Ordered, so a dump is reproducible.
    pub map: Vec<RegRule>,
    /// Fingerprints refused at the gate, from `revoked = <16 hex>` lines.
    ///
    /// ⚠️ This is a **config-time** revocation list: the operator edits the file and
    /// restarts. The mesh carries a signed `ORV1` list with a monotone epoch, persisted
    /// before apply and proved on silicon — and this gateway does **not** handle one over
    /// its link, so that is the gap, not this. Until it does, revoking here means an edit
    /// and a restart, which is slower than a broadcast and auditable in the same way a
    /// config is.
    pub revoked: Vec<[u8; FP_LEN]>,
    /// Who may sign an `ORV1` revocation list arriving over the link.
    ///
    /// One `operator = <64 hex>` line gives a single-key authority. Several give a k-of-n
    /// authority, with `quorum = <k>` saying how many **distinct** keys must sign (default:
    /// all of them, which is the safe reading of an unstated k). `None` refuses every
    /// list — a gateway with no authority configured must not take one from anyone.
    pub authority: Option<oasis_operator_key::OperatorAuthority>,
    /// Per-origin permissions, from the third field of a `peer` line
    /// (`peer = <fp>,<seedfile>,ACTUATE|STOP`).
    ///
    /// The **same** `enrollment::Registry` the firmware consults, so `allows()` means the
    /// same thing on both paths. **Fail closed**: a peer with no third field has no
    /// permissions and its orders are refused, because the alternative default would grant
    /// actuation to every key in the file.
    ///
    /// ⚠️ Config-time, not an owner-signed attestation. `OAU1` enrolment over the link is
    /// the remaining step; until then an operator edits the file and restarts.
    pub perms: Registry,
}

fn parse_hex(s: &str, out: &mut [u8]) -> Result<(), String> {
    let s = s.trim();
    if s.len() != out.len() * 2 {
        return Err(format!("expected {} hex chars, got {}", out.len() * 2, s.len()));
    }
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Config {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut kv: BTreeMap<String, String> = BTreeMap::new();
        let mut regs: Vec<RegRule> = Vec::new();
        let mut peers: Vec<(String, String, String)> = Vec::new();
        let mut revoked_hex: Vec<String> = Vec::new();
        let mut operator_hex: Vec<String> = Vec::new();
        let mut quorum: Option<String> = None;

        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once('=').ok_or_else(|| format!("{path}:{}: no '=' in {line:?}", n + 1))?;
            let (k, v) = (k.trim(), v.trim());
            match k {
                // register = addr,min,max
                "register" => {
                    let p: Vec<&str> = v.split(',').map(str::trim).collect();
                    if p.len() != 3 {
                        return Err(format!("{path}:{}: register wants addr,min,max", n + 1));
                    }
                    let num = |s: &str| -> Result<u16, String> {
                        if let Some(h) = s.strip_prefix("0x") {
                            u16::from_str_radix(h, 16).map_err(|e| e.to_string())
                        } else {
                            s.parse::<u16>().map_err(|e| e.to_string())
                        }
                    };
                    regs.push(RegRule { addr: num(p[0])?, min: num(p[1])?, max: num(p[2])? });
                }
                // revoked = <fp hex 16>
                "revoked" => revoked_hex.push(v.to_string()),
                "operator" => operator_hex.push(v.to_string()),
                "quorum" => quorum = Some(v.trim().to_string()),
                // peer = <fp hex 16>,<seed file>   (the seed file yields the public key)
                // peer = <fp hex 16>,<seed file>[,<PERM|PERM…>]
                "peer" => {
                    let mut it = v.splitn(3, ',');
                    let fp = it.next().ok_or_else(|| format!("{path}:{}: peer wants fp,seedfile", n + 1))?;
                    let seed = it.next().ok_or_else(|| format!("{path}:{}: peer wants fp,seedfile", n + 1))?;
                    let perms = it.next().unwrap_or("").trim().to_string();
                    peers.push((fp.trim().to_string(), seed.trim().to_string(), perms));
                }
                _ => {
                    kv.insert(k.to_string(), v.to_string());
                }
            }
        }

        let need = |k: &str| -> Result<String, String> { kv.get(k).cloned().ok_or_else(|| format!("{path}: missing `{k}`")) };

        let mut network_id = [0u8; MESH_V0B_NETWORK_LEN];
        parse_hex(&need("network_id")?, &mut network_id).map_err(|e| format!("network_id: {e}"))?;
        let mut our_fp = [0u8; FP_LEN];
        parse_hex(&need("our_fp")?, &mut our_fp).map_err(|e| format!("our_fp: {e}"))?;
        let mut peer_fp = [0u8; FP_LEN];
        if let Ok(v) = need("peer_fp") {
            parse_hex(&v, &mut peer_fp).map_err(|e| format!("peer_fp: {e}"))?;
        }

        let seed_path = need("our_seed_file")?;
        let our_seed = read_seed(&seed_path)?;

        let mut registry = MeshPubRegistry::new();
        let mut perms = Registry::default();
        for (fp_hex, seed_file, perm_spec) in peers {
            let mut fp = [0u8; FP_LEN];
            parse_hex(&fp_hex, &mut fp).map_err(|e| format!("peer fp: {e}"))?;
            let s = read_seed(&seed_file)?;
            let pk = mesh_v10_pubkey_from_seed(&s).map_err(|e| format!("peer key: {e}"))?;
            registry.insert(fp, pk);
            let bits = parse_perms(&perm_spec).map_err(|e| format!("peer {fp_hex}: {e}"))?;
            perms.entries.push(Entry { fp, pk: pk.0, role: 0, permissions: bits, seq: 0 });
        }

        Ok(Config {
            listen: need("listen")?,
            plc: need("peer_addr").or_else(|_| need("plc"))?,
            unit: u8::from_str_radix(need("unit")?.trim_start_matches("0x"), 16).map_err(|e| format!("unit: {e}"))?,
            timeout_ms: need("timeout_ms").unwrap_or_else(|_| "2000".into()).parse().unwrap_or(2000),
            network_id,
            our_fp,
            our_seed,
            registry,
            peer_fp,
            gateway_id: need("gateway_id").unwrap_or_else(|_| "1".into()).parse().unwrap_or(1),
            map: regs,
            perms,
            authority: build_authority(&operator_hex, quorum.as_deref())?,
            revoked: {
                let mut v = Vec::new();
                for h in &revoked_hex {
                    let mut fp = [0u8; FP_LEN];
                    parse_hex(h, &mut fp).map_err(|e| format!("revoked {h}: {e}"))?;
                    v.push(fp);
                }
                v
            },
        })
    }
}

/// One `operator` line is a single-key authority; several are a k-of-n one.
///
/// An unstated `quorum` means **all** the configured keys, not one: the safe reading of an
/// omission is the strict one, because the lenient reading would turn a list of five
/// operators into any one of five.
fn build_authority(keys_hex: &[String], quorum: Option<&str>) -> Result<Option<oasis_operator_key::OperatorAuthority>, String> {
    use oasis_operator_key::OperatorAuthority;
    if keys_hex.is_empty() {
        if quorum.is_some() {
            return Err("quorum sans operator: aucune cle a compter".into());
        }
        return Ok(None);
    }
    let mut keys = Vec::with_capacity(keys_hex.len());
    for h in keys_hex {
        let mut k = [0u8; 32];
        parse_hex(h, &mut k).map_err(|e| format!("operator {h}: {e}"))?;
        if keys.contains(&k) {
            return Err(format!("operator {h}: cle en double, un quorum de cles identiques n'en est pas un"));
        }
        keys.push(k);
    }
    let k = match quorum {
        Some(q) => q.parse::<usize>().map_err(|e| format!("quorum: {e}"))?,
        None => keys.len(),
    };
    if k == 0 || k > keys.len() {
        return Err(format!("quorum {k} impossible avec {} cle(s)", keys.len()));
    }
    Ok(Some(if keys.len() == 1 && k == 1 {
        OperatorAuthority::Single { pub_key: keys[0] }
    } else {
        OperatorAuthority::Multisig { pub_keys: keys, k }
    }))
}

/// `ACTUATE|STOP|SUPERVISE`, or empty for none. An unknown name is an error rather than
/// zero bits: a typo in a permission must not silently deny, because the operator would
/// then debug a refusal instead of a spelling mistake.
fn parse_perms(spec: &str) -> Result<u32, String> {
    let mut bits = 0u32;
    for name in spec.split('|') {
        match name.trim() {
            "" => {}
            "ACTUATE" => bits |= perm::ACTUATE,
            "STOP" => bits |= perm::STOP,
            "SUPERVISE" => bits |= perm::SUPERVISE,
            other => return Err(format!("unknown permission {other:?} (want ACTUATE, STOP or SUPERVISE)")),
        }
    }
    Ok(bits)
}

/// A seed file holds 64 hex chars. Reading it rather than compiling it is the point.
pub fn read_seed(path: &str) -> Result<MeshEdSeed, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut s = [0u8; 32];
    parse_hex(text.trim(), &mut s).map_err(|e| format!("{path}: {e}"))?;
    Ok(MeshEdSeed(s))
}
