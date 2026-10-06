use super::*;

/// In-memory store with a symbolic success flag (no SHA-256 to unroll).
struct MemStore {
    v: Option<u64>,
    ok: bool,
}
impl CeilingStore for MemStore {
    fn load(&self) -> Option<u64> {
        self.v
    }
    fn store(&mut self, c: u64) -> bool {
        if self.ok {
            self.v = Some(c);
            true
        } else {
            false
        }
    }
}

/// PROVE: the counter issued after a reboot is strictly greater than any counter
/// issued before it. Before the reboot every issued counter was `<=` the durable
/// ceiling (proved by `proof_tx_lease_never_issues_above_durable_ceiling`), so
/// "issued <= persisted" is the only assumption.
#[kani::proof]
fn proof_tx_lease_resume_strictly_above_issued() {
    let persisted: u64 = kani::any();
    kani::assume(persisted < u64::MAX - TX_LEASE_BLOCK);
    let issued_before: u64 = kani::any();
    kani::assume(issued_before <= persisted);
    let mut st = MemStore { v: Some(persisted), ok: true };
    let mut l = TxLease::boot(&st, TX_LEASE_BLOCK);
    let first = l.next(&mut st).unwrap();
    assert!(first > issued_before);
}

/// PROVE: `next` never returns a counter that is not covered by the durable
/// ceiling, whatever the store does (success or failure).
#[kani::proof]
fn proof_tx_lease_never_issues_above_durable_ceiling() {
    let ceiling: u64 = kani::any();
    let last: u64 = kani::any();
    kani::assume(last <= ceiling);
    kani::assume(ceiling < u64::MAX - TX_LEASE_BLOCK);
    let mut st = MemStore { v: Some(ceiling), ok: kani::any() };
    let mut l = TxLease { last, ceiling, block: TX_LEASE_BLOCK, writes: 0 };
    if let Some(x) = l.next(&mut st) {
        let durable = st.v.unwrap();
        assert!(x <= durable, "issued counter must be covered by the durable ceiling");
        assert!(x > last, "issued counters strictly increase");
    }
}

/// PROVE: a failed durable write issues nothing and leaves the lease unchanged.
#[kani::proof]
fn proof_tx_lease_failed_store_issues_nothing() {
    let ceiling: u64 = kani::any();
    kani::assume(ceiling < u64::MAX - TX_LEASE_BLOCK);
    let mut st = MemStore { v: Some(ceiling), ok: false };
    let mut l = TxLease { last: ceiling, ceiling, block: TX_LEASE_BLOCK, writes: 0 };
    let before = l;
    assert!(l.next(&mut st).is_none());
    assert!(l == before);
    assert!(st.v == Some(ceiling));
}
