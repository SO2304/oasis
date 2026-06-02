//! OASIS — Operator key management demo.
//!
//! Demonstrates the full lifecycle of the trust root:
//!
//!   Phase A — COMMISSIONING CEREMONY
//!     Day 0: operator generates the master key (single OR k-of-n
//!     quorum). Pubkey distributed to firmware build pipeline.
//!     Day 1: each node provisioned: SE keyed + op_pub_at_commissioning
//!     written to firmware (in production: OTP fuse / signed boot ROM).
//!     Day 2+: nodes deployed. Their trust root = op_pub_at_commissioning.
//!
//!   Phase B — STEADY STATE
//!     Operator broadcasts authorized actions (revocations, etc.) signed
//!     under the current authority. Receivers verify against op_pub.
//!
//!   Phase C — KEY ROTATION (e.g., HSM end-of-life, suspected compromise)
//!     Operator publishes a Transition envelope: (current_auth_id,
//!     new_authority, retire_at_unix), signed by CURRENT authority.
//!     Receivers verify, then on/after retire_at swap atomically.
//!     Old authority cannot authorize anything thereafter.
//!
//!   Phase D — MULTISIG QUORUM CHANGE
//!     Operator's k-of-n group changes membership (a member leaves or
//!     joins). Treated as a rotation: signed by CURRENT k-of-n quorum,
//!     swaps to NEW k-of-n.
//!
//!   Phase E — POST-COMPROMISE RESPONSE
//!     ONE seed of a 3-of-5 multisig is leaked. Without quorum, the
//!     attacker cannot authorize any action. Operator rotates to a
//!     new 3-of-5 set excluding the compromised seed.

use oasis_operator_key::{
    apply_transition, sign_with_seed, AuthorityError, OperatorAuthority, Pub, Sig, Transition,
};

fn seed(b: u8) -> [u8; 32] {
    [b; 32]
}

fn pub_from(s: &[u8; 32]) -> Pub {
    let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(s).unwrap());
    let mut p = [0u8; 32];
    p.copy_from_slice(kp.pk.as_ref());
    p
}

fn h(s: &str) {
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" {}", s);
    println!("──────────────────────────────────────────────────────────────────");
}

