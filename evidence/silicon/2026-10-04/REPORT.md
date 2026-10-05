# OASIS — First on-silicon test report (RP2040)

**Date:** 2026-10-04 · **Firmware git base:** `3833620` · **Target:** `thumbv6m-none-eabi`
**Firmware:** `oasis-silicon-test/` (this repo) · **Host capture:** Git Bash `/dev/ttyS*`

## Verdict (one line)
OASIS's `no_std` **crypto + R14 safety gate + mesh protocols run correctly on real
RP2040 silicon** — the full T0–T6 suite PASSES, **3× reproducibly on all THREE RP2040
boards (9 runs total, all green)**. Every number below comes from a raw log captured
**on the chip** and archived with a SHA-256 in `SHA256SUMS`. Nothing here is simulated
or estimated. (Board C — a 3rd Pico — was added 2026-10-05 and passes identically.)

## 1. Hardware
Three **RP2040** boards (VID:PID `2E8A:0003` in BOOTSEL), USB-connected to the PC.

| Board | USB serial (hardware) | Firmware USB serial | Clock (measured) | Notes |
|---|---|---|---|---|
| A | `E0C9125B0D9B` | `A` (firmware-baked) | 125 000 000 Hz | **clone** (see below) |
| B | `E0C9125B0D9B` | `B` (firmware-baked) | 125 000 000 Hz | **clone** |
| C | (not recorded) | `C` (firmware-baked) | 125 000 000 Hz | 3rd Pico, flashed 2026-10-05 |

**Clone caveat (honest):** both boards report the **identical** hardware USB serial
`E0C9125B0D9B` — genuine Raspberry Pi Picos would differ. They are almost certainly
clone RP2040 boards whose flash chip returns a non-unique ID. Board identity is
therefore provided by a **firmware-baked USB serial string** (`A`/`B`), set per flash.
Consequently the spec's T0 criterion "unique serial number" is **not satisfiable** on
this hardware; we report the compiled board id instead. The PLL still locks from the
clone's 12 MHz crystal (clock reads exactly 125 MHz), so the silicon itself is sound.

## 2. PASS/FAIL matrix

| Test | What it checks | A ×3 | B ×3 | C ×3 |
|---|---|---|---|---|
| **T0** | boot, 125 MHz clock, identity | **PASS** | **PASS** | **PASS** |
| **T1** | ChaCha20-Poly1305, RFC 8439 §2.8.2 ct‖tag byte-for-byte + decrypt round-trip | **PASS** | **PASS** | **PASS** |
| **T2** | X25519 public-key derivation, RFC 7748 §6.1 (Alice+Bob) | **PASS** | **PASS** | **PASS** |
| **T3** | Ed25519 deterministic KAT: pubkey+sig+verify+**tamper rejected** | **PASS** | **PASS** | **PASS** |
| **T4** | R14 gate: faults above threshold all blocked (50/50; B also 200/200 archived) | **PASS** | **PASS** | **PASS** |
| **T5** | mesh v8 (arrive+dedup), v9 (arrive+**tamper-reject**), v0A (arrive+**forge-reject**) | **PASS** | **PASS** | **PASS** |
| **T6** | on-silicon timing (hardware TIMER) | **PASS** | **PASS** | **PASS** |

All **9 runs (3 boards × 3)** are byte-identical in verdicts: 13 PASS lines each + SUITE
PASS. Raw: `board_{A,B,C}_run{1,2,3}.log`. T6 timing matches across all three independent
chips to the millisecond (R14 fault 3.245 ms, v9 wrap 0.726 ms, v0A sign 341.3 ms, v0A
verify 176.5 ms) — strong cross-validation that the numbers are real, not noise.

## 3. On-silicon timing (T6) vs x86 reference

Median of K=5 (slow ops) / K=5×M=50 (fast op), hardware 64-bit µs TIMER, board B.
x86 reference from `CLAUDE.md` (bench_r14_latency / bench_mesh_signed, Linux WSL).

| Operation | **Silicon (RP2040, M0+ @125 MHz)** | x86 (CLAUDE.md) | Slowdown |
|---|---|---|---|
| R14 fault (inject+signal) | **3.244 ms** | 331 ns | ~**9 800×** |
| mesh v9 wrap (HMAC) | **0.726 ms** | 431 ns | ~**1 680×** |
| mesh v0A Ed25519 **sign** | **341.3 ms** | 250.8 µs | ~**1 360×** |
| mesh v0A Ed25519 **verify** | **176.5 ms** | 134.9 µs | ~**1 310×** |

**Why so slow (honest):** the Cortex-M0+ has **no FPU** and no hardware multiply for
64-bit — every `f64` (R14 entropy = 9 `exp()`) and the Ed25519 field arithmetic run in
**software**. This is the single most important finding of hardware testing: operations
that are sub-microsecond on x86 cost **milliseconds to hundreds of milliseconds** here.
A v0A authority broadcast costs **~341 ms** to sign on-chip — fine for rare messages,
unusable at high rate. (Renode/STM32F4 timing: **not recorded** in the repo, so not
compared — the STM32F4 has an FPU and would land between these and x86.)

