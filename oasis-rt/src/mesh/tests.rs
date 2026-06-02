use super::*;

fn fp(i: u8) -> [u8; FP_LEN] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

// ── AC round (Bloom auto-reset) ──────────────────────────────
//
// Validates the new bloom_auto_reset_threshold contract:
//  - threshold = None  → never auto-resets (legacy behaviour)
//  - threshold = Some(n) → bloom_inserts_since_reset stays bounded
//  - reset preserves seen_set (short-term replay still rejected)
//  - reset_count increments on every reset (manual + auto)
//
// These tests target the hot-path boundary; the 7-day compressed
// soak in oasis-trl-harness then exercises the integrated behavior.

#[test]
fn bloom_auto_reset_default_off() {
    let mut r = MeshRouter::new(fp(1));
    assert_eq!(r.bloom_auto_reset_threshold(), None, "default: no auto-reset configured");
    for _ in 0..1000 {
        let _ = r.origin_wrap(b"x");
    }
    assert_eq!(r.bloom_reset_count(), 0, "no auto-reset means counter never increments");
    assert_eq!(r.bloom_inserts(), 1000);
    assert_eq!(r.bloom_inserts_since_reset(), 1000);
}

#[test]
fn bloom_auto_reset_fires_at_threshold() {
    let mut r = MeshRouter::new(fp(1));
    r.set_bloom_auto_reset_threshold(Some(100));
    for _ in 0..1000 {
        let _ = r.origin_wrap(b"x");
    }
    // 1000 inserts / 100 threshold = ~10 resets fired.
    assert!(r.bloom_reset_count() >= 9 && r.bloom_reset_count() <= 11, "expected ~10 auto-resets, got {}", r.bloom_reset_count());
    // bloom_inserts_since_reset bounded by threshold + 1
    assert!(r.bloom_inserts_since_reset() <= 101, "bloom_inserts_since_reset must stay bounded by threshold +1, got {}", r.bloom_inserts_since_reset());
}

#[test]
fn bloom_auto_reset_preserves_short_term_dedup() {
    // Short-term dedup (seen_set, 4096-entry LRU) must survive
    // Bloom auto-reset. Replay of recent msg_ids still rejected.
    let mut origin = MeshRouter::new(fp(1));
    let mut hop = MeshRouter::new(fp(2));
    hop.set_bloom_auto_reset_threshold(Some(50));
    // First send goes through.
    let env = origin.origin_wrap(b"first");
    match hop.process(&env) {
        MeshDecision::Arrived { .. } => {}
        d => panic!("expected first envelope to arrive, got {:?}", d),
    }
    // Send 200 more envelopes (triggers ~4 auto-resets at threshold 50)
    for _ in 0..200 {
        let e = origin.origin_wrap(b"x");
        let _ = hop.process(&e);
    }
    assert!(hop.bloom_reset_count() >= 3, "auto-reset should have fired, got {}", hop.bloom_reset_count());
    // Replay the FIRST envelope — still rejected by seen_set even though
    // the Bloom was reset multiple times in between.
    match hop.process(&env) {
        MeshDecision::Drop("duplicate") => {}
        d => panic!("first envelope replay should be rejected by seen_set, got {:?}", d),
    }
}

#[test]
fn bloom_health_snapshot_initial_state() {
    let r = MeshRouter::new(fp(1));
    let snap = r.bloom_health_snapshot();
    assert_eq!(snap.bloom_inserts_total, 0);
    assert_eq!(snap.bloom_inserts_since_reset, 0);
    assert_eq!(snap.bloom_reset_count, 0);
    assert_eq!(snap.auto_reset_threshold, None);
    // Pre-computed capacity constant should match feature flag.
    #[cfg(not(feature = "mesh_bloom_mcu"))]
    assert_eq!(snap.bloom_capacity_estimate_1pct_fpr, 52_000);
    #[cfg(feature = "mesh_bloom_mcu")]
    assert_eq!(snap.bloom_capacity_estimate_1pct_fpr, 1_600);
    // Initial state: 0% capacity consumed, no alert.
    assert_eq!(snap.capacity_consumed_milli(), 0);
    assert!(!snap.capacity_alert());
}

#[test]
fn bloom_health_snapshot_tracks_inserts() {
    let mut r = MeshRouter::new(fp(1));
    for _ in 0..100 {
        let _ = r.origin_wrap(b"x");
    }
    let snap = r.bloom_health_snapshot();
    assert_eq!(snap.bloom_inserts_total, 100);
    assert_eq!(snap.bloom_inserts_since_reset, 100);
    // 100 inserts vs default capacity 52 000:
    // capacity_consumed_milli = 100 * 1000 / 52 000 ≈ 1
    #[cfg(not(feature = "mesh_bloom_mcu"))]
    {
        assert!(snap.capacity_consumed_milli() <= 2, "100 / 52_000 should be ≈ 1.9 milli-units, got {}", snap.capacity_consumed_milli());
        assert!(!snap.capacity_alert());
    }
}

