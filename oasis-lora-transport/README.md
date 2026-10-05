# oasis-lora-transport — Gap 1 scaffolding (real radio transport)

Opens Gap 1 of the OASIS defense-vertical shadow audit
([SHADOW_AUDIT_DEFENSE_VERTICAL.md](../webots/SHADOW_AUDIT_DEFENSE_VERTICAL.md)).
Provides the software stack between an OASIS `MeshRouter` and a LoRa
radio. The **byte-layer** works end-to-end in simulation; the final
piece is an SPI driver for a real SX1262 (commodity ~$15 chip).

## What's in

- `LoRaRadio` trait matching SX126x semantics (init / tx / rx / sleep)
- `LoRaParams` config struct (frequency, SF, BW, CR, TX power, preamble, sync word)
- `LoRaTransport<R: LoRaRadio>` that wraps OASIS frame packing around any radio
- `SimulatedLoRaRadio` — host-only, channel-backed, with uniform-loss injection
- `Sx1262Driver` — **stub with honest init-sequence comments**; returns
  `LoRaError::Driver("init not yet implemented")` to prevent silent
  success against unconfigured hardware

## Tests (all green)

```
$ cargo test --release --features std
test tests::frame_roundtrip_preserves_payload ...          ok
test tests::frame_rejects_tampered_crc ...                 ok
test tests::frame_rejects_bad_magic ...                    ok
test sim::tests::two_node_v10_roundtrip_over_simulated_lora ... ok
test sim::tests::loss_injection_drops_packets ...          ok
test sim::tests::signed_envelope_survives_partial_loss ... ok

test result: ok. 6 passed; 0 failed
```

Cross-compilation confirmed clean on 3 targets:
- host x86_64-pc-windows-msvc
- `thumbv6m-none-eabi` (RP2040, M0+)
- `thumbv7em-none-eabihf` (STM32F4/F7, M4F/M7)

## How to use (today, without hardware)

```rust
use oasis_lora_transport::{LoRaParams, LoRaTransport, sim::SimulatedLoRaRadio};
use oasis_rt::mesh::*;

let (radio_a, radio_b) = SimulatedLoRaRadio::linked_pair(42);
let mut tx = LoRaTransport::new(radio_a, LoRaParams::default())?;
let mut rx = LoRaTransport::new(radio_b, LoRaParams::default())?;

let envelope = router_a.origin_wrap(b"payload");
tx.send_envelope(&envelope)?;
let mut buf = [0u8; 255];
let env_rx = rx.recv_envelope(&mut buf, 500)?;
match router_b.process(env_rx) {
    MeshDecision::Arrived { .. } => println!("verified over LoRa"),
    _ => println!("reject"),
}
```

## How to use (when hardware lands)

Replace `SimulatedLoRaRadio` with `Sx1262Driver`. The rest doesn't change.

```rust
use stm32f4xx_hal::{spi::Spi, pac, gpio::*};
// ... wire up SPI1, NSS on PB0, BUSY on PB1, RESET on PC14, DIO1 on PC13
let radio = Sx1262Driver::new(spi, nss, busy, reset, dio1);
let mut transport = LoRaTransport::new(radio, LoRaParams {
    frequency_hz: 868_000_000,
    sf: 9,           // tune for range vs throughput
    bw_code: 4,      // 125 kHz
    cr: 5,           // 4/5
    tx_power_dbm: 14,// EU868 ETSI max general-ISM
    preamble_len: 8,
    sync_word: 0x12,
})?;
```

## What's left to close Gap 1 fully

1. **Real `Sx1262Driver` impl**: the 12-step init sequence is commented
   in [src/sx1262.rs](src/sx1262.rs) verbatim from SX126x datasheet §15.
   Choice of dependency: `sx126x-rs` (blocking, stable, thumbv6m-friendly)
   or `lora-phy` (async, embassy ecosystem, thumbv7+).
2. **Pair of boards**: Waveshare Pico-LoRa-SX1262 × 2 (~$40 total) or
   SEEED Wio-WM1110 × 2 (~$70 total).
3. **Outdoor range test**: measure packet loss vs distance at SF7-SF12,
   BW 125-500 kHz. Publish K=10 banded PER (packet error rate) per
   range bin.
4. **FHSS extension (Gap 2)**: on top of the working SX1262, schedule
   frequency hops via the shared Ed25519 seed. Target: survive a
   bench-simulated narrowband jammer.

Gap 1 (bench-level: software stack ready + cross-compiled + sim-validated)
is **closed by this crate**. Gap 1 (field-level: measured range over real
radio) needs ~$40 of hardware and one afternoon.
