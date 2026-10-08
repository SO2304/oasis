//! Kani proofs for `OSE1` (C13).
//!
//! ChaCha20-Poly1305 and X25519 do not terminate under CBMC on this machine, so what is
//! proved here is the **framing and the addressing** — the parts that decide whether a
//! blob is even handed to the AEAD. The cryptography is tested in `tests.rs`, including
//! an exhaustive single-byte tamper sweep and five wrong-AAD cases. That split is the
//! same one `quorum` and `actuation` declare, and it is stated rather than implied.

use super::*;

/// `sealed_dest` is total: any byte string either yields a fingerprint or `None`, and it
/// yields one only for a blob long enough to hold a header and a tag. No panic, no slice
/// out of range — which matters because a relay calls this on attacker-supplied bytes.
#[kani::proof]
#[kani::unwind(33)]
fn proof_sealed_dest_total() {
    const N: usize = 32;
    let len: usize = kani::any();
    kani::assume(len <= N);
    let bytes: [u8; N] = kani::any();
    match sealed_dest(&bytes[..len]) {
        Some(fp) => {
            assert!(len >= OSE1_HEADER_LEN + OSE1_TAG_LEN);
            assert!(bytes[0..4] == OSE1_MAGIC);
            // The fingerprint is exactly the header's, byte for byte.
            let mut i = 0usize;
            while i < FP_LEN {
                assert!(fp[i] == bytes[4 + i]);
                i += 1;
            }
        }
        None => {
            assert!(len < OSE1_HEADER_LEN + OSE1_TAG_LEN || bytes[0..4] != OSE1_MAGIC);
        }
    }
}

/// `ose1_len` is monotone and cannot wrap for any plaintext a frame can hold, so a length
/// check on a sealed blob cannot be made to pass by overflow.
#[kani::proof]
fn proof_ose1_len_is_sane() {
    let n: usize = kani::any();
    kani::assume(n <= 4096);
    let a = ose1_len(n);
    assert!(a == OSE1_HEADER_LEN + n + OSE1_TAG_LEN);
    assert!(a > n);
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
    // The trailing four bytes are always zero: the counter owns the whole nonce.
    let mut i = 8usize;
    while i < NONCE_LEN {
        assert!(na[i] == 0);
        i += 1;
    }
}

/// `open` refuses on the destination fingerprint **before** it touches the AEAD, so a
/// node does no cryptographic work for traffic that is not addressed to it. Proved on the
/// addressing path: for any blob whose declared destination differs from ours, the result
/// is `None`.
#[kani::proof]
#[kani::unwind(33)]
fn proof_open_refuses_another_destination_without_crypto() {
    const N: usize = 32;
    let bytes: [u8; N] = kani::any();
    let our_fp: [u8; FP_LEN] = kani::any();
    let key: [u8; 32] = kani::any();
    match sealed_dest(&bytes[..]) {
        // Not ours: the only reachable answer is None, and it is decided by the header.
        Some(dest) if dest != our_fp => {
            assert!(open(&key, our_fp, &[], &bytes[..]).is_none());
        }
        None => {
            assert!(open(&key, our_fp, &[], &bytes[..]).is_none());
        }
        _ => (),
    }
}