fn ok(msg: &str) {
    println!("  [OK]  {}", msg);
}
fn fail(msg: &str) {
    println!("  [FAIL] {}", msg);
}
fn info(msg: &str) {
    println!("  [..]  {}", msg);
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  OASIS Operator Key — commissioning & ongoing management         ║");
    println!("║  trust root lifecycle: bootstrap → steady → rotate → respond     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    let mut score: u32 = 0;
    let mut total: u32 = 0;

    // ── PHASE A — COMMISSIONING CEREMONY ─────────────────────────
    h("Phase A — Commissioning ceremony");
    info("Day 0: operator generates master key.");
    info("       Strategy: 3-of-5 multisig (no single seed = full authority)");
    let s1 = seed(0xA1);
    let s2 = seed(0xA2);
    let s3 = seed(0xA3);
    let s4 = seed(0xA4);
    let s5 = seed(0xA5);
    let initial_auth = OperatorAuthority::multisig_from_seeds(&[s1, s2, s3, s4, s5], 3).unwrap();
    info(&format!(
        "       authority: 3-of-5 multisig with 5 distinct pubkeys"
    ));

    info("Day 1: each node firmware build embeds initial_auth as op_pub_at_commissioning.");
    info("       In production: OTP fuse / signed boot ROM / SE protected zone.");
    let mut node_trust_root = initial_auth.clone();
    info("       Node N's `trust_root` field initialized at commissioning.");

    info("Day 2+: nodes deployed, trust root in effect.");
    ok("commissioning ceremony complete");

    // ── PHASE B — STEADY STATE ───────────────────────────────────
    h("Phase B — Steady state: operator authorizes a revocation");
    let revocation_msg = b"REVOKE_BATCH_001: fp_C, fp_X, fp_Z (3 entries)";
    info("Operator's signing ceremony: 3 of the 5 quorum members each");
    info("produce a signature in their respective HSM. Sigs gathered + broadcast.");
    let sigs_3of5: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, revocation_msg)),
        (pub_from(&s2), sign_with_seed(&s2, revocation_msg)),
        (pub_from(&s4), sign_with_seed(&s4, revocation_msg)), // 1, 2, 4 (skip 3)
    ];
    total += 1;
    match node_trust_root.verify_authorization(revocation_msg, &sigs_3of5) {
        Ok(_) => {
            ok("revocation accepted (3-of-5 quorum met)");
            score += 1;
        }
        Err(e) => fail(&format!("unexpected reject: {:?}", e)),
    }

    h("Phase B — Adversary tries with only 2 sigs");
    let sigs_2of5: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, revocation_msg)),
        (pub_from(&s2), sign_with_seed(&s2, revocation_msg)),
    ];
    total += 1;
    match node_trust_root.verify_authorization(revocation_msg, &sigs_2of5) {
        Err(AuthorityError::QuorumNotMet {
            provided: 2,
            required: 3,
        }) => {
            ok("2-of-5 REJECTED (quorum not met)");
            score += 1;
        }
        other => fail(&format!("expected QuorumNotMet, got: {:?}", other)),
    }

    h("Phase B — Adversary tries with 3 sigs but ONE is duplicated");
    let sig_s1 = sign_with_seed(&s1, revocation_msg);
    let sigs_dup: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sig_s1),
        (pub_from(&s1), sig_s1), // SAME signer twice
        (pub_from(&s2), sign_with_seed(&s2, revocation_msg)),
    ];
    total += 1;
    match node_trust_root.verify_authorization(revocation_msg, &sigs_dup) {
        Err(AuthorityError::DuplicateSigner) => {
            ok("duplicate-signer attack REJECTED");
            score += 1;
        }
        other => fail(&format!("expected DuplicateSigner, got: {:?}", other)),
    }

    h("Phase B — Adversary tries with 3 sigs, one from OUTSIDE the quorum");
    let s_outside = seed(0xFF);
    let sigs_outside: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, revocation_msg)),
        (pub_from(&s2), sign_with_seed(&s2, revocation_msg)),
        (
            pub_from(&s_outside),
            sign_with_seed(&s_outside, revocation_msg),
        ),
    ];
    total += 1;
    match node_trust_root.verify_authorization(revocation_msg, &sigs_outside) {
        Err(AuthorityError::UnknownSigner) => {
            ok("unknown signer REJECTED (not in authority's pubkey set)");
            score += 1;
        }
        other => fail(&format!("expected UnknownSigner, got: {:?}", other)),
    }

    // ── PHASE C — KEY ROTATION ────────────────────────────────────
    h("Phase C — Key rotation (HSM EOL: replace seed s5 → s5_new)");
    info("Operator decides to rotate. Reason: HSM holding s5 reaches");
    info("end-of-life. New seed s5_new generated. Quorum: 3 of CURRENT 5");
    info("must sign the Transition envelope.");

    let s5_new = seed(0xB5);
    let new_auth = OperatorAuthority::multisig_from_seeds(&[s1, s2, s3, s4, s5_new], 3).unwrap();
    let transition = Transition {
        retire_at_unix: 1746883200,
        new_authority: new_auth.clone(),
    };
    let trans_msg = transition.signing_message();
    let trans_sigs: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, &trans_msg)),
        (pub_from(&s3), sign_with_seed(&s3, &trans_msg)),
        (pub_from(&s5), sign_with_seed(&s5, &trans_msg)), // s5 signs its own retirement
    ];
    info("Transition signed by s1, s3, s5 (3-of-5 of CURRENT authority).");
    total += 1;
    match apply_transition(&mut node_trust_root, &transition, &trans_sigs) {
        Ok(_) => {
            ok("Transition applied — node trust root now uses s5_new instead of s5");
            score += 1;
        }
        Err(e) => fail(&format!("rotation failed: {:?}", e)),
    }
    info(&format!(
        "post-rotation authority: {:?}",
        match &node_trust_root {
            OperatorAuthority::Multisig { k, pub_keys } =>
                format!("Multisig k={} n={}", k, pub_keys.len()),
            _ => "??".into(),
        }
    ));

    h("Phase C — Old s5 tries to authorize POST-rotation: must FAIL");
    let post_rotation_action = b"REVOKE_BATCH_002: fp_Q";
    let sigs_with_old_s5: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, post_rotation_action)),
        (pub_from(&s2), sign_with_seed(&s2, post_rotation_action)),
        (pub_from(&s5), sign_with_seed(&s5, post_rotation_action)), // OLD s5
    ];
    total += 1;
    match node_trust_root.verify_authorization(post_rotation_action, &sigs_with_old_s5) {
        Err(AuthorityError::UnknownSigner) => {
            ok("retired s5 REJECTED post-rotation (forward-secrecy of authorization)");
            score += 1;
        }
        other => fail(&format!(
            "expected UnknownSigner for retired s5, got: {:?}",
            other
        )),
    }

    h("Phase C — New s5_new + s1 + s2 sign post-rotation: must SUCCEED");
    let sigs_with_new_s5: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, post_rotation_action)),
        (pub_from(&s2), sign_with_seed(&s2, post_rotation_action)),
        (
            pub_from(&s5_new),
            sign_with_seed(&s5_new, post_rotation_action),
        ),
    ];
    total += 1;
    match node_trust_root.verify_authorization(post_rotation_action, &sigs_with_new_s5) {
        Ok(_) => {
            ok("new authority accepted — rotation atomic, fleet upgrade complete");
            score += 1;
        }
        Err(e) => fail(&format!("new authority rejected: {:?}", e)),
    }

    // ── PHASE D — POST-COMPROMISE RESPONSE ───────────────────────
    h("Phase D — Compromise of one quorum member (s2 stolen)");
    info("Attacker has s2's seed. Tries to forge a revocation alone:");
    let attack_msg = b"REVOKE_BATCH_003: fp_OPERATOR_OWN (attempted DoS)";
    let attacker_sigs: Vec<(Pub, Sig)> = vec![(pub_from(&s2), sign_with_seed(&s2, attack_msg))];
    total += 1;
    match node_trust_root.verify_authorization(attack_msg, &attacker_sigs) {
        Err(AuthorityError::QuorumNotMet {
            provided: 1,
            required: 3,
        }) => {
            ok("1-seed compromise INSUFFICIENT for any action (k-of-n threshold)");
            score += 1;
        }
        other => fail(&format!("expected QuorumNotMet 1/3, got: {:?}", other)),
    }

    info("Operator response: rotate again, EXCLUDE compromised s2.");
    let s2_replacement = seed(0xC2);
    let post_compromise_auth =
        OperatorAuthority::multisig_from_seeds(&[s1, s2_replacement, s3, s4, s5_new], 3).unwrap();
    let t3 = Transition {
        retire_at_unix: 1746883300,
        new_authority: post_compromise_auth.clone(),
    };
    let t3_msg = t3.signing_message();
    let t3_sigs: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, &t3_msg)),
        (pub_from(&s3), sign_with_seed(&s3, &t3_msg)),
        (pub_from(&s4), sign_with_seed(&s4, &t3_msg)),
    ];
    total += 1;
    match apply_transition(&mut node_trust_root, &t3, &t3_sigs) {
        Ok(_) => {
            ok("post-compromise rotation: s2 retired, s2_replacement enrolled");
            score += 1;
        }
        Err(e) => fail(&format!("post-compromise rotation failed: {:?}", e)),
    }

    info("Attacker (still holding old s2) tries again — but s2 not in current authority:");
    let post_recovery_attack: Vec<(Pub, Sig)> = vec![
        (pub_from(&s1), sign_with_seed(&s1, attack_msg)),
        (pub_from(&s2), sign_with_seed(&s2, attack_msg)), // stolen, but retired
        (pub_from(&s3), sign_with_seed(&s3, attack_msg)),
    ];
    total += 1;
    match node_trust_root.verify_authorization(attack_msg, &post_recovery_attack) {
        Err(AuthorityError::UnknownSigner) => {
            ok("post-recovery attack REJECTED: stolen s2 no longer in authority set");
            score += 1;
        }
        other => fail(&format!("expected UnknownSigner, got: {:?}", other)),
    }

    // ── Summary ───────────────────────────────────────────────────
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!(" Operator key lifecycle summary");
    println!("──────────────────────────────────────────────────────────────────");
    println!("  Test results: {} / {} PASS", score, total);
    println!();
    println!("  Coverage:");
    println!("    - Commissioning: 3-of-5 multisig embedded in node firmware");
    println!("    - Steady state: legit 3-of-5 authorization accepted");
    println!("    - Quorum failure: 2 sigs rejected (k=3 threshold)");
    println!("    - Duplicate-signer attack: rejected");
    println!("    - Outside-signer attack: rejected");
    println!("    - Rotation: signed transition swaps authority atomically");
    println!("    - Forward-secrecy: retired key rejected for new actions");
    println!("    - Post-compromise: 1-seed leak insufficient + rotation excludes");
    println!();
    if score == total {
        println!("  All cases produced expected outcomes.");
        std::process::exit(0);
    } else {
        eprintln!("  {} unexpected failures.", total - score);
        std::process::exit(1);
    }
}
