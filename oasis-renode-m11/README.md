# oasis-renode-m11 — M11 Federated Resonance on two MCUs (Renode)

Multi-machine emulation proof that OASIS Ed25519-signed mesh envelopes
(SPORE v0A / `mesh_v10`) round-trip cleanly between **two separate
Cortex-M4F MCUs** connected only by a UART wire.

- **Simulator**: Renode 1.16.1 (Windows portable in `C:\tools\renode_1.16.1-dotnet_portable\`).
- **Target**: `thumbv7em-none-eabihf` (STM32F407 on STM32F4 Discovery).
- **Wire**: Node A's USART2 ↔ Renode `UARTHub` ↔ Node B's USART2.
- **Logs**: each node's USART1 dumped via `CreateFileBackend`.

## Build

```bash
cd oasis-renode-m11
cargo build --release
# → target/thumbv7em-none-eabihf/release/sender   (~57 KB ELF)
# → target/thumbv7em-none-eabihf/release/receiver (~57 KB ELF)
```

## Run

```bash
/c/tools/renode_1.16.1-dotnet_portable/renode.exe \
    --console --disable-xwt --hide-log \
    -e 'include @C:/dev/oasis/oasis-renode-m11/renode_m11.resc'
cat log_node_a.txt log_node_b.txt
```

Expected Node B tail:
```
  [M11] ED25519 SIGNATURE VERIFIED — federation attested
[Node B] M11 RECV COMPLETE — VERIFIED
```

## Heap sizing note

`MeshRouter::new_ed25519_signed` allocates ~32 KB for the dedup VecDeque
plus a few KB for the Bloom filter and the ed25519 keypair. On MCU,
**set HEAP_SIZE >= 64 KiB**. The firmware here uses 80 KiB. Below
~40 KiB the router's construction silently fails (alloc error → panic
→ spin).

## Clock note

The STM32F4 HSE (external 8 MHz crystal) isn't reliably simulated in
Renode 1.16.1, so both firmwares use the HSI (16 MHz internal) oscillator.
Production firmware would want 168 MHz off HSE+PLL. Functional correctness
identical; throughput lower by ~10×.

## Files

- [src/bin/sender.rs](src/bin/sender.rs) — Node A: sign + transmit
- [src/bin/receiver.rs](src/bin/receiver.rs) — Node B: receive + verify
- [renode_m11.resc](renode_m11.resc) — Renode orchestration script
- [memory.x](memory.x), [build.rs](build.rs) — STM32F407 link config
- [log_node_a.txt](log_node_a.txt), [log_node_b.txt](log_node_b.txt) — captured UART logs

Shadow audit: [../webots/SHADOW_AUDIT_RENODE_M11.md](../webots/SHADOW_AUDIT_RENODE_M11.md).
