//! OASIS LoRa-mesh POC — software validation (host PC).
//!
//!   cd oasis-lora-transport
//!   cargo run --release --features std --example mesh_poc
//!
//! Validates the OASIS IoT stack END-TO-END in software, over a SIMULATED LoRa
//! radio: Ed25519-sign (mesh v0A) -> LoRa frame -> "transmit" -> receive ->
//! deframe -> VERIFY. Also prints the real LoRa time-on-air of the packet, proves
//! a forged sender is rejected, and shows the EU868 1% duty-cycle budget.
//!
//! HONEST SCOPE: this is the PRE-HARDWARE check. It exercises the full logic
//! before you wire a real SX1262. It does NOT touch a real radio — the SX1262
//! driver is still a stub. Output is ASCII-only (Windows console safe).

use oasis_lora_transport::airtime::DutyCycleThrottler;
use oasis_lora_transport::sim::SimulatedLoRaRadio;
use oasis_lora_transport::{LoRaParams, LoRaTransport};
use oasis_rt::mesh::{
    mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
};

fn fp(i: u8) -> [u8; 8] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

fn main() {
    println!("== OASIS LoRa-mesh POC (logiciel, radio simulee) ==\n");

    // [1] Two nodes with Ed25519 identities; each registers the other's pubkey.
    let seed_a = MeshEdSeed([0x0A; 32]);
    let seed_b = MeshEdSeed([0x0B; 32]);
    let pk_a = mesh_v10_pubkey_from_seed(&seed_a).unwrap();
    let pk_b = mesh_v10_pubkey_from_seed(&seed_b).unwrap();
    let mut reg_a = MeshPubRegistry::new();
    reg_a.insert(fp(0xBB), pk_b);
    let mut reg_b = MeshPubRegistry::new();
    reg_b.insert(fp(0xAA), pk_a);
    let mut node_a = MeshRouter::new_ed25519_signed(fp(0xAA), seed_a, reg_a);
    let mut node_b = MeshRouter::new_ed25519_signed(fp(0xBB), seed_b, reg_b);
    println!("[1] 2 noeuds Ed25519 (A=0xAA, B=0xBB), pubkeys croisees enregistrees.");

    // [2] Link them over a SIMULATED LoRa radio (SF7 / 125 kHz by default).
    let (radio_a, radio_b) = SimulatedLoRaRadio::linked_pair(42);
    let mut tx = LoRaTransport::new(radio_a, LoRaParams::default()).unwrap();
    let mut rx = LoRaTransport::new(radio_b, LoRaParams::default()).unwrap();
    println!(
        "[2] Lien LoRa simule (SF{}, BW code {}).",
        tx.params().sf,
        tx.params().bw_code
    );

    // [3] A signs a v0A mesh envelope and "transmits" it.
    let payload = b"telemetry: temp=21C batt=88%";
    let envelope = node_a.origin_wrap(payload);
    let air_ms = tx.airtime_us(envelope.len()) as f64 / 1000.0;
    tx.send_envelope(&envelope).unwrap();
    println!(
        "[3] A signe + emet ({} o charge -> {} o enveloppe, airtime {:.1} ms).",
        payload.len(),
        envelope.len(),
        air_ms
    );

    // [4] B receives, deframes, and VERIFIES the Ed25519 signature.
    let mut buf = [0u8; 255];
    let env_rx = rx.recv_envelope(&mut buf, 500).unwrap();
    match node_b.process(env_rx) {
        MeshDecision::Arrived {
            msg_id, hops_seen, ..
        } => println!(
            "[4] B recoit + VERIFIE la signature: ARRIVED (msg_id={:#018x}, hops={}).",
            msg_id, hops_seen
        ),
        MeshDecision::Drop(why) => {
            println!("[4] ECHEC inattendu: B rejette le message legitime ({why}).");
            std::process::exit(1);
        }
    }

    // [5] Security: a FORGED sender spoofing A's fingerprint must be rejected.
    //     (Advance its counter so a fresh msg_id reaches the signature check,
    //      not the dedup cache.)
    let mut attacker =
        MeshRouter::new_ed25519_signed(fp(0xAA), MeshEdSeed([0xEE; 32]), MeshPubRegistry::new());
    for _ in 0..3 {
        let _ = attacker.origin_wrap(b"warm");
    }
    let forged = attacker.origin_wrap(b"FORGED: takeover node B");
    let (rax, rbx) = SimulatedLoRaRadio::linked_pair(99);
    let mut tx2 = LoRaTransport::new(rax, LoRaParams::default()).unwrap();
    let mut rx2 = LoRaTransport::new(rbx, LoRaParams::default()).unwrap();
    tx2.send_envelope(&forged).unwrap();
    let env_f = rx2.recv_envelope(&mut buf, 500).unwrap();
    match node_b.process(env_f) {
        MeshDecision::Drop(why) => {
            println!("[5] Securite: enveloppe FORGEE rejetee ({why}). OK.")
        }
        MeshDecision::Arrived { .. } => {
            println!("[5] FAILLE: la forge a ete acceptee !");
            std::process::exit(1);
        }
    }

    // [6] Duty cycle: how many legal sends at EU868 1%, 1 msg/s for 50 s?
    let mut dc = DutyCycleThrottler::eu868_1pct(0);
    let air_us = tx.airtime_us(envelope.len());
    let (mut now, mut legal) = (0u64, 0u32);
    for _ in 0..50 {
        if dc.try_send(air_us, now) {
            legal += 1;
        }
        now += 1000;
    }
    println!("[6] EU868 1%: {legal}/50 envois legaux a 1 msg/s (le reste throttle).");

    println!("\n== POC logiciel OK. Etape suivante: ecrire le driver SX1262 reel, puis flasher 3 Pico. ==");
}