#[test]
fn bloom_health_snapshot_alert_fires_at_80pct() {
    let mut r = MeshRouter::new(fp(1));
    // Manually drive bloom_inserts_since_reset to 80% of capacity.
    // Capacity = 52 000 (default) → 80% = 41 600.
    // Easiest path: do 41 600 inserts.
    let target = (bloom_capacity_estimate_1pct_fpr() as u64 * 800) / 1000;
    for _ in 0..target {
        let _ = r.origin_wrap(b"x");
    }
    let snap = r.bloom_health_snapshot();
    assert!(snap.capacity_consumed_milli() >= 799 && snap.capacity_consumed_milli() <= 801, "expected ~800 milli-units, got {}", snap.capacity_consumed_milli());
    assert!(snap.capacity_alert(), "alert MUST fire at 80% of capacity");
}

#[test]
fn bloom_health_snapshot_after_auto_reset() {
    let mut r = MeshRouter::new(fp(1));
    r.set_bloom_auto_reset_threshold(Some(50));
    for _ in 0..200 {
        let _ = r.origin_wrap(b"x");
    }
    let snap = r.bloom_health_snapshot();
    assert_eq!(snap.bloom_inserts_total, 200);
    assert!(snap.bloom_inserts_since_reset <= 51, "since_reset must stay bounded by threshold + 1");
    assert_eq!(snap.auto_reset_threshold, Some(50));
    assert!(snap.bloom_reset_count >= 3, "auto-reset should have fired ~4 times");
    // Capacity-consumed should be tiny because reset cleared progress
    assert!(snap.capacity_consumed_milli() < 5, "post-reset, capacity-consumed should be near 0, got {}", snap.capacity_consumed_milli());
    assert!(!snap.capacity_alert());
}

#[test]
fn bloom_snapshot_serialize_size_is_36() {
    let r = MeshRouter::new(fp(1));
    let snap = r.bloom_health_snapshot();
    let wire = snap.serialize_topic_v1();
    assert_eq!(wire.len(), 36, "wire format v1 is exactly 36 bytes");
    assert_eq!(wire[0], b'B', "magic byte 0 is 'B'");
    assert_eq!(wire[1], 1, "version byte 1 is 1");
    assert_eq!(wire[2], 0, "reserved byte 2 is 0");
    assert_eq!(wire[3], 0, "reserved byte 3 is 0");
}

#[test]
fn bloom_snapshot_serialize_roundtrip_preserves_fields() {
    let mut r = MeshRouter::new(fp(1));
    r.set_bloom_auto_reset_threshold(Some(12_345));
    for _ in 0..7_000 {
        let _ = r.origin_wrap(b"x");
    }
    let snap = r.bloom_health_snapshot();
    let wire = snap.serialize_topic_v1();
    let local_cap = snap.bloom_capacity_estimate_1pct_fpr;
    let restored = BloomHealthSnapshot::deserialize_topic_v1(&wire, local_cap).expect("valid wire roundtrip");
    // All wire-carried fields preserved exactly.
    assert_eq!(restored.bloom_inserts_total, snap.bloom_inserts_total);
    assert_eq!(restored.bloom_inserts_since_reset, snap.bloom_inserts_since_reset);
    assert_eq!(restored.bloom_reset_count, snap.bloom_reset_count);
    assert_eq!(restored.auto_reset_threshold, snap.auto_reset_threshold);
    // Capacity restored from caller-supplied local estimate.
    assert_eq!(restored.bloom_capacity_estimate_1pct_fpr, local_cap);
    // Full equality
    assert_eq!(restored, snap);
}

#[test]
fn bloom_snapshot_serialize_none_threshold_encodes_zero() {
    let r = MeshRouter::new(fp(1));
    let snap = r.bloom_health_snapshot();
    assert_eq!(snap.auto_reset_threshold, None);
    let wire = snap.serialize_topic_v1();
    // bytes 28..36 are threshold; None encodes as 0
    let threshold_wire = u64::from_le_bytes(wire[28..36].try_into().unwrap());
    assert_eq!(threshold_wire, 0, "None auto_reset_threshold encodes as 0");
    // Roundtrip preserves None
    let restored = BloomHealthSnapshot::deserialize_topic_v1(&wire, 52_000).unwrap();
    assert_eq!(restored.auto_reset_threshold, None);
}

#[test]
fn bloom_snapshot_deserialize_rejects_bad_magic() {
    let mut wire = [0u8; 36];
    wire[0] = b'X'; // wrong magic
    wire[1] = 1;
    assert!(BloomHealthSnapshot::deserialize_topic_v1(&wire, 52_000).is_none());
}

#[test]
fn bloom_snapshot_deserialize_rejects_wrong_version() {
    let mut wire = [0u8; 36];
    wire[0] = b'B';
    wire[1] = 99; // wrong version
    assert!(BloomHealthSnapshot::deserialize_topic_v1(&wire, 52_000).is_none());
}

