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
use crate::mesh::{mesh_v10_pubkey_from_seed, MeshEdPub, MeshEdSeed, MeshPubRegistry, ED25519_PUB_LEN, FP_LEN, MESH_V0B_NETWORK_LEN};
use crate::modbus_gateway::{OriginRule, RegRule};

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
    /// Per-origin register rules, from `origin_register = <fp>,<addr>,<min>,<max>`.
    ///
    /// An origin named here is governed **only** by its own rules; one named nowhere here
    /// falls back to [`Config::map`]. So adding a line can tighten what one sender may
    /// write and never widen what another may — the property
    /// `mb_adding_a_rule_for_one_origin_does_not_widen_another` pins.
    ///
    /// Why it exists: with one shared map, every key the config authorises can write every
    /// register in it. A pilot wants "this HMI may move axis 1 between 0 and 100, that one
    /// may only reset the counter", which is an authorisation question and not a range
    /// check.
    pub origin_map: Vec<OriginRule>,
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
        let mut peer_pks: Vec<(String, String, String)> = Vec::new();
        let mut revoked_hex: Vec<String> = Vec::new();
        let mut origin_regs: Vec<(String, String, String, String)> = Vec::new();
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
                // origin_register = <fp hex 16>,<addr>,<min>,<max>
                "origin_register" => {
                    let p: Vec<&str> = v.split(',').map(str::trim).collect();
                    if p.len() != 4 {
                        return Err(format!("{path}:{}: origin_register wants fp,addr,min,max", n + 1));
                    }
                    origin_regs.push((p[0].to_string(), p[1].to_string(), p[2].to_string(), p[3].to_string()));
                }
                // revoked = <fp hex 16>
                "revoked" => revoked_hex.push(v.to_string()),
                "operator" => operator_hex.push(v.to_string()),
                "quorum" => quorum = Some(v.trim().to_string()),
                // peer_pk = <fp hex 16>,<Ed25519 public key, 64 hex>[,<PERM|PERM…>]
                //
                // The form to deploy with. `peer` below derives a peer's public key from
                // its **seed**, which is its private key: a gateway configured that way
                // holds the signing key of every node it trusts, the operator's included,
                // so taking the machine-side host yields the ability to forge the orders
                // it is there to check. On one lab host both ends share a directory and it
                // never showed; on two machines (A.6) it is a blocker, which is why this
                // exists. `oasis_mbtcp_revoke --print-pub <seedfile>` prints the hex to
                // put here — not `oasis_fingerprint`, which handles the spore layer's
                // X25519 keys and takes a public key as input rather than deriving one.
                "peer_pk" => {
                    let mut it = v.splitn(3, ',');
                    let fp = it.next().ok_or_else(|| format!("{path}:{}: peer_pk wants fp,pubkey", n + 1))?;
                    let pk = it.next().ok_or_else(|| format!("{path}:{}: peer_pk wants fp,pubkey", n + 1))?;
                    let perms = it.next().unwrap_or("").trim().to_string();
                    peer_pks.push((fp.trim().to_string(), pk.trim().to_string(), perms));
                }
                // peer = <fp hex 16>,<seed file>   (the seed file yields the public key)
                // peer = <fp hex 16>,<seed file>[,<PERM|PERM…>]
                //
                // ⚠️ Keeps a **private** seed in this host's reach. Fine for a single-host
                // test, wrong for a deployment: prefer `peer_pk`.
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

        // The deployable form: a public key, written out, no private material on this
        // host. Same registry, same permission rule — only where the key comes from
        // differs, so nothing about verification changes.
        for (fp_hex, pk_hex, perm_spec) in peer_pks {
            let mut fp = [0u8; FP_LEN];
            parse_hex(&fp_hex, &mut fp).map_err(|e| format!("peer_pk fp: {e}"))?;
            let mut pkb = [0u8; ED25519_PUB_LEN];
            parse_hex(&pk_hex, &mut pkb).map_err(|e| format!("peer_pk {fp_hex}: {e}"))?;
            let pk = MeshEdPub(pkb);
            registry.insert(fp, pk);
            let bits = parse_perms(&perm_spec).map_err(|e| format!("peer_pk {fp_hex}: {e}"))?;
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
            origin_map: {
                let mut v = Vec::with_capacity(origin_regs.len());
                for (fp_hex, a, lo, hi) in &origin_regs {
                    let mut fp = [0u8; FP_LEN];
                    parse_hex(fp_hex, &mut fp).map_err(|e| format!("origin_register {fp_hex}: {e}"))?;
                    let num = |s: &str| -> Result<u16, String> {
                        if let Some(h) = s.strip_prefix("0x") {
                            u16::from_str_radix(h, 16).map_err(|e| e.to_string())
                        } else {
                            s.parse::<u16>().map_err(|e| e.to_string())
                        }
                    };
                    let (addr, min, max) = (num(a)?, num(lo)?, num(hi)?);
                    if min > max {
                        return Err(format!("origin_register {fp_hex} reg {addr}: min {min} > max {max}"));
                    }
                    v.push(OriginRule { origin: fp, rule: RegRule { addr, min, max } });
                }
                v
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, body: &str) -> String {
        let p = std::env::temp_dir().join(format!("oasis_conf_{}_{}", std::process::id(), name));
        fs::write(&p, body).unwrap();
        p.to_str().unwrap().to_string()
    }

    fn seed_bytes(byte: u8) -> [u8; 32] {
        let mut s = [0u8; 32];
        for (i, b) in s.iter_mut().enumerate() {
            *b = byte.wrapping_add(i as u8);
        }
        s
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// `peer_pk` must be exactly equivalent to `peer`, minus the private material.
    ///
    /// The point of `peer_pk` is deployment: `peer` derives a trusted node's public key
    /// from its **seed**, so a gateway configured that way holds the signing key of every
    /// node it trusts — the operator's included — and taking the machine-side host yields
    /// the ability to forge the very orders the gateway exists to check. On one lab host
    /// both ends share a directory and it never showed. This test is what says the safe
    /// form grants the same trust and the same permissions, so switching to it changes
    /// nothing about verification.
    #[test]
    fn peer_pk_is_equivalent_to_peer_without_the_private_seed() {
        let sk = seed_bytes(0xAA);
        let pk = mesh_v10_pubkey_from_seed(&MeshEdSeed(sk)).unwrap();
        let our = tmp("our.seed", &hex(&seed_bytes(0xCC)));
        let theirs = tmp("their.seed", &hex(&sk));

        let common = format!(
            "listen = 127.0.0.1:0\npeer_addr = 127.0.0.1:1\nunit = 0x11\n\
             network_id = 4f415349536e6574\nour_fp = cc00000000000000\n\
             our_seed_file = {}\nregister = 10,0,1000\n",
            our.replace('\\', "/")
        );
        let with_seed = Config::load(&tmp("a.conf", &format!("{common}peer = aa00000000000000,{},ACTUATE\n", theirs.replace('\\', "/")))).unwrap();
        let with_pub = Config::load(&tmp("b.conf", &format!("{common}peer_pk = aa00000000000000,{},ACTUATE\n", hex(&pk.0)))).unwrap();

        let fp: [u8; FP_LEN] = [0xAA, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(with_seed.registry.get(&fp).map(|k| k.0), Some(pk.0), "the seed form trusts this key");
        assert_eq!(with_pub.registry.get(&fp).map(|k| k.0), Some(pk.0), "the public form trusts the same key");
        assert_eq!(with_pub.perms.entries.len(), with_seed.perms.entries.len());
        assert!(with_pub.perms.allows(&fp, perm::ACTUATE), "and grants the same permission");
        assert!(with_seed.perms.allows(&fp, perm::ACTUATE));

        // The whole point: the deployed file contains no private key.
        let text = fs::read_to_string(tmp("b.conf", &format!("{common}peer_pk = aa00000000000000,{},ACTUATE\n", hex(&pk.0)))).unwrap();
        assert!(!text.contains(&hex(&sk)), "a peer_pk config must not carry the peer's seed");
    }

    /// A malformed public key is refused at load, not at the first order.
    #[test]
    fn peer_pk_refuses_a_key_that_is_not_32_bytes() {
        let our = tmp("our2.seed", &hex(&seed_bytes(0xCC)));
        let body = format!(
            "listen = 127.0.0.1:0\npeer_addr = 127.0.0.1:1\nunit = 0x11\n\
             network_id = 4f415349536e6574\nour_fp = cc00000000000000\n\
             our_seed_file = {}\npeer_pk = aa00000000000000,abcd,ACTUATE\n",
            our.replace('\\', "/")
        );
        // Not `unwrap_err()`: `Config` has no `Debug`, deliberately — deriving it would
        // print `our_seed`, a private key, into test output and panic messages.
        let e = match Config::load(&tmp("c.conf", &body)) {
            Err(e) => e,
            Ok(_) => panic!("a 2-byte public key was accepted"),
        };
        assert!(e.contains("peer_pk"), "the error must name the line: {e}");
    }

    /// Fail closed: a `peer_pk` with no permission field grants nothing, exactly as a
    /// `peer` line with none does. The opposite default would hand actuation to every key
    /// in the file.
    #[test]
    fn peer_pk_without_a_permission_grants_nothing() {
        let pk = mesh_v10_pubkey_from_seed(&MeshEdSeed(seed_bytes(0xAA))).unwrap();
        let our = tmp("our3.seed", &hex(&seed_bytes(0xCC)));
        let body = format!(
            "listen = 127.0.0.1:0\npeer_addr = 127.0.0.1:1\nunit = 0x11\n\
             network_id = 4f415349536e6574\nour_fp = cc00000000000000\n\
             our_seed_file = {}\npeer_pk = aa00000000000000,{}\n",
            our.replace('\\', "/"),
            hex(&pk.0)
        );
        let c = Config::load(&tmp("d.conf", &body)).unwrap();
        let fp: [u8; FP_LEN] = [0xAA, 0, 0, 0, 0, 0, 0, 0];
        assert!(c.registry.get(&fp).is_some(), "the key is still trusted for verification");
        assert!(!c.perms.allows(&fp, perm::ACTUATE), "but it may not actuate");
    }
}
