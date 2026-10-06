# Phase 1.1 on silicon — ML-DSA-44 on RP2040

Spec: `docs/specs/PQ_AUTHORITY_SPEC.md` (§4 crate choice, §5 measurements, §6
silicon tests). Three RP2040 clone boards (A, B, C), Cortex-M0+ at 125 MHz, no FPU,
264 KB RAM. Logs are LF-only and come straight from USB-CDC
(`OASIS|board|test|PASS/FAIL|value|unit|git_stamp`).

## 1. Bake-off: which ML-DSA-44 verifier goes on the device

Firmware `oasis-silicon-test/src/bin/pq_bench.rs` at stamp **`98ecb35`**. It
verifies one hybrid `OAU1` revocation (2 526 B, signed on the PC by
`oasis-operator-key/examples/pq_payloads.rs`; public key and blob in
`oasis-silicon-test/src/pq/`). Time is median/min/max over K = 5 single runs on the
1 MHz hardware timer. Peak stack is measured by painting the free stack with a known
word, running the verifier once, and finding the lowest overwritten word. That gives
a lower bound on bytes written below the caller's stack pointer; USB is polled, so no
interrupt shares the stack. 3 boards × 3 runs = 9 runs, 171 lines, **0 FAIL**
(`10_bench_{A,B,C}_run{1,2,3}.log`).

| Operation | Median of the 9 run medians | Spread of run medians | Peak stack |
|---|---:|---:|---:|
| Ed25519 verify (`ed25519-compact`, same message) | 177.86 ms | 177.83–177.89 ms | 8 548 B |
| ML-DSA-44 verify, RustCrypto `ml-dsa` 0.1.1 | 177.87 ms | 177.87–177.88 ms | **84 020 B** |
| Full hybrid gate `verify_authority` (ml-dsa + Ed25519) | 357.93 ms | 357.88–357.97 ms | 84 180 B |
| ML-DSA-44 verify, `libcrux-ml-dsa` 0.0.10 (incl. copying key + signature into its types) | 198.16 ms | 198.11–198.22 ms | **44 796 B** |

Correctness on every run and every board: the valid signature is accepted and one
flipped bit (signature byte 1000) is refused, by both implementations.
`PQ_HEAP used=52` = only the 51-byte signed-message `Vec`; neither verifier allocates.

The Ed25519 figure matches the earlier, independent T6 measurement
(`v0A_verify` median 176.55 ms, `../board_A_run1.log`). That ML-DSA-44 under
`ml-dsa` lands within 0.1 % of Ed25519 is a coincidence of two different
computations: the stack (84 KB vs 8.5 KB) and the code are entirely different.

**Flash** (`00_sizes.log`, `llvm-size -B` on `pq_bench`, text = code + rodata):

| Verifiers linked | text | added vs none |
|---|---:|---:|
| none (Ed25519 only) | 68 544 B | — |
| `ml-dsa` | 85 180 B | +16 636 B |
| `libcrux-ml-dsa` | 186 208 B | +117 664 B |
| both | 201 236 B | +132 692 B |

(Each verifier figure includes the 1 312-byte public key.)

**Against the public baseline** (PQClean C, arXiv 2603.19340): verification there
takes 44.0 ms and 9.4 KB of stack. Both pure-Rust crates are **4.0–4.5× slower** and
need **4.8–8.9× more stack** on this core (taking 9.4 KB as 9 400 B; 4.7–8.7× if
it means KiB). We did not try to optimise either crate.

**Decision: `libcrux-ml-dsa` 0.0.10 verifies on the device**, applying the spec's
rule ("keep the safest that fits in memory"):

- both fit (264 KB RAM, 2 MB flash);
- libcrux's arithmetic, NTT and serialization are formally verified (hax/F*), while
  `ml-dsa`'s README says it has never been independently audited;
- libcrux uses **47 % less stack** (44.8 KB vs 84.0 KB). On a 264 KB part that also
  protects the firmware's heap and stack budget, and a stack overflow here is a
  HardFault;
- the cost is +11 % time (+20 ms per verification) and +101 KB flash compared with
  `ml-dsa`.

`ml-dsa` remains the independent oracle: it signs on the host (`pq_payloads`), and on
the PC every test signature is checked by both implementations, plus the 15 NIST
ACVP sigVer vectors.

Version pin: the first run (`09_bench_A_lx0.0.11pre1.log`, `00_sizes_lx0.0.11pre1.log`,
stamp `1b00414`) used `libcrux-ml-dsa 0.0.11-pre.1`, a pre-release published the same
day. It was re-pinned to the latest stable release, 0.0.10 (2026-07-15), in `98ecb35`
before the 9 runs above. Code size within 4 bytes, times within 0.1 %.

Lockfile note: commit `98ecb35` pins `libcrux-ml-dsa = "=0.0.10"` in
`oasis-silicon-test/Cargo.toml`, but its `oasis-silicon-test/Cargo.lock` still
listed 0.0.11-pre.1, because the lock was committed before the silicon crate was
rebuilt. The exact pin forced cargo to re-resolve at build time, so the images
above contain 0.0.10. The lockfile actually used is committed together with this
report.

## 2. The device path after the switch (stamp `be3d47b`)

`oasis_rt::authority::mldsa44_verify` now calls libcrux. `pq_bench` was rewired so that
its labels stay true: `PQ_RC_*` calls RustCrypto `ml-dsa` directly, `PQ_LX_*` goes
through `oasis_rt` (the production path), and `PQ_AUTH_*` is the full gate. 3 boards ×
1 run × K = 5, 57 lines, **0 FAIL** (`11_bench_device_path_{A,B,C}.log`).

| Operation | Run medians (A / B / C) | Peak stack |
|---|---|---:|
| **Full hybrid gate `verify_authority`** (parse, precheck, ML-DSA-44 via libcrux, Ed25519) | 377.08 / 377.04 / 377.10 ms | **48 724 B** |
| ML-DSA-44 via `oasis_rt` (libcrux) | 197.99 / 198.06 / 198.03 ms | 48 564 B |
| ML-DSA-44, `ml-dsa` called directly | 177.98 / 177.97 / 178.00 ms | 102 476 B |

**Peak stack depends on the call site, not only on the crate.** Through `oasis_rt`,
libcrux takes 3 768 B more than in the bake-off closure (48 564 vs 44 796). That is
the copy of the key and the signature into owned arrays (1 312 + 2 420 = 3 732 B).
RustCrypto `ml-dsa` called directly peaks at 102 476 B, against 84 020 B through the
old `oasis_rt` function, because inlining differs. libcrux uses about half as much
in both arrangements, so the decision holds. The figure that sizes the firmware is
the gate: **48.7 KB of stack, 377 ms**.