#[test]
fn bloom_snapshot_deserialize_rejects_nonzero_reserved() {
    let mut wire = [0u8; 36];
    wire[0] = b'B';
    wire[1] = 1;
    wire[2] = 0xAA; // reserved byte non-zero
    assert!(BloomHealthSnapshot::deserialize_topic_v1(&wire, 52_000).is_none());
}

#[test]
fn bloom_capacity_estimate_matches_feature_flag() {
    let estimate = bloom_capacity_estimate_1pct_fpr();
    #[cfg(not(feature = "mesh_bloom_mcu"))]
    assert_eq!(estimate, 52_000, "default 64 KiB Bloom → ~52 k inserts");
    #[cfg(feature = "mesh_bloom_mcu")]
    assert_eq!(estimate, 1_600, "MCU 2 KiB Bloom → ~1.6 k inserts");
}

#[test]
fn bloom_reset_count_monotonic() {
    let mut r = MeshRouter::new(fp(1));
    let c0 = r.bloom_reset_count();
    r.bloom_reset();
    assert_eq!(r.bloom_reset_count(), c0 + 1);
    r.bloom_reset();
    assert_eq!(r.bloom_reset_count(), c0 + 2);
    r.set_bloom_auto_reset_threshold(Some(10));
    for _ in 0..30 {
        let _ = r.origin_wrap(b"y");
    }
    assert!(r.bloom_reset_count() >= c0 + 4, "manual + auto-resets accumulate monotonically, got {}", r.bloom_reset_count());
}

#[test]
fn origin_wrap_parse_roundtrip() {
    let mut r = MeshRouter::new(fp(1));
    let inner = b"hello mesh world";
    let env = r.origin_wrap(inner);
    assert!(env.starts_with(SPORE_V8_MAGIC));
    let hdr = parse_envelope(&env).unwrap();
    assert_eq!(hdr.origin_fp, fp(1));
    assert_eq!(hdr.ttl, DEFAULT_TTL);
    assert_eq!(hdr.hops_so_far, 0);
    assert_eq!(hdr.inner, inner);
}

#[test]
fn invariant_forward_decrements_ttl_and_increments_hops() {
    let mut origin = MeshRouter::new(fp(1));
    let mut hop_a = MeshRouter::new(fp(2));
    let env = origin.origin_wrap(b"payload");
    match hop_a.process(&env) {
        MeshDecision::Arrived { envelope, hops_seen, forward, .. } => {
            assert!(forward, "hop sees forward=true when TTL > 0");
            assert_eq!(hops_seen, 0, "first hop sees hops=0 (just emitted)");
            let fwd = parse_envelope(&envelope).unwrap();
            assert_eq!(fwd.ttl, DEFAULT_TTL - 1);
            assert_eq!(fwd.hops_so_far, 1);
        }
        other => panic!("expected Arrived{{forward=true}}, got {:?}", other),
    }
}

#[test]
fn invariant_ttl_zero_is_terminal() {
    let mut origin = MeshRouter::with_config(fp(1), 1, 16);
    let mut hop_a = MeshRouter::new(fp(2));
    let env_ttl1 = origin.origin_wrap(b"x"); // ttl = 1
    let fwd_bytes = match hop_a.process(&env_ttl1) {
        MeshDecision::Arrived { envelope, forward: true, .. } => envelope,
        other => panic!("expected Arrived{{forward=true}} at hop A, got {:?}", other),
    };
    let mut hop_b = MeshRouter::new(fp(3));
    match hop_b.process(&fwd_bytes) {
        MeshDecision::Arrived { hops_seen, forward: false, .. } => {
            assert_eq!(hops_seen, 1);
        }
        other => panic!("expected Arrived{{forward=false}} (ttl=0), got {:?}", other),
    }
}

#[test]
fn invariant_duplicate_msg_id_dropped() {
    let mut origin = MeshRouter::new(fp(1));
    let mut hop = MeshRouter::new(fp(2));
    let env = origin.origin_wrap(b"x");
    let first = hop.process(&env);
    assert!(matches!(first, MeshDecision::Arrived { forward: true, .. }));
    let second = hop.process(&env);
    assert!(matches!(second, MeshDecision::Drop("duplicate")));
}

#[test]
fn invariant_own_echo_dropped() {
    let mut origin = MeshRouter::new(fp(1));
    let env = origin.origin_wrap(b"x");
    // Simulate: the envelope comes back to origin via a neighbour
    let d = origin.process(&env);
    assert!(matches!(d, MeshDecision::Drop("own echo")), "origin receiving its own broadcast must drop: {:?}", d);
}

