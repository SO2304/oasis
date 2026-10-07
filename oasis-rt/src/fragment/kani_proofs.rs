use super::*;

// Unwind bound >= 9 for the [u8; 4] magic memcmp and the [u8; 8] msg_id copy
// (see mesh_revocation/kani_proofs.rs).

/// PROVE: `parse_ofr1` never panics, and an accepted fragment always lands inside
/// the message (`idx * stride + chunk_len <= total_len`, chunk non-empty), so the
/// reassembler's copy can never go out of bounds or overlap another index.
#[kani::proof]
#[kani::unwind(10)]
fn proof_frag_parse_in_bounds() {
    const N: usize = 40;
    let buf: [u8; N] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= N);
    if let Some((h, chunk)) = parse_ofr1(&buf[..len]) {
        let (t, c, i) = (h.total_len as usize, h.count as usize, h.idx as usize);
        assert!(c >= 1 && c <= MAX_FRAGMENTS && i < c);
        assert!(t >= 1 && t <= MAX_ASSEMBLED);
        let s = t.div_ceil(c);
        assert!(!chunk.is_empty());
        assert!(i * s + chunk.len() <= t);
        // Fragments of the same layout are disjoint: idx i owns [i*s, (i+1)*s).
        assert!(chunk.len() <= s);
    }
}

/// PROVE: for any header layout, the received-bitmap target has exactly `count`
/// bits, so completion requires every index once (count <= 32 fits a u32).
#[kani::proof]
fn proof_frag_completion_mask() {
    let count: u8 = kani::any();
    kani::assume(count >= 1 && count as usize <= MAX_FRAGMENTS);
    let all = if count as usize == 32 { u32::MAX } else { (1u32 << count) - 1 };
    assert_eq!(all.count_ones(), count as u32);
    let idx: u8 = kani::any();
    kani::assume(idx < count);
    assert!(all & (1u32 << idx) != 0);
}

/// Stand-in for SHA-256 in `proof_frag_reassembler_never_panics`: any digest. The
/// bounds property does not depend on the hash value, and the real SHA-256 rounds
/// would dominate the solver. Hash checking itself is unit-tested.
fn any_msg_id(_msg: &[u8]) -> [u8; 8] {
    kani::any()
}

/// PROVE: two arbitrary byte strings pushed from arbitrary origins (3 possible) at
/// arbitrary times never make `Reassembler::push` panic or write outside a slot
/// buffer, and never hold more than `REASSEMBLY_SLOTS` reassemblies.
/// Needs `-Z stubbing`.
#[kani::proof]
#[kani::stub(crate::fragment::msg_id_of, any_msg_id)]
#[kani::unwind(21)]
fn proof_frag_reassembler_never_panics() {
    let mut r = Reassembler::new();
    for _ in 0..2 {
        let buf: [u8; 20] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= 20);
        let o: u8 = kani::any();
        kani::assume(o < 3);
        let _ = r.push([o; 8], kani::any(), &buf[..len]);
        assert!(r.in_use() <= REASSEMBLY_SLOTS);
    }
}
