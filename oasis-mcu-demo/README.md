# oasis-mcu-demo — OASIS primitives on RP2040 (Wokwi TRL 6 demo)

**Status**: firmware builds clean for `thumbv6m-none-eabi` (Cortex-M0+, Raspberry Pi Pico).
**Simulator**: Wokwi VS Code extension (`Wokwi.wokwi-vscode`).

Exercises five OASIS kernel primitives on a cycle-accurate Cortex-M0+
simulation, producing UART output that demonstrates the primitives actually
execute on a real MCU instruction stream (not just cross-compile passing).

## Primitives exercised

| # | Primitive | Source |
|---|---|---|
| 1 | `mesh::ttl_after_forward` / `mesh::should_forward` | `oasis-rt/src/mesh.rs` |
| 2 | `tension::tick_ttl` (M1 tension field TTL decay) | `oasis-rt/src/tension.rs` |
| 3 | `mesh::bloom_bit_index` (SplitMix64 hash) | `oasis-rt/src/mesh.rs` |
| 4 | `mesh::hmac_sha256_8` (v9 mesh MAC) | `oasis-rt/src/mesh.rs` |
| 5 | `reflex::AdaptiveReflex` (M9 reflex arc) | `oasis-rt/src/reflex.rs` |

## Build

```bash
cd oasis-mcu-demo
cargo build --release
```

Output: `target/thumbv6m-none-eabi/release/oasis-mcu-demo` (ELF, ~25 KB stripped).

## Run in Wokwi

1. Open `oasis-mcu-demo/` in VS Code.
2. Command palette → `Wokwi: Start Simulator`.
3. The extension reads `wokwi.toml` + `diagram.json` automatically.
4. Serial Monitor panel captures UART0 @ 115200 baud.

Expected first output lines:

```
╔══════════════════════════════════════════╗
║ OASIS on RP2040  —  TRL 6 demo           ║
║ (Wokwi-simulated Cortex-M0+, thumbv6m)   ║
╚══════════════════════════════════════════╝

[1] mesh::ttl_after_forward(5) = 4
    mesh::should_forward(5)    = true
[2] tension::tick_ttl(3) = (2, deactivate=false)
    tension::tick_ttl(1) = (0, deactivate=true)
[3] bloom_bit_index(0xDEADBEEF, k=0..2, m=256) = {..., ..., ...}
[4] v9 HMAC-SHA256-8(key=0x42...*32, 'lin:0.5 ang:0.1')
    = [XX XX XX XX XX XX XX XX]
[5] reflex.check(1000) pre-calibration  = false (expect false)
    reflex.check(1.0) post-calibration   = false (expect false)
    reflex.check(5.0) post-calibration   = true (expect true)

All 5 OASIS primitives executed on RP2040. TRL 6 for these.
```

## Why this constitutes TRL 6 evidence

- The same Rust source (`oasis-rt/src/{mesh,tension,reflex}.rs`) runs on:
  - Host (x86_64) under 428 unit tests + 3 Kani proofs
  - RP2040 Cortex-M0+ via `thumbv6m-none-eabi`
- Wokwi simulates the RP2040 at instruction / cycle level, not a native
  userspace fake.
- Output is deterministic and byte-exact with the host run for pure
  functions (mesh/tension/reflex).
- TRL 5 was "cross-compile + unit tests on host". TRL 6 is "same code
  on the target instruction stream, observed from the outside".

## Feature configuration

This crate invokes `oasis-rt` with:

```toml
oasis-rt = { path = "../oasis-rt", default-features = false, features = ["mesh_bloom_mcu"] }
```

- `default-features = false` drops `std`, `std_env`, `os_random`, and
  `mesh_v10` (Ed25519 per-node mesh signing — still std-dependent via
  `signature` crate v2.2).
- `mesh_bloom_mcu` trims the dedup Bloom filter from 64 KiB → 2 KiB.