#[test]
fn invariant_hops_increase_along_chain() {
    let mut origin = MeshRouter::new(fp(1));
    let mut a = MeshRouter::new(fp(2));
    let mut b = MeshRouter::new(fp(3));
    let mut c = MeshRouter::new(fp(4));
    let env_origin = origin.origin_wrap(b"relay me");
    let env_after_a = match a.process(&env_origin) {
        MeshDecision::Arrived { envelope, hops_seen, forward: true, .. } => {
            assert_eq!(hops_seen, 0);
            envelope
        }
        other => panic!("unexpected {:?}", other),
    };
    let env_after_b = match b.process(&env_after_a) {
        MeshDecision::Arrived { envelope, hops_seen, forward: true, .. } => {
            assert_eq!(hops_seen, 1);
            envelope
        }
        other => panic!("unexpected {:?}", other),
    };
    match c.process(&env_after_b) {
        MeshDecision::Arrived { hops_seen, forward: true, .. } => {
            assert_eq!(hops_seen, 2, "C sees 2 hops already traveled");
        }
        other => panic!("unexpected {:?}", other),
    }
}

#[test]
fn invariant_inner_preserved_through_hops() {
    let mut origin = MeshRouter::new(fp(1));
    let mut a = MeshRouter::new(fp(2));
    let mut b = MeshRouter::new(fp(3));
    let payload = b"OASIS digest: sensor_trace..xyz..abc";
    let e0 = origin.origin_wrap(payload);
    let e1 = match a.process(&e0) {
        MeshDecision::Arrived { envelope, .. } => {
            assert_eq!(inner_slice(&envelope), payload);
            envelope
        }
        _ => unreachable!(),
    };
    match b.process(&e1) {
        MeshDecision::Arrived { envelope, .. } => {
            assert_eq!(inner_slice(&envelope), payload, "inner MUST be byte-identical through N hops");
        }
        _ => unreachable!(),
    }
}

#[test]
fn process_owned_fast_path_no_double_copy() {
    // Verify process_owned returns the SAME envelope buffer we gave it,
    // just with TTL/hops mutated. Zero extra allocations (behaviorally).
    let mut origin = MeshRouter::new(fp(1));
    let mut hop = MeshRouter::new(fp(2));
    let env = origin.origin_wrap(b"perf test payload");
    let original_ptr = env.as_ptr() as usize;
    let original_capacity = env.capacity();
    match hop.process_owned(env) {
        MeshDecision::Arrived { envelope, forward: true, .. } => {
            // Same backing buffer: the mutation was in place
            assert_eq!(envelope.as_ptr() as usize, original_ptr, "process_owned must reuse the input buffer");
            assert_eq!(envelope.capacity(), original_capacity);
            // Header mutated correctly
            let hdr = parse_envelope(&envelope).unwrap();
            assert_eq!(hdr.ttl, DEFAULT_TTL - 1);
            assert_eq!(hdr.hops_so_far, 1);
        }
        other => panic!("unexpected {:?}", other),
    }
}

#[test]
fn inner_slice_zero_copy_view() {
    let mut origin = MeshRouter::new(fp(1));
    let payload = b"inner-content-bytes";
    let env = origin.origin_wrap(payload);
    let view = inner_slice(&env);
    assert_eq!(view, payload);
    // Point arithmetic: view points INTO env, not a copy
    assert_eq!(view.as_ptr() as usize, env[MESH_HEADER_LEN..].as_ptr() as usize);
}

#[test]
fn malformed_envelopes_drop_cleanly() {
    let mut r = MeshRouter::new(fp(1));
    assert!(matches!(r.process(b""), MeshDecision::Drop("mesh envelope too short")));
    // Wrong magic but long enough to pass length check
    let mut wrong_magic = vec![0u8; MESH_HEADER_LEN + 5];
    wrong_magic[..6].copy_from_slice(b"SPORE\x07");
    assert!(matches!(r.process(&wrong_magic), MeshDecision::Drop("bad mesh magic")));
    // Valid magic but too short
    let mut fake = vec![0u8; MESH_HEADER_LEN - 1];
    fake[..6].copy_from_slice(SPORE_V8_MAGIC);
    assert!(matches!(r.process(&fake), MeshDecision::Drop(_)));
}

#[test]
fn dedup_cache_evicts_oldest() {
    let mut origin = MeshRouter::new(fp(99));
    let cap = 16; // matches the implementation's floor
    let mut hop = MeshRouter::with_config(fp(1), 8, cap);
    // Send cap+3 distinct messages through hop so the first ones evict
    let mut envs = Vec::new();
    for _ in 0..(cap + 3) {
        envs.push(origin.origin_wrap(b"x"));
    }
    for env in &envs {
        hop.process(env);
    }
    assert_eq!(hop.dedup_cache_size(), cap, "cache clamps at cap");
    // Post-Bloom-fix: an evicted msg_id remains blocked by the second-level
    // Bloom memory. Replay must be dropped.
    let first = &envs[0];
    let decision = hop.process(first);
    assert!(matches!(decision, MeshDecision::Drop("duplicate")), "evicted msg_id must stay blocked by Bloom layer, got {:?}", decision);
}

