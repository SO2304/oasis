use super::*;

/// PROVE: an accepted install has an authorized manifest, the right hardware, a
/// bounded non-empty image whose hash matched, an image version equal to the
/// manifest's, at or above the anti-rollback floor and strictly newer than the
/// running firmware.
#[kani::proof]
fn proof_fw_install_requires_all_conditions() {
    let image_version: Option<u32> = if kani::any() { Some(kani::any()) } else { None };
    let i = InstallInput {
        manifest_authorized: kani::any(),
        hw_ok: kani::any(),
        image_len: kani::any(),
        hash_ok: kani::any(),
        image_version,
        manifest_version: kani::any(),
        floor: kani::any(),
        running_version: kani::any(),
    };
    if install_decision(&i).is_ok() {
        assert!(i.manifest_authorized && i.hw_ok && i.hash_ok);
        assert!(i.image_len > 0 && i.image_len as usize <= ACTIVE_MAX);
        assert!(i.image_version == Some(i.manifest_version));
        assert!(i.manifest_version as u64 >= i.floor);
        assert!(i.manifest_version > i.running_version);
    }
}

/// PROVE: the floor never decreases, and covers the confirmed image's version.
#[kani::proof]
fn proof_fw_floor_never_lowers() {
    let floor: u64 = kani::any();
    let v: u32 = kani::any();
    let f = raised_floor(floor, v);
    assert!(f >= floor && f >= v as u64);
}

/// PROVE: the manifest and image-header parsers never panic and accept only exact
/// lengths.
#[kani::proof]
#[kani::unwind(10)]
fn proof_fw_parsers_total() {
    let buf: [u8; MANIFEST_LEN + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= MANIFEST_LEN + 1);
    if parse_manifest(&buf[..len]).is_some() {
        assert!(len == MANIFEST_LEN);
    }
    let img: [u8; FWINFO_OFFSET + FWINFO_LEN] = kani::any();
    let ilen: usize = kani::any();
    kani::assume(ilen <= FWINFO_OFFSET + FWINFO_LEN);
    if image_version(&img[..ilen]).is_some() {
        assert!(ilen == FWINFO_OFFSET + FWINFO_LEN);
    }
}
