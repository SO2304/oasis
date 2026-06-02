//! Demo: SE-backed signing → tamper detected → wipe → all subsequent
//! ops fail. The graceful-degradation pattern that links this Gap 4
//! work to the L5 layer of the lab-pack v2 demo (Vitality + KillSwitch).

use oasis_secure_element::{SecureElement, TamperReason};
use oasis_secure_element::sim::SimSecureElement;
use oasis_secure_element::sign_v10_envelope_via_se;

fn main() {
    println!();
    println!("OASIS Secure Element — tamper wipe demo");
    println!("─────────────────────────────────────────");
    println!();

    // Step 1: provision a fresh SE with a deterministic seed.
    let mut se = SimSecureElement::new();
    se.provision(&[0xAB; 32]).unwrap();
    println!("[1] SE provisioned. wiped? {}", se.is_wiped());
    println!("    pubkey = {:02x?}...", &se.pubkey().unwrap()[..8]);

    // Step 2: sign a few envelopes — normal operation.
    for msg_id in [0x100u64, 0x101, 0x102] {
        let sig = sign_v10_envelope_via_se(&se, msg_id, [0xAA; 8]).unwrap();
        println!("[2] sign(msg_id={:#x}) = {:02x?}...", msg_id, &sig[..8]);
    }

    // Step 3: simulated chassis switch fires — TAMP0 asserts.
    println!();
    println!("[3] *** CHASSIS SWITCH ASSERTED — simulated tamper event ***");
    se.wipe(TamperReason::ChassisSwitch).unwrap();
    println!("    SE wiped. is_wiped() = {}", se.is_wiped());
    println!("    wipe_reason = {:?}", se.wipe_reason());

    // Step 4: any subsequent sign/pubkey returns Err(Wiped).
    println!();
    println!("[4] Post-tamper operations:");
    println!("    pubkey() = {:?}", se.pubkey());
    println!("    sign(...) = {:?}",
             sign_v10_envelope_via_se(&se, 0x103, [0xAA; 8]));
    println!("    re-provision attempt = {:?}", se.provision(&[0x77; 32]));

    println!();
    println!("Result: a captured/tampered node CANNOT continue to sign mesh");
    println!("envelopes. Combined with the L5 KillSwitch + the v6 revocation");
    println!("envelope path, this is the OASIS anti-tampering chain.");
    println!();
    println!("Next round: wire the chassis switch GPIO IRQ → SE.wipe() →");
    println!("broadcast revocation envelope on all mesh links → atomize");
    println!("(per R20 in CLAUDE.md inviolable rules).");
}