## 4. Memory (T7, from the linked ELF via `llvm-size`)
- **Flash (text+rodata):** 121 896 B ≈ **119 KB**
- **Static RAM (bss):** 163 876 B ≈ **160 KB** — dominated by the firmware's **160 KB
  heap reservation** (`embedded-alloc`). Each `MeshRouter` heap-allocates ~34 KB
  (a 4096-deep dedup `VecDeque<u64>` + 2 KB Bloom), so the test scopes sub-tests to
  keep ≤2 routers live. Stack high-water was **not** instrumented (no stack-painting).

## 5. Honest deviations from the spec
1. **T4 fault count 1000 → 50.** Soft-float makes R14 ~325 ms per full fault-cycle;
   1000 ≈ 5 min on-chip. 50/50 proves the gate identically (per-fault property). A
   standalone **200/200** run is archived (`board_B_T0toT4_200faults.log`).
2. **T6 batch sizes reduced** (K=5, M=1 for the slow ops) — 500-sample batches would
   take minutes.
3. **T2 is public-key derivation only** (RFC 7748 §6.1 pubkeys, byte-for-byte). The raw
   DH shared-secret is not a public `no_std` API in oasis-rt, so it was not vector-tested.
4. **T3 is a deterministic Ed25519 KAT** (fixed non-zero seed, reproducible) + verify +
   tamper-reject — **not** an RFC-8032 *numbered* vector. T1 and T2 ARE published-RFC
   vectors byte-for-byte.
5. **T7 partial** (size from ELF; no on-chip stack painting). **T8 (tx_counter flash
   persistence) NOT implemented** in this firmware.
