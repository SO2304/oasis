use super::*;

const A: Fp = [0xAA; 8];
const B: Fp = [0xBB; 8];
const C: Fp = [0xCC; 8];

fn msg(n: usize, salt: u8) -> Vec<u8> {
    (0..n).map(|i| (i as u8).wrapping_mul(31) ^ salt).collect()
}

fn feed(r: &mut Reassembler, origin: Fp, frags: &[Vec<u8>], order: &[usize]) -> FragOutcome {
    let mut last = FragOutcome::Incomplete;
    for &i in order {
        last = r.push(origin, 0, &frags[i]);
    }
    last
}

#[test]
fn frag_in_order_roundtrip() {
    let m = msg(2526, 1);
    let f = fragment(&m, 300).unwrap();
    assert_eq!(f.len(), 9, "2526 B / 284 B chunks");
    assert!(f.iter().all(|x| x.len() <= 300));
    let mut r = Reassembler::new();
    let order: Vec<usize> = (0..f.len()).collect();
    assert_eq!(feed(&mut r, A, &f, &order), FragOutcome::Complete(m));
    assert_eq!(r.in_use(), 0, "slot freed after completion");
}

#[test]
fn frag_out_of_order_and_duplicates() {
    let m = msg(1000, 2);
    let f = fragment(&m, 100).unwrap();
    let mut r = Reassembler::new();
    let n = f.len();
    for i in (1..n).rev() {
        assert_eq!(r.push(A, 0, &f[i]), FragOutcome::Incomplete);
        assert_eq!(r.push(A, 0, &f[i]), FragOutcome::Duplicate);
    }
    assert_eq!(r.push(A, 0, &f[0]), FragOutcome::Complete(m));
}

#[test]
fn frag_missing_fragment_never_completes_and_times_out() {
    let m = msg(600, 3);
    let f = fragment(&m, 200).unwrap();
    let mut r = Reassembler::new();
    for x in f.iter().skip(1) {
        assert_eq!(r.push(A, 1_000, x), FragOutcome::Incomplete);
    }
    assert_eq!(r.in_use(), 1);
    // After the timeout the unfinished slot is dropped; a late fragment 0
    // starts a new reassembly instead of completing a stale one.
    assert_eq!(r.push(A, 1_000 + REASSEMBLY_TIMEOUT_MS + 1, &f[0]), FragOutcome::Incomplete);
    assert_eq!(r.in_use(), 1);
}

#[test]
fn frag_two_origins_interleaved() {
    let (ma, mb) = (msg(900, 4), msg(700, 5));
    let (fa, fb) = (fragment(&ma, 128).unwrap(), fragment(&mb, 128).unwrap());
    let mut r = Reassembler::new();
    let mut done = Vec::new();
    for i in 0..fa.len().max(fb.len()) {
        for (o, f) in [(A, &fa), (B, &fb)] {
            if let Some(x) = f.get(i) {
                if let FragOutcome::Complete(v) = r.push(o, 0, x) {
                    done.push(v);
                }
            }
        }
    }
    assert_eq!(done.len(), 2);
    assert!(done.contains(&ma) && done.contains(&mb));
}

#[test]
fn frag_third_origin_gets_no_slot() {
    let f = |s| fragment(&msg(500, s), 100).unwrap();
    let mut r = Reassembler::new();
    assert_eq!(r.push(A, 0, &f(1)[0]), FragOutcome::Incomplete);
    assert_eq!(r.push(B, 0, &f(2)[0]), FragOutcome::Incomplete);
    assert_eq!(r.push(C, 0, &f(3)[0]), FragOutcome::Rejected(FragReject::NoSlot));
}

#[test]
fn frag_new_message_from_same_origin_replaces_old() {
    let (m1, m2) = (msg(500, 6), msg(500, 7));
    let (f1, f2) = (fragment(&m1, 100).unwrap(), fragment(&m2, 100).unwrap());
    let mut r = Reassembler::new();
    assert_eq!(r.push(A, 0, &f1[0]), FragOutcome::Incomplete);
    let order: Vec<usize> = (0..f2.len()).collect();
    assert_eq!(feed(&mut r, A, &f2, &order), FragOutcome::Complete(m2));
    assert_eq!(r.in_use(), 0, "the one unfinished m1 slot was replaced, not kept");
}