#[test]
fn bloom_blocks_replay_beyond_dedup_cap() {
    // Integration: 50 distinct msgs through a 16-cap router. None of
    // the evicted ones should replay successfully.
    let mut origin = MeshRouter::new(fp(99));
    let cap = 16;
    let mut hop = MeshRouter::with_config(fp(1), 8, cap);
    let mut envs = Vec::new();
    for _ in 0..50 {
        envs.push(origin.origin_wrap(b"x"));
    }
    for env in &envs {
        hop.process(env);
    }
    // Replay every single one — all must be blocked.
    for (i, env) in envs.iter().enumerate() {
        let d = hop.process(env);
        assert!(matches!(d, MeshDecision::Drop("duplicate")), "msg #{} replay must be dropped, got {:?}", i, d);
    }
}

#[test]
fn bloom_reset_clears_long_memory() {
    let mut origin = MeshRouter::new(fp(99));
    let mut hop = MeshRouter::with_config(fp(1), 8, 16);
    let env = origin.origin_wrap(b"x");
    hop.process(&env);
    assert!(hop.bloom_inserts_since_reset() > 0);
    let inserts_before_reset = hop.bloom_inserts();
    hop.bloom_reset();
    // AE-refinement: lifetime counter is monotonic, not cleared.
    assert_eq!(hop.bloom_inserts(), inserts_before_reset, "lifetime bloom_inserts must survive reset (AB3 monotonic invariant)");
    // Per-cycle counter IS cleared.
    assert_eq!(hop.bloom_inserts_since_reset(), 0, "per-cycle bloom_inserts_since_reset is cleared by reset");
    // After reset + cache already holds env (seen_set), still dupe from exact.
    // Clear seen_set cache indirectly by overflowing:
    for _ in 0..20 {
        hop.process(&origin.origin_wrap(b"x"));
    }
    // Original env must NOT be in exact cache anymore AND bloom is reset.
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Arrived { forward: true, .. }), "after bloom_reset + seen_set eviction, msg should be accepted again");
}

#[test]
fn msg_id_unique_across_distinct_broadcasts() {
    let mut r = MeshRouter::new(fp(7));
    let mut ids = Vec::new();
    for _ in 0..1000 {
        let env = r.origin_wrap(b"x");
        let hdr = parse_envelope(&env).unwrap();
        ids.push(hdr.msg_id);
    }
    let unique: HashSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "msg_ids from the same origin+counter must all be unique");
}

fn mac_key(seed: u8) -> MeshMacKey {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = seed.wrapping_add(i as u8);
    }
    MeshMacKey(k)
}

#[test]
fn v9_signed_roundtrip_origin_to_hop() {
    let key = mac_key(7);
    let mut origin = MeshRouter::new_signed(fp(1), key.clone());
    let mut hop = MeshRouter::new_signed(fp(2), key.clone());
    assert!(origin.is_signed() && hop.is_signed());
    let env = origin.origin_wrap(b"signed payload");
    assert!(env.starts_with(SPORE_V9_MAGIC), "origin_wrap must emit v9 when signed");
    assert_eq!(env.len(), MESH_V9_HEADER_LEN + b"signed payload".len());
    match hop.process(&env) {
        MeshDecision::Arrived { envelope, forward: true, .. } => {
            // After forward, MAC still valid at next hop
            let mut hop2 = MeshRouter::new_signed(fp(3), key);
            let d2 = hop2.process(&envelope);
            assert!(matches!(d2, MeshDecision::Arrived { forward: true, .. }), "hop2 must accept forwarded v9, got {:?}", d2);
        }
        other => panic!("expected Arrived forward=true, got {:?}", other),
    }
}

#[test]
fn v9_tag_tampering_rejected() {
    let key = mac_key(7);
    let mut origin = MeshRouter::new_signed(fp(1), key.clone());
    let mut hop = MeshRouter::new_signed(fp(2), key);
    let mut env = origin.origin_wrap(b"x");
    // Flip one bit of the tag (byte 25)
    env[25] ^= 0x01;
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("bad mesh mac")), "tampered tag must be rejected, got {:?}", d);
}

#[test]
fn v9_origin_fp_spoofing_rejected() {
    // The attack: someone replays our envelope but claims a different fp.
    // Must fail because the tag is tied to the original fp.
    let key = mac_key(7);
    let mut origin = MeshRouter::new_signed(fp(1), key.clone());
    let mut hop = MeshRouter::new_signed(fp(2), key);
    let mut env = origin.origin_wrap(b"x");
    // Tamper origin_fp bytes [14..22]
    env[14] = 99;
    env[15] = 99;
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("bad mesh mac")), "origin_fp spoof must be rejected, got {:?}", d);
}

#[test]
fn v9_wrong_key_rejected() {
    let mut origin = MeshRouter::new_signed(fp(1), mac_key(7));
    let mut hop = MeshRouter::new_signed(fp(2), mac_key(99));
    let env = origin.origin_wrap(b"x");
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("bad mesh mac")), "mismatched keys must reject, got {:?}", d);
}