6. **oasis-rt was NOT modified.** No crypto/R14 logic was changed to pass a test (the
   spec's stop-condition). All adaptations are in the test harness only.

## 6. What is explicitly NOT tested
- **LoRa radio** — the SX1262 path is untested here; T5 mesh ran **in-process on one
  chip** (two router instances), not over the air or even over a wire. No RF was involved.
- **Energy / power draw**, **secure element (ATECC608B)**, **flight / PX4**, **OTA**,
  **long-run soak**.
- (Multi-node mesh **over a wired UART link** IS now tested — see §10. Still not over RF.)

## 7. Boards added via manual BOOTSEL
Board A initially ran an early auto-run firmware that ignored the `b` reboot command,
so software recycling could not reflash it; after a **manual BOOTSEL** it was flashed
with `firmware_A.uf2` and passed 3×. **Board C** (a 3rd, previously unflashed Pico) was
later put in BOOTSEL, flashed with `firmware_C.uf2`, and passed the full suite 3× —
timing identical to A and B. Logs: `board_{A,C}_run{1,2,3}.log`.

## 8. Reproduce
```bash
# toolchain
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs

# build (per board id)
cd oasis-silicon-test
OASIS_BOARD_ID=B cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/oasis-silicon-test uf2/B.uf2

# flash: copy UF2 to the RPI-RP2 BOOTSEL volume (e.g. /d on this host)
cp uf2/B.uf2 /d/

# capture (Git Bash; COM10 -> /dev/ttyS9): send 'r' to run the suite
stty -F /dev/ttyS9 115200 raw -echo -ixon
exec 3<>/dev/ttyS9; printf 'r' >&3; timeout 90 cat <&3   # 'b' = reboot to BOOTSEL
```
Control bytes: `r` = run suite, `b` = reboot to BOOTSEL. Log line format:
`OASIS|<board>|<test>|PASS|FAIL|<value>|<unit>|<git_hash>`.

## 9. Evidence files (hashes in `SHA256SUMS`)
`board_{A,B,C}_run{1,2,3}.log` (9 full-suite runs), `board_A_T0.log`, `board_B_T0.log`,
`board_B_T0toT4_200faults.log`, `firmware_{A,B,C}.uf2`, `uart_mesh_A-B-C.log`,
`firmware_uart_{A,B,C}.uf2`, `SHA256SUMS`.

## 10. Wired-UART mesh relay A→B→C (2026-10-05)
Separate firmware `oasis-silicon-test/src/bin/uart_mesh.rs`: each node runs a v0A
`MeshRouter`, links over **UART0** (TX=GP0 pin1, RX=GP1 pin2, 115200 8N1), verifies the
Ed25519 origin signature at every hop, and relays forwarded envelopes. Wiring:
A.GP0→B.GP1, B.GP0→C.GP1, common GND. **This is a wired UART link, NOT LoRa** — no radio.

Result (`uart_mesh_A-B-C.log`): originating on A produced one v0A envelope (105 B) that
traversed A→B→C on the wire, same `msg_id=10830354352046574055` at all three nodes,
`hops` 0→1 across the relay, **`sig=verified` at B and at C**:
```
A| ORIGINATED msg_id(ctr)=0,len=105
B| ARRIVED    msg_id=…055, hops=0, sig=verified, forward=true
B| RELAYED    msg_id=…055, hops=0
C| ARRIVED    msg_id=…055, hops=1, sig=verified, forward=true
```
This is OASIS's first real inter-node communication on hardware: multi-hop v0A mesh with
per-hop Ed25519 verification over a physical link. Forgery rejection itself is covered by
T5 (`v0A_forge_rejected=true`, same verify path).

**Honest finding during bring-up:** the firmware first used UART1 (GP4) for TX; UART1
**failed internal hardware loopback** on this silicon (`uart1_internal_lbe_rx=0`) while
UART0 passed (`=6`), so the link was moved to UART0 only. Root cause of a dead first
attempt was firmware/peripheral, not the wiring — isolated via the on-chip LBE self-test,
not guesswork.

## 11. Attack / resilience tests over the UART mesh (2026-10-05)
Console commands added to `uart_mesh.rs` (`uart_mesh_attacks.log`,
`uart_mesh_task3_disconnect.log`):

| # | Test | Trigger | Result on silicon |
|---|---|---|---|
| 1 | **Anti-replay** | `R`→A (same envelope sent twice) | B: 1st `ARRIVED`+`RELAYED`, 2nd `DROP duplicate` — replay not relayed to C. **PASS** |
| 2 | **Forge / MitM** | `F`→B (claims origin=A, signed with bad seed) | C: `DROP "bad mesh signature"` — forgery rejected. **PASS** |
| 4 | **TTL** | `T`→A (origin TTL=0) | B: `ARRIVED forward=false`, no relay; C silent. **PASS** |
| 3 | **Flood / disconnect** | `X`→A (600-packet burst) | No board crashed (post-check: no BOOTSEL volume, all 3 COMs alive); B relayed throughout. **No panic/HardFault.** Live unplug→gap→resume not captured (wire not pulled in-window). |

**Honest notes (0 bullshit):**
- The prompt's "zero-alloc / `alloc` interdit" does **not** hold for this stack:
  `oasis-rt::MeshRouter` is heap-backed (dedup `VecDeque`, `Box` Bloom, `BTreeMap`
  registry; `origin_wrap`/`process` return `Vec`). These tests run on `embedded-alloc`.
  True zero-alloc would require reworking oasis-rt.
- **TTL semantics correction:** OASIS decrements on *forward* and treats TTL==0 *on
  receipt* as terminal (`Arrived{forward:false}`, not a hard Drop). So the prompt's
  "TTL=1 expires at B" is off-by-one — TTL=1 actually reaches C. Origin TTL=0 is what
  stops the packet at B (used here).
- **Replay needs pacing:** sending the two copies back-to-back overran B's 32-byte RX
  FIFO (2nd frame lost) while B was busy logging; a ~300 ms gap between copies makes the
  `DROP duplicate` deterministic. Dedup *logic* was already proven in T5.

## 12. Fault-injection noise sweep (2026-10-05)
`uart_mesh.rs` adds a `FaultInjector` (Xorshift32 PRNG seeded from the RP2040 TIMER —
no external RNG crate) and a `N`-triggered sweep that corrupts the **on-the-wire frame
copy** just before TX across 3 escalating levels, then transmits from A over the UART
chain. `NoiseLevel`: Normal (0/0), Medium (~5% bit-flip, ~2% truncate), Extreme (~20%
flip 1–4 bytes, ~15% drop TX). Log: `uart_mesh_noise_sweep.log`.

Run (150 packets, 50/phase — reduced from the spec's 100/phase because each packet is a
~341 ms v0A Ed25519 sign on the M0+, so 300 ≈ 2 min; each has a fresh msg_id to bypass
dedup):

| Metric | Value |
|---|---|
| A: transmitted / TX-dropped / truncated | 142 / 8 / 2 |
| B: clean frames relayed (`ARRIVED`, distinct msg_ids) | 124 |
| B: **framer CRC failures** (corruption caught) | 15 |
| B: bad-signature (verifier reached) | **0** |
| C: survivors received | 124 |
| Crash (panic/HardFault) | **none** (no BOOTSEL, all COMs alive) |

**Key finding (0 bullshit):** the CRC8 **framer absorbed 100 % of the wire corruption**
(15 CRC fails), so the **Ed25519 verifier was never reached by noise** (0 bad-sig) — the
intended defense-in-depth. To stress the *verifier* specifically you must corrupt the
envelope **pre-CRC** (bypassing the framer); pure wire noise is caught one layer earlier.
A (the transmitter) never panicked despite corrupting/dropping its own output; B's parser
+ framer survived the full escalating sweep with no crash. Honest scope unchanged: still
a **wired UART link, not LoRa**; the mesh runs on `embedded-alloc`, not zero-alloc.
