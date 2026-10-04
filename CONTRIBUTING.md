# Contributing to OASIS

Thanks for your interest. OASIS is a research prototype with rigorous
engineering discipline — your contributions should reflect that.

## Ground rules

1. **R10: files < 400 lines.** No exceptions. Refactor if you hit the wall.
2. **R1: strict typing.** Zero `any` in TypeScript. Rust is strict by default.
3. **R14: no physical action if entropy > threshold.** Safety rule. Don't bypass.
4. **Every feature needs a test.** Rust unit tests for `oasis-rt`, Vitest for
   `kernel/` TypeScript, hardware validation for mechanisms.
5. **Honest metrics.** A full coverage loop requires `inspected.len() == targets.len()`.
   No gaming the counter.

## Workflow

1. Fork → feature branch
2. `cargo test --release --lib` must pass (125+ tests)
3. `cargo clippy --release` should be clean
4. `cargo fmt --check` must pass
5. Open a PR; CI runs on Linux/macOS/Windows
6. A maintainer reviews, adversarial audit may be run

## Mechanism status changes

If your PR upgrades a mechanism's status (PARTIAL → WIRED+TESTED → PROVEN), you
must update `CLAUDE.md` AND `README.md` in the same commit, and justify the
change with a link to the test run or hardware session.

## Hostile auditor reviews

We routinely run "shadow audits" on the codebase with hostile LLM agents
that try to find OASIS-washing, fake metrics, and status-rebadging. PRs that
don't survive such an audit are rejected or rewritten. Expect scrutiny on:
- Banner statements that oversell what the code does
- Loop counts / inspection counts (must have honest criteria)
- Fast-mode simulation vs real-time disclaimers
- Module imports that are present but never called (cosmetic labels)

## Hardware contributions

If you have access to a Crazyflie 2.1, an ABB arm, or any other actuator
platform, please test `drone_bridge` (or a platform-specific adapter) and
report results — real hardware sessions are the highest-value contributions.

## Licensing

By contributing, you agree that your contributions are licensed under MIT.