#[test]
fn v9_signed_router_rejects_unsigned_v8() {
    let mut v8_origin = MeshRouter::new(fp(1));
    let mut v9_hop = MeshRouter::new_signed(fp(2), mac_key(7));
    let env = v8_origin.origin_wrap(b"x");
    assert!(env.starts_with(SPORE_V8_MAGIC));
    let d = v9_hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("unsigned v8 rejected by signed router")), "signed router must reject v8, got {:?}", d);
}

#[test]
fn v9_unsigned_router_rejects_v9() {
    let mut v9_origin = MeshRouter::new_signed(fp(1), mac_key(7));
    let mut v8_hop = MeshRouter::new(fp(2));
    let env = v9_origin.origin_wrap(b"x");
    let d = v8_hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("v9 received but router has no key")), "unsigned router must reject v9, got {:?}", d);
}

#[test]
fn v9_ttl_decrement_preserves_mac_through_chain() {
    let key = mac_key(7);
    let mut origin = MeshRouter::new_signed(fp(1), key.clone());
    let mut a = MeshRouter::new_signed(fp(2), key.clone());
    let mut b = MeshRouter::new_signed(fp(3), key.clone());
    let mut c = MeshRouter::new_signed(fp(4), key);
    let e0 = origin.origin_wrap(b"relay signed");
    let e1 = match a.process(&e0) {
        MeshDecision::Arrived { envelope, .. } => envelope,
        other => panic!("{:?}", other),
    };
    let e2 = match b.process(&e1) {
        MeshDecision::Arrived { envelope, .. } => envelope,
        other => panic!("{:?}", other),
    };
    match c.process(&e2) {
        MeshDecision::Arrived { hops_seen: 2, forward: true, .. } => {}
        other => panic!("3-hop v9 chain failed at c: {:?}", other),
    }
}

#[test]
fn v9_inner_slice_zero_copy_view() {
    let mut origin = MeshRouter::new_signed(fp(1), mac_key(7));
    let payload = b"inner-v9-bytes";
    let env = origin.origin_wrap(payload);
    let view = inner_slice(&env);
    assert_eq!(view, payload);
    assert_eq!(view.as_ptr() as usize, env[MESH_V9_HEADER_LEN..].as_ptr() as usize);
}

#[test]
fn origin_wrap_with_matches_origin_wrap_byte_for_byte() {
    // The new zero-intermediate-Vec builder must produce byte-identical
    // output to the old chained `wrap_topic + origin_wrap` path.
    let payload = vec![0xABu8; 1500];
    let topic_hash = 0xDEAD_BEEF_CAFE_F00D;

    // Old path: 2 allocations + 2 copies.
    let mut r1 = MeshRouter::new(fp(7));
    let topic_env = crate::topics::wrap_topic_hash(topic_hash, &payload);
    let mesh_env_old = r1.origin_wrap(&topic_env);

    // New path: 1 allocation + 1 copy.
    let mut r2 = MeshRouter::new(fp(7));
    let inner_len = crate::topics::TOPIC_HEADER_LEN + payload.len();
    let mesh_env_new = r2
        .origin_wrap_with(inner_len, |buf| {
            crate::topics::write_topic_envelope_into(buf, topic_hash, &payload);
        })
        .unwrap();

    assert_eq!(mesh_env_old, mesh_env_new, "buffer-backed builder must produce identical bytes");
}

#[test]
fn origin_wrap_with_returns_none_on_signed_router() {
    let key = MeshMacKey([0u8; 32]);
    let mut r = MeshRouter::new_signed(fp(1), key);
    let result = r.origin_wrap_with(10, |_buf| {});
    assert!(result.is_none(), "signed router should refuse the zero-copy path (MAC needs full inner)");
}

#[test]
fn tx_counter_roundtrip_through_persistence() {
    // Simulate: router emits 5 msgs, we snapshot counter, then rebuild
    // a fresh router with the same fp and restore the counter. Next
    // msg_id from the rebuilt router must NOT collide with the first 5.
    let mut r1 = MeshRouter::new(fp(42));
    let mut seen_msg_ids = Vec::new();
    for _ in 0..5 {
        let env = r1.origin_wrap(b"x");
        seen_msg_ids.push(parse_envelope(&env).unwrap().msg_id);
    }
    let snapshot = r1.tx_counter();
    assert_eq!(snapshot, 5);

    // Reboot: fresh router, restore counter
    let mut r2 = MeshRouter::new(fp(42));
    r2.set_tx_counter(snapshot);
    // The next wrap must use counter 6, producing a msg_id not in seen_msg_ids
    let env = r2.origin_wrap(b"x");
    let new_id = parse_envelope(&env).unwrap().msg_id;
    assert!(!seen_msg_ids.contains(&new_id), "post-reboot msg_id must not collide with pre-reboot msg_ids");
    assert_eq!(r2.tx_counter(), 6);
}

