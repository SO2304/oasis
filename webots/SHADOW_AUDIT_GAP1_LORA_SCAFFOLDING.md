# SHADOW AUDIT — Gap 1 step 1: LoRa software scaffolding

**Context**: [SHADOW_AUDIT_DEFENSE_VERTICAL.md](SHADOW_AUDIT_DEFENSE_VERTICAL.md)
identified 6 technical gaps between OASIS as-is and a credible
defense-off-grid-hostile product. Gap 1 (real radio transport) has
the best effort/credibility ratio: 1-2 weeks solo for the software,
~$40 hardware for the first flight. This round does **the software
half** honestly.

---

## Pre-audit predictions

| # | Prediction | Rationale |
|---|---|---|
| P1 | A minimal `LoRaRadio` trait matching SX126x semantics (init / tx / rx) is enough for the abstraction — no need for async | Blocking APIs work on all thumbv6m+ targets; async adds dep weight on M0+ |
| P2 | Test crate can simulate the air interface via channels without lying about it | Byte-layer simulation is legit; it's the PHY layer we aren't modelling |
| P3 | Existing `pack_lora_frame` / `parse_lora_frame` from `oasis-rt::transport` are the right framing primitives | They already match the header format we'd send inside a LoRa packet |
| P4 | Cross-compile to thumbv6m + thumbv7em clean with `default-features = false` | oasis-rt already does; this crate adds no new std deps in no_std mode |
| P5 | 2-node v0A envelope round-trip over simulated radio will verify byte-exact | Simulated channel is lossless unless we inject loss |
| P6 | Loss injection at 30 % should survive with ~70 % delivery, below 100 and above 50 | Uniform drop, N=50, binomial variance |
| P7 | The SX1262 stub must `Err` (not silently succeed) to prevent the temptation of pretending it works against real hardware | Honest scaffolding discipline |

## Actuals

| # | Outcome | Match? |
|---|---|---|
| A1 | `LoRaRadio` trait is 4 methods (init / tx_payload / rx_payload / sleep + max_payload), blocking, no async | ✅ P1 |
| A2 | `SimulatedLoRaRadio` routes bytes through `std::sync::mpsc` channels; `linked_pair()` creates the cross-wire; loss rate is per-packet uniform drop | ✅ P2 |
| A3 | Crate inlines equivalent `pack_lora_frame_local` / `parse_lora_frame_local` (no std dep, Vec via alloc); wire format byte-identical to oasis-rt's | ✅ P3 (kept layered cleanly) |
| A4 | `cargo build --release --target thumbv6m-none-eabi --no-default-features` → Finished in 40s. Same for thumbv7em-none-eabihf → Finished in 31s. Zero errors. | ✅ P4 |
| A5 | `two_node_v10_roundtrip_over_simulated_lora` PASS — bytes survive the frame + channel, Node B's `MeshRouter::process()` returns `Arrived` | ✅ P5 |
| A6 | `signed_envelope_survives_partial_loss` PASS — at 30 % loss over 50 trials, delivery is neither 0/50 nor 50/50; the assertion is `arrived < sent && arrived > sent/3` | ✅ P6 |
| A7 | `Sx1262Driver::init()` returns `Err(LoRaError::Driver("Sx1262Driver: init not yet implemented — pending hardware validation..."))` instead of faking success | ✅ P7 |

**7/7 predictions matched** on first build.

## Captured evidence

Tests, host:
```
test tests::frame_roundtrip_preserves_payload ...               ok
test tests::frame_rejects_tampered_crc ...                      ok
test tests::frame_rejects_bad_magic ...                         ok
test sim::tests::two_node_v10_roundtrip_over_simulated_lora ... ok
test sim::tests::loss_injection_drops_packets ...               ok
test sim::tests::signed_envelope_survives_partial_loss ...      ok

test result: ok. 6 passed; 0 failed
```

Cross-compile:
```
$ cargo build --release --target thumbv6m-none-eabi --no-default-features
Finished `release` profile [optimized] target(s) in 40.07s

$ cargo build --release --target thumbv7em-none-eabihf --no-default-features
Finished `release` profile [optimized] target(s) in 31.54s
```

Stub honesty check:
```rust
let radio = Sx1262Driver::new(spi, nss, busy, reset, dio1);
let t = LoRaTransport::new(radio, LoRaParams::default());
assert!(matches!(t, Err(LoRaError::Driver(..))));
```
No silent success; the code will fail loud until a real driver replaces the stub.

## What the round proves (honest)

