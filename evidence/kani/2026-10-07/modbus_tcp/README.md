# Kani — Modbus TCP layer (2026-10-07)

Tree: `7353899` (`01807ab` adds the module). Kani 0.68.0, CBMC 6.11.0, Linux.
Reproduce from the repository root: `evidence/kani/2026-10-07/modbus_tcp/run_mbtcp.sh <out_dir>`.

| Run | Harness | Expected | Result |
|---|---|---|---|
| tree as committed | `proof_mbtcp_frame_iff_act_and_matches_order` | verified | VERIFICATION SUCCESSFUL, 13.8 s |
| tree as committed | `proof_mbtcp_parse_total` | verified | VERIFICATION SUCCESSFUL, 8.0 s |
| mutant A: MBAP `tid ^ 1` in `from_rtu` | `proof_mbtcp_frame_iff_act_and_matches_order` | fails | FAILED: `u16_be(b, 0) == tid` |
| mutant B: FC16 quantity 9 accepted | `proof_mbtcp_parse_total` | fails | FAILED: bounded-write assertion |

The script aborts if a mutation does not apply, and restores the source on exit.

**First negative control had no effect.** Mutant B was first run, during development, against
a parser harness limited to 30-byte inputs. A 9-register FC16 frame is 31 bytes, so the
mutation was never explored and the harness still passed. The harness now explores inputs up
to 37 bytes (8 past the largest valid frame), and mutant B fails as expected.

Scope: these harnesses prove the TCP transport only. That a frame exists only for `Act` is
inherited from `modbus_gateway::gateway_decision`, whose own proofs are in `../modbus/`.