#[test]
fn tx_counter_without_restore_collides() {
    // Control test: reboot WITHOUT restoring counter ⇒ collision risk.
    // Documents why set_tx_counter matters.
    let mut r1 = MeshRouter::new(fp(42));
    let env1 = r1.origin_wrap(b"x");
    let id1 = parse_envelope(&env1).unwrap().msg_id;
    let mut r2 = MeshRouter::new(fp(42));
    // No set_tx_counter ⇒ counter starts at 0, first wrap uses counter 1
    let env2 = r2.origin_wrap(b"x");
    let id2 = parse_envelope(&env2).unwrap().msg_id;
    assert_eq!(id1, id2, "without persistence, reboot with same fp collides msg_ids");
}

#[test]
fn bloom_words_matches_feature_flag() {
    // Compile-time constant check: verify the right tier was picked.
    #[cfg(feature = "mesh_bloom_mcu")]
    assert_eq!(BLOOM_WORDS, 256, "mcu feature = 2 KiB Bloom");
    #[cfg(not(feature = "mesh_bloom_mcu"))]
    assert_eq!(BLOOM_WORDS, 8192, "default = 64 KiB Bloom");
}

// ── v0A Ed25519 per-node signed envelope tests ────────────────────
#[cfg(feature = "mesh_v10")]
fn ed_seed(byte: u8) -> MeshEdSeed {
    let mut s = [0u8; 32];
    for i in 0..32 {
        s[i] = byte.wrapping_add(i as u8);
    }
    MeshEdSeed(s)
}

#[cfg(feature = "mesh_v10")]
fn build_registry(entries: &[([u8; FP_LEN], &MeshEdSeed)]) -> MeshPubRegistry {
    let mut r = MeshPubRegistry::new();
    for (fp, seed) in entries {
        let pk = mesh_v10_pubkey_from_seed(seed).unwrap();
        r.insert(*fp, pk);
    }
    r
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_signed_roundtrip_origin_to_hop() {
    let seed_origin = ed_seed(7);
    let seed_hop = ed_seed(9);
    // Registry must contain BOTH nodes (each one verifies the other's envelopes)
    let origin_reg = build_registry(&[(fp(2), &seed_hop)]);
    let hop_reg = build_registry(&[(fp(1), &seed_origin)]);
    let mut origin = MeshRouter::new_ed25519_signed(fp(1), seed_origin.clone(), origin_reg);
    let mut hop = MeshRouter::new_ed25519_signed(fp(2), seed_hop.clone(), hop_reg);
    assert!(origin.is_ed25519_signed() && hop.is_ed25519_signed());

    let env = origin.origin_wrap(b"v0A signed payload");
    assert!(env.starts_with(SPORE_V10_MAGIC), "origin_wrap must emit v0A");
    assert_eq!(env.len(), MESH_V10_HEADER_LEN + b"v0A signed payload".len());

    match hop.process(&env) {
        MeshDecision::Arrived { envelope, forward: true, .. } => {
            // MAC covers immutable fields; TTL-- + hops++ still parse clean at next hop.
            let mut hop2 = MeshRouter::new_ed25519_signed(fp(3), ed_seed(11), build_registry(&[(fp(1), &seed_origin)]));
            let d2 = hop2.process(&envelope);
            assert!(matches!(d2, MeshDecision::Arrived { forward: true, .. }), "hop2 must accept forwarded v0A with unchanged signature, got {:?}", d2);
        }
        other => panic!("expected Arrived{{forward=true}}, got {:?}", other),
    }
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_sig_tampering_rejected() {
    let seed_o = ed_seed(7);
    let seed_h = ed_seed(9);
    let mut origin = MeshRouter::new_ed25519_signed(fp(1), seed_o.clone(), build_registry(&[(fp(2), &seed_h)]));
    let mut hop = MeshRouter::new_ed25519_signed(fp(2), seed_h, build_registry(&[(fp(1), &seed_o)]));
    let mut env = origin.origin_wrap(b"x");
    // Flip one bit in the signature field (bytes 25..89)
    env[30] ^= 0x01;
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("bad mesh signature")), "tampered signature must be rejected, got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_spoofed_origin_fp_rejected() {
    // Attacker obtains a valid signed envelope from a node THEY control.
    // They flip `origin_fp` bytes to impersonate another node. The
    // signature no longer matches that fp → reject. This is the v5-style
    // sender-authenticity property but at the MESH layer.
    let seed_attacker = ed_seed(7);
    let seed_victim = ed_seed(9);
    let seed_hop = ed_seed(11);
    // Hop's registry trusts the VICTIM's pubkey under fp(5).
    // The attacker signs with their key but claims fp(5).
    let mut attacker = MeshRouter::new_ed25519_signed(fp(99), seed_attacker.clone(), build_registry(&[]));
    let mut env = attacker.origin_wrap(b"forgery");
    env[14..22].copy_from_slice(&fp(5));
    let mut hop = MeshRouter::new_ed25519_signed(fp(2), seed_hop, build_registry(&[(fp(5), &seed_victim)]));
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("bad mesh signature")), "spoofed origin_fp must be rejected (signature doesn't match claimed fp's pubkey), got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_unknown_sender_rejected() {
    // Envelope is legitimate (signed correctly by its origin) but the
    // receiver's registry doesn't include that origin's pubkey.
    let seed_o = ed_seed(7);
    let mut origin = MeshRouter::new_ed25519_signed(fp(1), seed_o, build_registry(&[]));
    let env = origin.origin_wrap(b"x");
    // Hop has an EMPTY registry → fp(1) not recognized.
    let mut hop = MeshRouter::new_ed25519_signed(fp(2), ed_seed(11), MeshPubRegistry::new());
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("unknown sender")), "envelope from unregistered fp must be rejected, got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_wrong_pubkey_in_registry_rejected() {
    // Registry has origin_fp but mapped to the WRONG pubkey.
    let seed_real = ed_seed(7);
    let seed_wrong = ed_seed(42); // different seed → different pubkey
    let mut origin = MeshRouter::new_ed25519_signed(fp(1), seed_real, build_registry(&[]));
    let env = origin.origin_wrap(b"x");
    let mut hop = MeshRouter::new_ed25519_signed(fp(2), ed_seed(11), build_registry(&[(fp(1), &seed_wrong)]));
    let d = hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("bad mesh signature")), "registry pointing at wrong pubkey must reject, got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_router_rejects_v8() {
    let mut v8_origin = MeshRouter::new(fp(1));
    let mut v10_hop = MeshRouter::new_ed25519_signed(fp(2), ed_seed(11), build_registry(&[]));
    let env = v8_origin.origin_wrap(b"x");
    let d = v10_hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop(_)), "v0A router must reject v8 envelope, got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_router_rejects_v9() {
    let mut v9_origin = MeshRouter::new_signed(fp(1), MeshMacKey([3u8; 32]));
    let mut v10_hop = MeshRouter::new_ed25519_signed(fp(2), ed_seed(11), build_registry(&[]));
    let env = v9_origin.origin_wrap(b"x");
    let d = v10_hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("v9 rejected by v0A-only router")), "v0A router must reject v9 envelope, got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_unsigned_router_rejects_v10() {
    let mut v10_origin = MeshRouter::new_ed25519_signed(fp(1), ed_seed(7), build_registry(&[]));
    let mut v8_hop = MeshRouter::new(fp(2));
    let env = v10_origin.origin_wrap(b"x");
    let d = v8_hop.process(&env);
    assert!(matches!(d, MeshDecision::Drop("v0A received but router has no registry")), "unsigned router must reject v0A, got {:?}", d);
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_pubkey_from_seed_matches_ed25519_compact() {
    // Sanity: our helper returns the same 32-byte pubkey that
    // ed25519_compact's keypair exposes.
    let seed = ed_seed(13);
    let pk = mesh_v10_pubkey_from_seed(&seed).unwrap();
    let s = ed25519_compact::Seed::from_slice(&seed.0).unwrap();
    let kp = ed25519_compact::KeyPair::from_seed(s);
    assert_eq!(&pk.0[..], kp.pk.as_ref());
}