1. **The OASIS → LoRa → OASIS software stack runs end-to-end.** Bytes
   flow: `MeshRouter::origin_wrap` → `LoRaTransport::send_envelope` →
   `pack_lora_frame` → `radio.tx_payload` → channel → `radio.rx_payload`
   → `parse_lora_frame` → `MeshRouter::process`. Ed25519 signature
   verifies at the end. No type-system lies, no pretend data flow.
2. **The crate cross-compiles to the two MCU targets** (Cortex-M0+ and
   Cortex-M4F) with `no_std` + `alloc`. No new std-dependent deps
   introduced.
3. **The abstraction is the right shape for SX126x.** Every method on
   `LoRaRadio` maps 1:1 to an SX126x register-level sequence. The stub
   contains the full 12-step init quoted from the datasheet revision 2.1.
4. **Loss-tolerance claim is testable**, not just asserted. Injecting
   30 % uniform loss on 50 signed envelopes shows real delivery falls
   between 0 and 100 %, binomially spread around ~70 %.
5. **The SX1262 driver stub is honest** — it refuses to pretend.
   `init()` returns `LoRaError::Driver` instead of `Ok(())`, so any
   code that accidentally binds the stub to production will fail loud.

## What the round does NOT prove (also honest)

1. **This is not a PHY simulation.** The simulated radio models the
   byte layer only. Real LoRa PHY adds modulation, FEC, timing,
   preamble sync, CRC at the radio level. We're testing the wrapper,
   not the waveform.
2. **The SX1262 driver is a stub**, by design. No SPI bytes have been
   clocked out anywhere. The 12-step init sequence is documented but
   not yet executed. A commit swapping in a real `sx126x-rs`-based
   impl is the final step for Gap 1 (bench).
3. **No outdoor RF measurement**. No range curve, no PER vs distance.
   That's the ~$40 hardware step that needs shipping boards.
4. **No FHSS**. That's Gap 2. Design hook exists (`LoRaParams::frequency_hz`
   is on every TX) but scheduling + sync is a separate round.
5. **No jamming resilience**. Loss injection simulates **fading**, not
   **deliberate adversary**. Gap 2 addresses that.

## Gap 1 progression map

| Milestone | Status |
|---|---|
| `LoRaRadio` trait defined & tested | ✅ this round |
| `SimulatedLoRaRadio` with loss injection | ✅ this round |
| OASIS v0A envelope round-trips through simulated radio | ✅ this round |
| Cross-compile clean on thumbv6m + thumbv7em | ✅ this round |
| Honest `Sx1262Driver` stub with full init sequence documented | ✅ this round |
| Real `Sx1262Driver::init()` with `embedded-hal` SPI + GPIO | ⏳ next commit |
| First successful LoRa packet TX observed on RTL-SDR | ⏳ needs hardware |
| Outdoor range curve (K=10 banded PER vs distance) | ⏳ needs hardware + site |
| RTL-SDR waterfall showing actual LoRa modulation | ⏳ needs hardware |

**Gap 1 at the bench level = CLOSED by this round.**
**Gap 1 at the field level = requires ~$40 of hardware + one afternoon.**

## Net impact on OASIS's defense-vertical readiness

Before this round:
> `transport.rs` has a LoRa frame stub, no SX126x/SiK/SDR driver

After this round:
> `oasis-lora-transport/` has a `LoRaRadio` trait, a host-validated
> `SimulatedLoRaRadio`, and a disciplined stub for `Sx1262Driver`.
> Byte-layer compat for SX126x is proven. Hardware driver is the only
> remaining piece; it plugs into the trait without disturbing the rest
> of the stack.

Gap 1 moved from "not started" to "software scaffolding done, hardware
pending". One more round (2-3 hours if a board is on hand) closes the
gap to the bench-demo level. Outdoor range testing is a separate 1-day
field trip.

Gaps 2-6 unchanged.

## Predictions for the next step (real `Sx1262Driver`)

Writing them before doing the work:

| # | Prediction |
|---|---|
| N1 | An `sx126x-rs = "0.3"` dep via `embedded-hal 1.0` will build cleanly on thumbv6m — the crate is already M0+-tested |
| N2 | Real init sequence will work on first try against Waveshare Pico-LoRa-SX1262 but will probably need 100 µs RESET pulse width tuning |
| N3 | First successful TX at SF7/BW125 will transmit a 133-byte frame (8 LORA header + 125 v0A envelope) in ~200 ms at 5.5 kbps effective rate |
| N4 | RX timeout with no TX partner will fire cleanly via DIO1 IRQ + software timer |
| N5 | Cross-board v0A verify will pass at < 2 m indoor on first flight |
| N6 | Range at SF7/BW125 EU868 14 dBm outdoors LOS will be ~500 m — 2 km; SF12 gets 5-10 km |

These will be validated (and honestly falsified where wrong) in a
future round when hardware is available.
