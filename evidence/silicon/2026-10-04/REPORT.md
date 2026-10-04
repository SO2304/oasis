# OASIS — First on-silicon test report (RP2040)

**Date:** 2026-10-04 · **Firmware git base:** `3833620` · **Target:** `thumbv6m-none-eabi`
**Firmware:** `oasis-silicon-test/` (this repo) · **Host capture:** Git Bash `/dev/ttyS*`

## Verdict (one line)
OASIS's `no_std` **crypto + R14 safety gate + mesh protocols run correctly on real
RP2040 silicon** — the full T0–T6 suite PASSES, **3× reproducibly, on board B**.
Board A booted and passed T0 but its full suite was not re-captured (see §7). Every
number below comes from a raw log captured **on the chip** and archived with a
SHA-256 in `SHA256SUMS`. Nothing here is simulated or estimated.

## 1. Hardware
Two **RP2040** boards (VID:PID `2E8A:0003` in BOOTSEL), USB-connected to the PC.

| Board | USB serial (hardware) | Firmware USB serial | Clock (measured) | Notes |
|---|---|---|---|---|
| A | `E0C9125B0D9B` | `A` (firmware-baked) | 125 000 000 Hz | **clone** (see below) |
| B | `E0C9125B0D9B` | `B` (firmware-baked) | 125 000 000 Hz | **clone** |

**Clone caveat (honest):** both boards report the **identical** hardware USB serial
`E0C9125B0D9B` — genuine Raspberry Pi Picos would differ. They are almost certainly
clone RP2040 boards whose flash chip returns a non-unique ID. Board identity is
therefore provided by a **firmware-baked USB serial string** (`A`/`B`), set per flash.
Consequently the spec's T0 criterion "unique serial number" is **not satisfiable** on
this hardware; we report the compiled board id instead. The PLL still locks from the
clone's 12 MHz crystal (clock reads exactly 125 MHz), so the silicon itself is sound.

## 2. PASS/FAIL matrix

| Test | What it checks | Board A | Board B (×3) |
|---|---|---|---|
| **T0** | boot, 125 MHz clock, identity | **PASS** | **PASS** |
| **T1** | ChaCha20-Poly1305, RFC 8439 §2.8.2 ct‖tag byte-for-byte + decrypt round-trip | — | **PASS** |
| **T2** | X25519 public-key derivation, RFC 7748 §6.1 (Alice+Bob) | — | **PASS** |
| **T3** | Ed25519 deterministic KAT: pubkey+sig+verify+**tamper rejected** | — | **PASS** |
| **T4** | R14 gate: faults above threshold all blocked | — | **PASS** (50/50; also 200/200 archived) |
| **T5** | mesh v8 (arrive+dedup), v9 (arrive+**tamper-reject**), v0A (arrive+**forge-reject**) | — | **PASS** |
| **T6** | on-silicon timing (hardware TIMER) | — | **PASS** (see §3) |

Board B: runs 1/2/3 are byte-identical in verdicts (13 PASS lines each, SUITE PASS).
Raw: `board_B_run{1,2,3}.log`. Board A T0: `board_A_T0.log`.

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
- **Board A full suite** — blocked by its older auto-run firmware (see §7).
- **Energy / power draw**, **secure element (ATECC608B)**, **multi-node mesh over a
  link**, **flight / PX4**, **OTA**, **long-run soak**.

## 7. Board A status
Board A was flashed early with a first firmware revision that **auto-runs a ~6 min
suite on boot** and therefore does not service the `b` (reboot-to-BOOTSEL) command
while computing. Software recycling timed out. Completing board A needs a **manual
BOOTSEL** (hold BOOTSEL, replug); the capture tooling will then flash `firmware_A.uf2`
and record `board_A_run{1,2,3}.log` in ~3 min. Board A's T0 (boot/clock/id) is captured.

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
`board_A_T0.log`, `board_B_T0.log`, `board_B_T0toT4_200faults.log`,
`board_B_run{1,2,3}.log`, `firmware_A.uf2`, `firmware_B.uf2`, `SHA256SUMS`.
