use super::*;
use crate::actuation::{encode_oac1, parse_oac1, ActCommand};
use crate::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed};

const NET: [u8; MESH_V0B_NETWORK_LEN] = *b"OASISnet";

fn fp(i: u8) -> [u8; FP_LEN] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

fn seed(byte: u8) -> MeshEdSeed {
    let mut s = [0u8; 32];
    for (i, b) in s.iter_mut().enumerate() {
        *b = byte.wrapping_add(i as u8);
    }
    MeshEdSeed(s)
}

fn order(seq: u32) -> ActCommand {
    ActCommand {
        actuator_id: 1,
        cmd_seq: seq,
        boot_id: 7,
        deadline_ms: 4_000,
        force: 12.5,
        torque: 0.5,
        velocity: 0.25,
        pos: [1.0, 2.0, 3.0],
    }
}

/// The origin and the destination derive the **same** key from opposite sides, with no
/// exchange: the fingerprints are sorted, so the salt matches.
#[test]
fn seal_key_agrees_from_both_sides() {
    let (a, b) = (seed(1), seed(2));
    let (pa, pb) = (mesh_v10_pubkey_from_seed(&a).unwrap(), mesh_v10_pubkey_from_seed(&b).unwrap());
    let ka = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let kb = seal_key(&b, fp(2), &pa, fp(1)).unwrap();
    assert_eq!(ka, kb, "both ends must derive the same key");

    // A third party derives something else.
    let c = seed(3);
    let kc = seal_key(&c, fp(3), &pa, fp(1)).unwrap();
    assert_ne!(ka, kc);
}

/// And it differs from the per-link MAC key of the same pair, because the domain differs.
/// Otherwise one key would serve two purposes.
#[test]
fn seal_key_differs_from_the_link_key() {
    let (a, b) = (seed(1), seed(2));
    let pb = mesh_v10_pubkey_from_seed(&b).unwrap();
    let sealing = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let linking = crate::mesh::prefilter::link_key(&a, fp(1), &pb, fp(2)).unwrap();
    assert_ne!(sealing, linking, "a sealing key must not equal a MAC key");
}

/// Round trip: an order goes in, the same bytes come out, and the Part F parser accepts
/// them unchanged. The gate never learns that the payload travelled sealed.
#[test]
fn seal_roundtrip_yields_the_original_order() {
    let (a, b) = (seed(1), seed(2));
    let pb = mesh_v10_pubkey_from_seed(&b).unwrap();
    let pa = mesh_v10_pubkey_from_seed(&a).unwrap();
    let key = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let counter = 42u64;
    let aad = seal_aad(&NET, fp(1), fp(2), counter);

    let inner = encode_oac1(&order(10));
    let blob = seal_with_nonce(&key, &nonce_from_counter(counter), fp(2), &aad, &inner).unwrap();
    assert_eq!(blob.len(), ose1_len(inner.len()));
    assert_eq!(blob.len(), 50 + 54, "OSE1 over an OAC1 order: 104 bytes");
    assert_eq!(sealed_dest(&blob), Some(fp(2)));
    assert_ne!(&blob[OSE1_HEADER_LEN..], &inner[..], "the order must not be in clear");

    let dest_key = seal_key(&b, fp(2), &pa, fp(1)).unwrap();
    let opened = open(&dest_key, fp(2), &aad, &blob).expect("the addressee opens it");
    assert_eq!(opened, inner.to_vec());
    assert_eq!(parse_oac1(&opened).unwrap(), order(10));
}

/// A relay in the middle holds no key for traffic it forwards — that is the whole point
/// of an origin-to-destination key rather than the per-link one.
#[test]
fn a_relay_cannot_open_it() {
    let (a, b, r) = (seed(1), seed(2), seed(9));
    let pb = mesh_v10_pubkey_from_seed(&b).unwrap();
    let pa = mesh_v10_pubkey_from_seed(&a).unwrap();
    let key = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let aad = seal_aad(&NET, fp(1), fp(2), 1);
    let blob = seal_with_nonce(&key, &nonce_from_counter(1), fp(2), &aad, b"secret order").unwrap();

    // The relay knows both parties' public keys — it is in the registry — and still
    // cannot derive the pair key, because it has neither private key.
    for (s, f) in [(&r, fp(9))] {
        let rk_a = seal_key(s, f, &pa, fp(1)).unwrap();
        let rk_b = seal_key(s, f, &pb, fp(2)).unwrap();
        assert!(open(&rk_a, fp(2), &aad, &blob).is_none());
        assert!(open(&rk_b, fp(2), &aad, &blob).is_none());
    }
    // It can, however, read who the payload is for: OSE1 hides content, not addressing.
    assert_eq!(sealed_dest(&blob), Some(fp(2)));
}

