//! Unit tests for the pilot's agent half: the persisted sequence and the promise that
//! the HMI is never left in silence.
use super::agent::{exception_for, SeqStore, SEQ_LEASE};
use crate::modbus_tcp::EXC_GATEWAY_PATH_UNAVAILABLE;

fn scratch(name: &str) -> String {
    let p = std::env::temp_dir().join(format!("oasis_seqstore_{}_{}.seq", std::process::id(), name));
    let _ = std::fs::remove_file(&p);
    p.to_str().unwrap().to_string()
}

/// The property the agent's header claims and nothing checked until now: a number is
/// claimed on disk **before** the caller has it, so a crash between reserving and sending
/// can only lose a sequence. Losing one is harmless — the gateway wants strictly newer,
/// not consecutive — while reusing one is a replay it would have to accept.
///
/// Stated as the property and not as an exact file content, which is what the first
/// version of this test did: it asserted the file read back `"1"` after handing out 1, and
/// that assertion was about the implementation, so leasing broke it while the guarantee
/// was untouched. The invariant is `handed_out <= claimed_on_disk`, at every step.
#[test]
fn seq_store_claims_on_disk_before_returning_and_never_repeats() {
    let p = scratch("persist");
    let mut s = SeqStore::load(&p).unwrap();
    assert_eq!(s.peek(), 1, "a missing file is a first run");

    let mut seen = Vec::new();
    for _ in 0..5 {
        let n = s.reserve().unwrap();
        assert!(n <= s.claimed(), "handed out {n} but the file only claims {}", s.claimed());
        let on_disk: u32 = std::fs::read_to_string(&p).unwrap().trim().parse().unwrap();
        assert!(n <= on_disk, "handed out {n} while the file says {on_disk}");
        seen.push(n);
    }
    assert_eq!(seen, [1, 2, 3, 4, 5], "within a run the sequence is consecutive");

    // A restart, which is the case that matters: whatever it hands out must be strictly
    // greater than everything the previous run did, gap or no gap.
    let mut after = SeqStore::load(&p).unwrap();
    let first_after = after.reserve().unwrap();
    assert!(first_after > *seen.last().unwrap(), "a restart must not reuse {}", seen.last().unwrap());
    assert!(!seen.contains(&first_after), "and must not reuse any earlier number");
    let _ = std::fs::remove_file(&p);
}

/// Leasing is only sound if one write really does cover a whole run of numbers: that is
/// the entire reason it is cheaper. Counting the writes is the only way to know.
#[test]
fn seq_store_writes_once_per_lease_not_once_per_order() {
    let p = scratch("lease");
    let mut s = SeqStore::load(&p).unwrap();
    let mut writes = 0;
    let mut last_claim = 0;
    for _ in 0..(SEQ_LEASE * 2) {
        s.reserve().unwrap();
        if s.claimed() != last_claim {
            writes += 1;
            last_claim = s.claimed();
        }
    }
    assert_eq!(writes, 2, "{} orders must cost exactly 2 disk writes", SEQ_LEASE * 2);
    let _ = std::fs::remove_file(&p);
}

/// A corrupt file is an error, not a guess. Starting over at 1 would hand the gateway
/// sequences it has already executed.
#[test]
fn seq_store_refuses_a_corrupt_file() {
    let p = scratch("corrupt");
    std::fs::write(&p, "not a number").unwrap();
    assert!(SeqStore::load(&p).is_err());
    let _ = std::fs::remove_file(&p);
}

/// "Never silence towards the HMI" has to hold for a frame too short to parse, which
/// is where an exception builder normally panics on a slice.
#[test]
fn exception_for_answers_even_a_truncated_frame() {
    for len in 0..9usize {
        let req: Vec<u8> = (0..len).map(|i| i as u8 + 1).collect();
        let e = exception_for(&req, EXC_GATEWAY_PATH_UNAVAILABLE);
        assert_eq!(e.len(), 9);
        assert_eq!(e[8], EXC_GATEWAY_PATH_UNAVAILABLE);
        assert_eq!(e[7] & 0x80, 0x80, "an exception sets the high bit of the function code");
        assert_eq!(e[5], 3, "MBAP length covers unit + fc + code");
    }
    // And for a complete request it echoes the transaction id, so the HMI can match it.
    let req = [0xAB, 0xCD, 0, 0, 0, 6, 0x11, 0x06, 0, 10, 0, 1];
    let e = exception_for(&req, crate::modbus_tcp::EXC_ILLEGAL_ADDRESS);
    assert_eq!(&e[0..2], &[0xAB, 0xCD]);
    assert_eq!(e[6], 0x11, "unit echoed");
    assert_eq!(e[7], 0x86, "FC06 with the high bit");
}
