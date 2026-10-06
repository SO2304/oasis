use super::*;

/// Flash simulator: two slots, with an optional torn write injected on the next
/// write (models a power loss in the middle of erase+program).
#[derive(Clone)]
struct MockFlash {
    slots: [[u8; TX_LEASE_RECORD_LEN]; 2],
    tear_next: Option<Tear>,
    writes: u64,
}

#[derive(Clone, Copy)]
enum Tear {
    /// Sector erased, nothing programmed.
    Erased,
    /// Sector erased, only the first `k` bytes programmed.
    Partial(usize),
}

impl MockFlash {
    fn new() -> Self {
        MockFlash { slots: [[0xFF; TX_LEASE_RECORD_LEN]; 2], tear_next: None, writes: 0 }
    }
}

impl SlotIo for MockFlash {
    fn read(&self, slot: usize) -> [u8; TX_LEASE_RECORD_LEN] {
        self.slots[slot]
    }
    fn write(&mut self, slot: usize, rec: &[u8; TX_LEASE_RECORD_LEN]) -> bool {
        self.writes += 1;
        if let Some(t) = self.tear_next.take() {
            let mut x = [0xFF; TX_LEASE_RECORD_LEN];
            if let Tear::Partial(k) = t {
                x[..k].copy_from_slice(&rec[..k]);
            }
            self.slots[slot] = x;
            return false; // power lost mid-write: the caller never sees success
        }
        self.slots[slot] = *rec;
        true
    }
}

/// Deterministic xorshift64 for the randomized reboot test.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn lease_fresh_device_starts_at_one() {
    let mut st = DualSlotStore::new(MockFlash::new());
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    assert_eq!(l.next(&mut st), Some(1));
    assert_eq!(l.ceiling(), TX_LEASE_BLOCK);
    assert_eq!(st.load(), Some(TX_LEASE_BLOCK), "ceiling must be durable before counter 1 is used");
}

#[test]
fn lease_resumes_strictly_above_everything_issued() {
    let mut st = DualSlotStore::new(MockFlash::new());
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    let mut last = 0;
    for _ in 0..300 {
        last = l.next(&mut st).unwrap();
    }
    // Reboot: RAM lost, only the store survives.
    let mut l2 = TxLease::boot(&st, TX_LEASE_BLOCK);
    let first = l2.next(&mut st).unwrap();
    assert!(first > last, "resumed counter {} must exceed last issued {}", first, last);
    assert_eq!(first, 2 * TX_LEASE_BLOCK + 1, "resumes at the persisted ceiling (block 2 = 512)");
}

#[test]
fn lease_one_durable_write_per_block() {
    let mut st = DualSlotStore::new(MockFlash::new());
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    let n = 10_000u64;
    for _ in 0..n {
        l.next(&mut st).unwrap();
    }
    // ceil(10_000 / 256) = 40 ceilings for 10_000 messages.
    assert_eq!(l.writes(), (n + TX_LEASE_BLOCK - 1) / TX_LEASE_BLOCK);
    assert_eq!(st.io.writes, l.writes());
}

#[test]
fn lease_torn_ceiling_write_keeps_old_ceiling_valid() {
    for tear in [Tear::Erased, Tear::Partial(3), Tear::Partial(8), Tear::Partial(15)] {
        let mut st = DualSlotStore::new(MockFlash::new());
        let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
        let mut max_issued = 0;
        for _ in 0..TX_LEASE_BLOCK {
            max_issued = l.next(&mut st).unwrap(); // 1..=256, ceiling 256 durable
        }
        // Counter 257 needs ceiling 512: power is lost during that write.
        st.io.tear_next = Some(tear);
        assert_eq!(l.next(&mut st), None, "no counter may be issued above a non-durable ceiling");
        // Reboot: the new ceiling is lost, the old one (256) must still be readable.
        assert_eq!(st.load(), Some(TX_LEASE_BLOCK), "old ceiling must survive the torn write");
        let mut l2 = TxLease::boot(&st, TX_LEASE_BLOCK);
        let first = l2.next(&mut st).unwrap();
        assert!(first > max_issued, "after a torn write the sender must still resume above {}", max_issued);
    }
}

#[test]
fn lease_failed_store_issues_nothing() {
    let mut st = DualSlotStore::new(MockFlash::new());
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    st.io.tear_next = Some(Tear::Erased);
    let before = l;
    assert_eq!(l.next(&mut st), None);
    assert_eq!(l, before, "a failed durable write must leave the lease unchanged");
}

#[test]
fn lease_no_counter_reuse_over_10000_random_reboots() {
    let mut rng = Rng(0x0A51_5EED_1234_5678);
    let mut st = DualSlotStore::new(MockFlash::new());
    let mut max_issued: u64 = 0;
    let mut total_issued: u64 = 0;
    let mut torn: u64 = 0;
    for _ in 0..10_000 {
        let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
        let burst = rng.upto(600);
        for _ in 0..burst {
            // ~2 % of durable writes are torn by a power loss.
            if rng.upto(50) == 0 {
                st.io.tear_next = Some(if rng.upto(2) == 0 { Tear::Erased } else { Tear::Partial(rng.upto(TX_LEASE_RECORD_LEN as u64) as usize) });
            }
            match l.next(&mut st) {
                Some(c) => {
                    assert!(c > max_issued, "counter {} reused or regressed (max so far {})", c, max_issued);
                    max_issued = c;
                    total_issued += 1;
                }
                None => {
                    torn += 1;
                    break; // power lost during the ceiling write: reboot
                }
            }
            st.io.tear_next = None;
        }
    }
    assert!(total_issued > 1_000_000, "the run must actually exercise the lease ({} issued)", total_issued);
    assert!(torn > 0, "the run must include torn ceiling writes");
}

#[test]
fn lease_ensure_with_router_owned_counter() {
    // Integration shape used by the firmware: the router owns tx_counter and the
    // lease only guarantees durability before each emission.
    let mut st = DualSlotStore::new(MockFlash::new());
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    let mut router_ctr = l.resume_point();
    for _ in 0..1000 {
        assert!(l.ensure(router_ctr + 1, &mut st));
        router_ctr += 1;
        l.note_issued(router_ctr);
    }
    let l2 = TxLease::boot(&st, TX_LEASE_BLOCK);
    assert!(l2.resume_point() >= router_ctr, "resume point must cover every emitted counter");
}

#[test]
fn lease_refuses_to_wrap_at_u64_max() {
    struct Mem(Option<u64>);
    impl CeilingStore for Mem {
        fn load(&self) -> Option<u64> {
            self.0
        }
        fn store(&mut self, c: u64) -> bool {
            self.0 = Some(c);
            true
        }
    }
    let mut st = Mem(Some(u64::MAX));
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    assert_eq!(l.next(&mut st), None, "an exhausted counter space must refuse, never wrap to 0");
}

#[test]
fn lease_record_rejects_corruption() {
    let r = encode_record(4242);
    assert_eq!(decode_record(&r), Some(4242));
    for i in 0..TX_LEASE_RECORD_LEN {
        for b in 0..8 {
            let mut x = r;
            x[i] ^= 1 << b;
            assert_eq!(decode_record(&x), None, "flip byte {} bit {} must invalidate the record", i, b);
        }
    }
    assert_eq!(decode_record(&[0xFF; TX_LEASE_RECORD_LEN]), None, "erased slot is not a record");
}
