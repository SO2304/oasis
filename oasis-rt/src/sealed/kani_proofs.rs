//! Kani proofs for `OSE1` (C13).
//!
//! ChaCha20-Poly1305 and X25519 do not terminate under CBMC on this machine, so what is
//! proved here is the **framing and the addressing** — the parts that decide whether a
//! blob is even handed to the AEAD. The cryptography is tested in `tests.rs`, including
//! an exhaustive single-byte tamper sweep and five wrong-AAD cases. That split is the
//! same one `quorum` and `actuation` declare, and it is stated rather than implied.
//!
//! ⚠️ **Two errata from the first run (`dd993b5`), both in these harnesses and not in the
//! code.** Kani refuted `proof_ose1_len_is_sane` and `proof_sealed_dest_total` because
//! they still asserted the old formula `OSE1_HEADER_LEN + n + OSE1_TAG_LEN`; the layout
//! had been corrected to wrap the AEAD envelope, making the overhead
//! `OSE1_OVERHEAD = 50`, and I had updated the module and the tests but not the proofs.
//! The proofs were wrong and the code was right.
//!
//! Worse, and only visible once that was fixed: the buffers were `[u8; 32]`, **below the
//! 50-byte minimum of a sealed blob**, so `sealed_dest` could only ever return `None` and
//! the `Some` branches were unreachable — `proof_open_refuses_another_destination_without_crypto`
//! was passing **vacuously**. `BUF` is now 64, and each harness asserts that its
//! interesting branch is reachable rather than trusting that it is.

use super::*;

/// Big enough for a well-formed blob: `ose1_len(0)` is 50, so 64 leaves room for a
/// payload and keeps the `Some` branch reachable.
const BUF: usize = 64;

/// `sealed_dest` is total: any byte string either yields a fingerprint or `None`, and it
/// yields one only for a blob long enough to hold a header, an AEAD envelope and a tag.
/// No panic, no slice out of range — which matters because a relay calls this on
/// attacker-supplied bytes.
#[kani::proof]
#[kani::unwind(65)]
fn proof_sealed_dest_total() {
    let len: usize = kani::any();
    kani::assume(len <= BUF);
    let bytes: [u8; BUF] = kani::any();
    match sealed_dest(&bytes[..len]) {
        Some(fp) => {
            assert!(len >= ose1_len(0));
            assert!(bytes[0..4] == OSE1_MAGIC);
            // The fingerprint is exactly the header's, byte for byte.
            let mut i = 0usize;
            while i < FP_LEN {
                assert!(fp[i] == bytes[4 + i]);
                i += 1;
            }
        }
        None => {
            assert!(len < ose1_len(0) || bytes[0..4] != OSE1_MAGIC);
        }
    }
}

/// The `Some` branch above is reachable: a long-enough buffer with the right magic does
/// yield a fingerprint. Without this, the harness could pass while testing nothing — the
/// mistake the first version actually made.
#[kani::proof]
#[kani::unwind(65)]
fn proof_sealed_dest_accepts_a_wellformed_blob() {
    let mut bytes: [u8; BUF] = kani::any();
    bytes[0] = OSE1_MAGIC[0];
    bytes[1] = OSE1_MAGIC[1];
    bytes[2] = OSE1_MAGIC[2];
    bytes[3] = OSE1_MAGIC[3];
    let len: usize = kani::any();
    kani::assume(len >= ose1_len(0) && len <= BUF);
    let got = sealed_dest(&bytes[..len]);
    assert!(got.is_some(), "a long-enough blob with the right magic must be addressed");
    let fp = got.unwrap();
    let mut i = 0usize;
    while i < FP_LEN {
        assert!(fp[i] == bytes[4 + i]);
        i += 1;
    }
}

/// `ose1_len` is monotone and cannot wrap for any plaintext a frame can hold, so a length
/// check on a sealed blob cannot be made to pass by overflow.
#[kani::proof]
fn proof_ose1_len_is_sane() {
    let n: usize = kani::any();
    kani::assume(n <= 4096);
    let a = ose1_len(n);
    assert!(a == OSE1_OVERHEAD + n);
    assert!(a > n);
    assert!(OSE1_OVERHEAD >= OSE1_HEADER_LEN + OSE1_TAG_LEN);
    let m: usize = kani::any();
    kani::assume(m <= 4096);
    kani::assume(m > n);
    assert!(ose1_len(m) > a);
}

/// The nonce is a function of the counter alone, and distinct counters give distinct
/// nonces. That is the property that makes a persisted counter a safe nonce source for
/// ChaCha20-Poly1305, where a repeat is fatal.
#[kani::proof]
fn proof_nonce_is_injective_in_the_counter() {
    let a: u64 = kani::any();
    let b: u64 = kani::any();
    let na = nonce_from_counter(a);
    let nb = nonce_from_counter(b);
    if a == b {
        assert!(na == nb);
    } else {
        let mut differs = false;
        let mut i = 0usize;
        while i < 8 {
            if na[i] != nb[i] {
                differs = true;
            }
            i += 1;
        }
        assert!(differs, "two counters must not share a nonce");
    }
    // The trailing bytes are always zero: the counter owns the whole nonce.
    let mut i = 8usize;
    while i < NONCE_LEN {
        assert!(na[i] == 0);
        i += 1;
    }
}

/// `open` refuses on the destination fingerprint **before** it touches the AEAD, so a
/// node does no cryptographic work for traffic that is not addressed to it. The buffer is
/// large enough for the addressing branch to be the one exercised.
#[kani::proof]
#[kani::unwind(65)]
fn proof_open_refuses_another_destination_without_crypto() {
    let mut bytes: [u8; BUF] = kani::any();
    bytes[0] = OSE1_MAGIC[0];
    bytes[1] = OSE1_MAGIC[1];
    bytes[2] = OSE1_MAGIC[2];
    bytes[3] = OSE1_MAGIC[3];
    let our_fp: [u8; FP_LEN] = kani::any();
    let key: [u8; 32] = kani::any();
    // A well-formed, long-enough blob whose declared destination is NOT ours.
    let dest = sealed_dest(&bytes[..]);
    assert!(dest.is_some(), "the blob must be addressed for this harness to mean anything");
    kani::assume(dest.unwrap() != our_fp);
    assert!(open(&key, our_fp, &[], &bytes[..]).is_none());
}
