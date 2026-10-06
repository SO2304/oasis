use super::*;
use crate::authority::{kind, verify_authority, AuthPolicy, AuthReject, SUITE_ED25519, SUITE_HYBRID};
use crate::test_support::{o1, o2, o3, NET};
use sha2::{Digest, Sha256};

/// A fake image: vector table filler, then the version header, then code bytes.
fn image(version: u32, len: usize) -> Vec<u8> {
    let mut v: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(7)).collect();
    v[FWINFO_OFFSET..FWINFO_OFFSET + FWINFO_LEN].copy_from_slice(&encode_fwinfo(version));
    v
}

fn manifest_for(img: &[u8], version: u32) -> Manifest {
    Manifest { version, image_len: img.len() as u32, sha256: Sha256::digest(img).into(), hw_id: HW_ID }
}

/// What the node does: verify the OAU1 manifest with the owner's keys, then apply
/// the rule against the uploaded image.
fn node_install(msg: &[u8], owner: &crate::test_support::TestOwner, uploaded: &[u8], floor: u64, running: u32) -> Result<(), UpdateReject> {
    let p = match verify_authority(&AuthPolicy::default(), &NET, &owner.keys(), msg) {
        Ok(p) if p.kind == kind::FIRMWARE_MANIFEST => p,
        _ => return Err(UpdateReject::NotAuthorized),
    };
    let m = parse_manifest(p.content).ok_or(UpdateReject::Malformed)?;
    let n = (m.image_len as usize).min(uploaded.len());
    install_decision(&InstallInput {
        manifest_authorized: true,
        hw_ok: m.hw_id == HW_ID,
        image_len: m.image_len,
        hash_ok: m.image_len as usize <= uploaded.len() && <[u8; 32]>::from(Sha256::digest(&uploaded[..n])) == m.sha256,
        image_version: image_version(&uploaded[..n]),
        manifest_version: m.version,
        floor,
        running_version: running,
    })
}

#[test]
fn fw_codec_and_header() {
    let img = image(7, 1000);
    assert_eq!(image_version(&img), Some(7));
    assert_eq!(image_version(&img[..FWINFO_OFFSET + FWINFO_LEN - 1]), None, "truncated");
    let mut bad = img.clone();
    bad[FWINFO_OFFSET] = b'X';
    assert_eq!(image_version(&bad), None, "no magic");
    let m = manifest_for(&img, 7);
    assert_eq!(parse_manifest(&encode_manifest(&m)), Some(m));
    assert_eq!(parse_manifest(&encode_manifest(&m)[..MANIFEST_LEN - 1]), None);
}

#[test]
fn fw_install_rule_each_refusal() {
    let ok = InstallInput {
        manifest_authorized: true,
        hw_ok: true,
        image_len: 300_000,
        hash_ok: true,
        image_version: Some(2),
        manifest_version: 2,
        floor: 1,
        running_version: 1,
    };
    assert_eq!(install_decision(&ok), Ok(()));
    let r = |f: &dyn Fn(&mut InstallInput)| {
        let mut i = ok;
        f(&mut i);
        install_decision(&i)
    };
    assert_eq!(r(&|i| i.manifest_authorized = false), Err(UpdateReject::NotAuthorized));
    assert_eq!(r(&|i| i.hw_ok = false), Err(UpdateReject::WrongHardware));
    assert_eq!(r(&|i| i.image_len = 0), Err(UpdateReject::TooLarge));
    assert_eq!(r(&|i| i.image_len = ACTIVE_MAX as u32 + 1), Err(UpdateReject::TooLarge));
    assert_eq!(r(&|i| i.image_len = ACTIVE_MAX as u32), Ok(()));
    assert_eq!(r(&|i| i.hash_ok = false), Err(UpdateReject::HashMismatch));
    assert_eq!(r(&|i| i.image_version = None), Err(UpdateReject::VersionMismatch));
    assert_eq!(r(&|i| i.image_version = Some(3)), Err(UpdateReject::VersionMismatch));
    // Old image: below the floor (rollback) or not above the running version.
    assert_eq!(r(&|i| i.floor = 3), Err(UpdateReject::Rollback));
    assert_eq!(r(&|i| i.running_version = 2), Err(UpdateReject::NotNewer));
    // After a cut between "floor raised" and "mark_booted": v1 runs with floor 2;
    // reinstalling v2 is allowed (>= floor and > running), v1 is not.
    assert_eq!(r(&|i| i.floor = 2), Ok(()));
    assert_eq!(
        r(&|i| {
            i.floor = 2;
            i.manifest_version = 1;
            i.image_version = Some(1);
        }),
        Err(UpdateReject::Rollback)
    );
    assert_eq!(raised_floor(1, 2), 2);
    assert_eq!(raised_floor(5, 2), 5, "never lowered");
}

#[test]
fn fw_end_to_end_signed_manifests() {
    let v2 = image(2, 4096);
    let m2 = encode_manifest(&manifest_for(&v2, 2));
    let by_o2 = o2().sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &NET, &m2);
    // Valid: signed by the current owner (o2), image matches, newer, above floor.
    assert_eq!(node_install(&by_o2, &o2(), &v2, 1, 1), Ok(()));
    // Modified image after signing.
    let mut tampered = v2.clone();
    tampered[3000] ^= 0x01;
    assert_eq!(node_install(&by_o2, &o2(), &tampered, 1, 1), Err(UpdateReject::HashMismatch));
    // Another signer (o3), and the previous owner (o1) after a transfer to o2.
    let by_o3 = o3().sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &NET, &m2);
    assert_eq!(node_install(&by_o3, &o2(), &v2, 1, 1), Err(UpdateReject::NotAuthorized));
    let by_o1 = o1().sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &NET, &m2);
    assert_eq!(node_install(&by_o1, &o2(), &v2, 1, 1), Err(UpdateReject::NotAuthorized));
    // Old, correctly signed image once v2 runs: refused.
    let v1 = image(1, 4096);
    let m1 = o2().sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &NET, &encode_manifest(&manifest_for(&v1, 1)));
    assert_eq!(node_install(&m1, &o2(), &v1, 2, 2), Err(UpdateReject::Rollback));
    // A manifest that relabels an image (says v3, image header says v2): refused.
    let relabel = o2().sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &NET, &encode_manifest(&manifest_for(&v2, 3)));
    assert_eq!(node_install(&relabel, &o2(), &v2, 2, 2), Err(UpdateReject::VersionMismatch));
    // Ed25519-only manifest: a downgrade (kind 3 is hybrid by default since 1.3).
    let ed_only = o2().sign(kind::FIRMWARE_MANIFEST, SUITE_ED25519, &NET, &m2);
    assert_eq!(verify_authority(&AuthPolicy::default(), &NET, &o2().keys(), &ed_only), Err(AuthReject::Downgrade));
    // Wrong board class.
    let mut other_hw = manifest_for(&v2, 2);
    other_hw.hw_id = *b"STM32F4X";
    let m = o2().sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &NET, &encode_manifest(&other_hw));
    assert_eq!(node_install(&m, &o2(), &v2, 1, 1), Err(UpdateReject::WrongHardware));
}