#[test]
fn frag_inconsistent_header_cancels_slot() {
    let m = msg(500, 8);
    let f = fragment(&m, 100).unwrap();
    let mut r = Reassembler::new();
    assert_eq!(r.push(A, 0, &f[0]), FragOutcome::Incomplete);
    // Same msg_id, different count: a self-consistent fragment for another
    // layout (count 2, stride 250, idx 1 → 250 bytes).
    let mut bad = f[1][..FRAG_HEADER_LEN].to_vec();
    bad[14] = 1;
    bad[15] = 2;
    bad.extend_from_slice(&m[250..]);
    assert!(parse_ofr1(&bad).is_some());
    assert_eq!(r.push(A, 0, &bad), FragOutcome::Rejected(FragReject::Malformed));
    assert_eq!(r.in_use(), 0);
}

#[test]
fn frag_tampered_chunk_fails_hash() {
    let m = msg(800, 9);
    let mut f = fragment(&m, 100).unwrap();
    let n = f[3].len();
    f[3][n - 1] ^= 0x01;
    let mut r = Reassembler::new();
    let order: Vec<usize> = (0..f.len()).collect();
    assert_eq!(feed(&mut r, A, &f, &order), FragOutcome::Rejected(FragReject::HashMismatch));
    assert_eq!(r.in_use(), 0);
}

#[test]
fn frag_limits() {
    assert!(fragment(&[], 100).is_none());
    assert!(fragment(&msg(MAX_ASSEMBLED + 1, 0), 300).is_none());
    assert!(fragment(&msg(100, 0), FRAG_HEADER_LEN).is_none());
    assert!(fragment(&msg(MAX_ASSEMBLED, 0), 50).is_none(), "more than 32 fragments");
    let f = fragment(&msg(MAX_ASSEMBLED, 0), 300).unwrap();
    assert!(f.len() <= MAX_FRAGMENTS);
    // Every produced fragment parses and the chunks tile the message exactly.
    let total: usize = f.iter().map(|x| parse_ofr1(x).unwrap().1.len()).sum();
    assert_eq!(total, MAX_ASSEMBLED);
}

#[test]
fn frag_parse_rejects_bad_headers() {
    let f = fragment(&msg(500, 1), 100).unwrap();
    let base = f[1].clone();
    let mutate = |i: usize, v: u8| {
        let mut x = base.clone();
        x[i] = v;
        x
    };
    assert!(parse_ofr1(&mutate(0, b'X')).is_none(), "magic");
    assert!(parse_ofr1(&mutate(15, 0)).is_none(), "count 0");
    assert!(parse_ofr1(&mutate(15, 33)).is_none(), "count > 32");
    assert!(parse_ofr1(&mutate(14, base[15])).is_none(), "idx >= count");
    assert!(parse_ofr1(&[&base[..], &[0u8][..]].concat()).is_none(), "chunk too long");
    assert!(parse_ofr1(&base[..base.len() - 1]).is_none(), "chunk too short");
    assert!(parse_ofr1(&base[..FRAG_HEADER_LEN]).is_none(), "empty chunk");
    let mut big = base.clone();
    big[12..14].copy_from_slice(&((MAX_ASSEMBLED + 1) as u16).to_le_bytes());
    assert!(parse_ofr1(&big).is_none(), "total_len too large");
}

#[test]
fn frag_push_never_panics_on_garbage() {
    let mut r = Reassembler::new();
    let mut x = 0x9E37_79B9u32;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    for _ in 0..20_000 {
        let len = (next() % 80) as usize;
        let mut b: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        if len >= 16 && next() % 2 == 0 {
            b[..4].copy_from_slice(&OFR1_MAGIC);
            b[15] = (next() % 4) as u8 + 1;
            b[14] = (next() % 4) as u8;
            let t = (next() % 200) as u16 + 1;
            b[12..14].copy_from_slice(&t.to_le_bytes());
        }
        let o = [(next() % 3) as u8; 8];
        let _ = r.push(o, next() as u64 % 100_000, &b);
        assert!(r.in_use() <= REASSEMBLY_SLOTS);
    }
}
