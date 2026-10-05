# Scope plan — Round 7: HIL network federation (next session)

**Status**: NOT STARTED. Scoped honestly to enable a focused future round.

## Goal

Complete the user's "ESP32 / Pi Pico W federation sur WiFi UDP →
Wireshark sur host" item — but route around the WiFi RF gap by using
**Ethernet** in Renode + `stm32h753zi` board (which has built-in MAC +
LAN8742 PHY in the platform definition).

## Why Ethernet, not WiFi

- Renode 1.16.1 does **NOT** include an ESP32 platform.
- Renode does **NOT** simulate WiFi RF (any chip with WiFi would be
  exposed as an Ethernet-equivalent transport).
- Therefore an Ethernet-based HIL gives the SAME demonstrable property
  ("MCU-emitted UDP packet captured by host network stack") without
  spending a day fighting WiFi stack initialization.
- Pi Pico W under Wokwi is the alternative (cyw43 + embassy-net), with
  similar ~5h cost. Ethernet via Renode is faster.

## Architecture

```
                   ┌─────────────────────────┐
                   │  Wokwi/Renode host PC   │
                   │   ┌───────────────┐     │
                   │   │ Wireshark on  │     │
                   │   │ tap0 / lo     │     │
                   │   └───────┬───────┘     │
                   │           │             │
                   │           ▼             │
       MCU eth ───── Renode SynchronizedEthernet ───── Host UDP socket
       (sends v0A    (bridges MCU                      (receives raw bytes,
        envelope to   eth frame to                       passes to oasis-rt
        239.0.0.1)   host network)                      mesh process())
```

## Estimated work

| Task | Effort |
|---|---|
| New crate `oasis-renode-hil-h7/` for STM32H7 target | 30 min |
| Set up `stm32h7xx-hal` + `cortex-m-rt` + memory.x | 30 min |
| `smoltcp` no_std TCP/IP stack with static IP config | 1.5 h |
| UDP socket + send v0A envelope as payload | 30 min |
| Renode .resc with `SynchronizedEthernet` to host TAP | 1 h |
| Host-side: `tcpdump` or `socat` to capture packets, decode envelope, verify sig | 30 min |
| Cross-check: bytes received on host = `MeshDecision::Arrived` via existing oasis-rt unit | 30 min |
| Shadow audit + diagrams | 30 min |
| **Total** | **~5 h** |

## Acceptance criteria

1. STM32H7 firmware boots in Renode, smoltcp comes up, gets static IP.
2. Firmware emits a UDP datagram with payload = v0A signed envelope.
3. Host captures the packet (Wireshark / tcpdump shows it on the bridge interface).
4. Host runs the captured payload through `MeshRouter::process()` with
   the firmware's pubkey in registry → `MeshDecision::Arrived`.
5. Byte-level match: payload bytes captured = bytes the firmware sent
   = signature verifies.

## Why this matters for OASIS

- Closes the **last remaining gap** between "OASIS works on host AND
  on MCU" and "OASIS federates between MCU AND host over a real
  network protocol stack".
- Removes the implicit assumption that mesh transport = direct UART.
  Real swarm scenarios want IP/UDP for routing flexibility.
- Validates that smoltcp's UDP socket layer is no_std-compatible with
  oasis-rt's mesh primitives (no hidden std dep surprises).

## Why deferred from current session

Already completed today (2026-04-22 → 2026-04-23):
- 5 OASIS primitives on Wokwi RP2040 (round 1)
- M9 reflex on GPIO IRQ (round 2)
- Ed25519 timing single + K=10 bands on M0+ (rounds 3 + 4)
- Renode 2-node M11 federation (round 4)
- Renode 3-hop chain TTL+sig integrity (round 5)
- Renode Bloom dedup (round 6)
- M1 TensionField + M5/M7 stack-overflow finding (round 5b)
- 2 shadow audits ([WOKWI_TRL6](SHADOW_AUDIT_WOKWI_TRL6.md), [RENODE_M11](SHADOW_AUDIT_RENODE_M11.md))

Adding 5h of new work would make the session sprawling. Better as a
focused next sit-down.

## Pre-flight checklist for next session

- [ ] Verify Renode `SynchronizedEthernet` works on Windows host
  (may require WinTAP adapter setup)
- [ ] Decide payload size: v0A envelope = ~125 B + UDP overhead = fits
  in single MTU
- [ ] Decide multicast vs unicast: 239.x.x.x for swarm, 192.168.x.x for
  point-to-point demo
- [ ] Cargo deps to add: `stm32h7xx-hal`, `smoltcp`, `embassy-net` (if
  using async pattern) or `nb`-style polling