/// The AAD binds the ciphertext to its envelope. Lift it into another counter, another
/// network or another pair and the AEAD refuses — before the gate is reached.
#[test]
fn a_ciphertext_cannot_be_moved_to_another_envelope() {
    let (a, b) = (seed(1), seed(2));
    let pb = mesh_v10_pubkey_from_seed(&b).unwrap();
    let pa = mesh_v10_pubkey_from_seed(&a).unwrap();
    let key = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let dest_key = seal_key(&b, fp(2), &pa, fp(1)).unwrap();
    let good = seal_aad(&NET, fp(1), fp(2), 7);
    let blob = seal_with_nonce(&key, &nonce_from_counter(7), fp(2), &good, b"open the valve").unwrap();
    assert!(open(&dest_key, fp(2), &good, &blob).is_some());

    let other: [u8; MESH_V0B_NETWORK_LEN] = *b"OTHERnet";
    for bad in [
        seal_aad(&NET, fp(1), fp(2), 8),   // another counter
        seal_aad(&NET, fp(1), fp(2), 6),   // an earlier counter
        seal_aad(&other, fp(1), fp(2), 7), // another network
        seal_aad(&NET, fp(3), fp(2), 7),   // another origin
        seal_aad(&NET, fp(1), fp(3), 7),   // another destination
    ] {
        assert!(open(&dest_key, fp(2), &bad, &blob).is_none(), "a moved ciphertext must not open");
    }
}

/// Addressed to someone else: refused on the fingerprint, before any cryptography.
#[test]
fn a_blob_for_another_node_is_refused() {
    let (a, b) = (seed(1), seed(2));
    let pb = mesh_v10_pubkey_from_seed(&b).unwrap();
    let pa = mesh_v10_pubkey_from_seed(&a).unwrap();
    let key = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let dest_key = seal_key(&b, fp(2), &pa, fp(1)).unwrap();
    let aad = seal_aad(&NET, fp(1), fp(2), 3);
    let blob = seal_with_nonce(&key, &nonce_from_counter(3), fp(2), &aad, b"hello").unwrap();
    assert!(open(&dest_key, fp(5), &aad, &blob).is_none(), "fp(5) is not the addressee");
}

/// Every single-byte corruption is refused, and the parser is total on junk.
#[test]
fn tamper_and_junk_are_refused() {
    let (a, b) = (seed(1), seed(2));
    let pb = mesh_v10_pubkey_from_seed(&b).unwrap();
    let pa = mesh_v10_pubkey_from_seed(&a).unwrap();
    let key = seal_key(&a, fp(1), &pb, fp(2)).unwrap();
    let dest_key = seal_key(&b, fp(2), &pa, fp(1)).unwrap();
    let aad = seal_aad(&NET, fp(1), fp(2), 1);
    let blob = seal_with_nonce(&key, &nonce_from_counter(1), fp(2), &aad, b"payload-1234").unwrap();

    for i in 0..blob.len() {
        let mut bad = blob.clone();
        bad[i] ^= 0x01;
        assert!(open(&dest_key, fp(2), &aad, &bad).is_none(), "byte {i} accepted");
    }
    for junk in [&b""[..], &b"OSE1"[..], &[0u8; 23][..], &[0xFFu8; 200][..]] {
        assert!(sealed_dest(junk).is_none() || open(&dest_key, fp(2), &aad, junk).is_none());
    }
}

/// Sizes, asserted so the radio-budget claim cannot drift: sealing an order costs 40
/// bytes of overhead, and the whole v0B frame stays under the 255-byte PHY limit.
#[test]
fn sealed_sizes_still_fit_one_radio_frame() {
    let v0b = crate::mesh::MESH_V0B_HEADER_LEN;
    assert_eq!(ose1_len(54) - 54, 50, "overhead: 12 OSE1 + 22 AEAD header + 16 tag");
    assert_eq!(v0b + ose1_len(54), 99 + 104, "sealed order on the wire");
    assert_eq!(v0b + ose1_len(54), 203);
    assert!(v0b + ose1_len(54) <= 255, "still one raw-LoRa frame");
    // The compact stop stays small enough too.
    assert_eq!(v0b + ose1_len(11), 160);
}