#[cfg(feature = "mesh_v10")]
#[test]
fn v10_inner_slice_zero_copy_view() {
    let mut origin = MeshRouter::new_ed25519_signed(fp(1), ed_seed(7), build_registry(&[]));
    let payload = b"inner-v0A-bytes";
    let env = origin.origin_wrap(payload);
    let view = inner_slice(&env);
    assert_eq!(view, payload);
    assert_eq!(view.as_ptr() as usize, env[MESH_V10_HEADER_LEN..].as_ptr() as usize);
}

#[test]
fn hmac_sha256_8_matches_rfc_2104_shape() {
    // Smoke test: output is 8 bytes, deterministic for same input,
    // differs for different keys.
    let k1 = [0u8; 32];
    let mut k2 = [0u8; 32];
    k2[0] = 1;
    let t1 = hmac_sha256_8(&k1, b"hello");
    let t1b = hmac_sha256_8(&k1, b"hello");
    let t2 = hmac_sha256_8(&k2, b"hello");
    let t3 = hmac_sha256_8(&k1, b"HELLO");
    assert_eq!(t1, t1b, "deterministic");
    assert_ne!(t1, t2, "key affects tag");
    assert_ne!(t1, t3, "message affects tag");
}

#[test]
fn different_origins_produce_different_msg_ids() {
    let mut r1 = MeshRouter::new(fp(1));
    let mut r2 = MeshRouter::new(fp(2));
    let e1 = r1.origin_wrap(b"x");
    let e2 = r2.origin_wrap(b"x");
    let h1 = parse_envelope(&e1).unwrap();
    let h2 = parse_envelope(&e2).unwrap();
    assert_ne!(h1.msg_id, h2.msg_id, "different origins with same counter must hash to different msg_ids");
}
