#!/usr/bin/env python3
"""lora_budget.py -- time on air and duty-cycle budget for OASIS message sizes.

Formula: the standard LoRa modem time-on-air, BW 125 kHz, 8 preamble symbols, explicit
header, CRC on, CR 4/5, low-data-rate optimisation on for SF11 and SF12.

    T_sym      = 2^SF / BW
    T_preamble = (n_pre + 4.25) * T_sym
    n_payload  = 8 + max(ceil((8*PL - 4*SF + 28 + 16*CRC - 20*IH)
                              / (4*(SF - 2*DE))) * (CR+4), 0)
    ToA        = T_preamble + n_payload * T_sym

Duty cycle: ETSI EN 300 220-2 clause 4.4.3.2, Tobs = 1 h, per BAND (not per channel), so
the three default LoRaWAN channels in band M share one budget:

    Ton_cum_max = 0.01 * 3600 s = 36 s / h
    msgs/h      = floor(36 / ToA)

!! The ToA formula itself is "non verifie a la source": the Semtech SX1276 datasheet is
behind a commercial portal (see docs/compliance/PQC.md). It is cross-checked here against
the independent figures in that document: PL=64 at SF12 must give 2793.5 ms.
"""
import math

BW = 125_000
N_PRE = 8
CR = 1          # 4/5
CRC = 1
IH = 0
DUTY_SECONDS = 0.01 * 3600


def toa_ms(sf, pl):
    de = 1 if sf >= 11 else 0
    t_sym = (2 ** sf) / BW
    t_pre = (N_PRE + 4.25) * t_sym
    num = 8 * pl - 4 * sf + 28 + 16 * CRC - 20 * IH
    den = 4 * (sf - 2 * de)
    n_pay = 8 + max(math.ceil(num / den) * (CR + 4), 0)
    return (t_pre + n_pay * t_sym) * 1000


# Self-check against docs/compliance/PQC.md
check = toa_ms(12, 64)
assert abs(check - 2793.5) < 0.1, 'formula does not reproduce the published 2793.5 ms: %.1f' % check
print('self-check OK: PL=64 SF12 -> %.1f ms (published 2793.5)\n' % check)

# LoRaWAN EU863-870 application payload cap, RP002-1.0.3 tables 12/13.
CAP = {7: 242, 8: 242, 9: 115, 10: 51, 11: 51, 12: 51}

MSGS = [
    ('stop order, compact OAS1 (proposed)', 99 + 11),
    ('supervision beacon OSB1', 131),
    ('Modbus order OMB1', 132),
    ('actuation order OAC1', 153),
    ('enrolment attestation OAU1 (Ed25519)', 221),
    ('revocation ORV1, 1 node', 234),
    ('revocation ORV1, 2 nodes', 242),
    ('revocation ORV1, 16 nodes [extrapolated +8/node]', 234 + 15 * 8),
]

print('| message | bytes | ' + ' | '.join('SF%d' % sf for sf in range(7, 13)) + ' |')
print('|---|---:|' + '---:|' * 6)
for name, n in MSGS:
    cells = []
    for sf in range(7, 13):
        if n > CAP[sf]:
            cells.append('**over cap**')
        else:
            t = toa_ms(sf, n)
            cells.append('%.0f ms / %d/h' % (t, math.floor(DUTY_SECONDS / (t / 1000))))
    print('| %s | %d | %s |' % (name, n, ' | '.join(cells)))

print()
print('LoRaWAN application payload cap (RP002-1.0.3): ' +
      ', '.join('SF%d=%d B' % (sf, CAP[sf]) for sf in range(7, 13)))
print('Duty cycle budget: %.0f s of air time per hour per band (1 %%, Tobs = 1 h).' % DUTY_SECONDS)


# ─── docs/CRYPTO_MIGRATION.md §2 and §6-d: fragmenting a post-quantum payload ───
#
# A payload bigger than the application cap must be fragmented (OFR1). The cost is the
# air time of every fragment, charged against the same 36 s/hour/band budget. Reported
# as "hours of hourly budget", the unit PQC.md §4 already uses: air_time / DUTY_SECONDS.
#
# Sizes read at the source: FIPS 204 Table 2 (ML-DSA) and FIPS 203 Table 3 (ML-KEM).
MLDSA44_SIG, MLDSA44_PK = 2420, 1312
MLKEM768_CT, MLKEM768_PK = 1088, 1184


#   A full fragment carries CAP[sf] application bytes inside a PHY payload that also
# carries the LoRaWAN frame overhead. PQC.md §4 charged 13 bytes of overhead (51 + 13 = 64,
# hence its ToA column at PL = 64); the same convention is kept here so the two documents
# are comparable, and the assertion at the bottom reproduces PQC.md's own figure.
# ⚠️ The 13-byte overhead is "non vérifié à la source" (TS001 not re-read), like the ToA
# formula itself. Every hour figure below is [calcul].
LORAWAN_OVERHEAD = 13


def frag_cost(nbytes, sf):
    """(fragments, air time s, hours of the hourly budget) for `nbytes` at `sf`."""
    frames = math.ceil(nbytes / CAP[sf])
    air = frames * toa_ms(sf, CAP[sf] + LORAWAN_OVERHEAD) / 1000
    return frames, air, air / DUTY_SECONDS


def oaq1_len(n):
    """quorum::oaq1_len — 51-byte body, one count byte, 96 bytes per inline signature."""
    return 51 + 1 + 96 * n


assert oaq1_len(1) == 148 and oaq1_len(2) == 244, 'must match quorum::tests'

print()
print('--- CRYPTO_MIGRATION.md: post-quantum payloads at SF12 ---')
CASES = [
    ('ML-KEM-768 ciphertext, once per peer', MLKEM768_CT),
    ('ML-KEM-768 encapsulation key', MLKEM768_PK),
    ('OAQ1 k=2, keys carried: 51+1+2*(1312+2420)', 51 + 1 + 2 * (MLDSA44_PK + MLDSA44_SIG)),
    ('OAQ1 k=2, one-byte indices: 51+1+2*(1+2420)', 51 + 1 + 2 * (1 + MLDSA44_SIG)),
]
for name, n in CASES:
    f12, a12, h12 = frag_cost(n, 12)
    f7, a7, h7 = frag_cost(n, 7)
    print('%-46s %5d B | SF12 %3d frames %6.1f s %5.1f h | SF7 %3d frames %5.1f s %4.1f %% of the hour'
          % (name, n, f12, a12, h12, f7, a7, 100 * h7))

# The four numbers quoted in CRYPTO_MIGRATION.md, asserted so the document cannot drift.
assert frag_cost(MLKEM768_CT, 12)[0] == 22, frag_cost(MLKEM768_CT, 12)
assert abs(frag_cost(MLKEM768_CT, 12)[2] - 1.7) < 0.05, frag_cost(MLKEM768_CT, 12)
assert frag_cost(51 + 1 + 2 * (MLDSA44_PK + MLDSA44_SIG), 12)[0] == 148
assert abs(frag_cost(51 + 1 + 2 * (MLDSA44_PK + MLDSA44_SIG), 12)[2] - 11.5) < 0.05
assert frag_cost(51 + 1 + 2 * (1 + MLDSA44_SIG), 12)[0] == 96
assert abs(frag_cost(51 + 1 + 2 * (1 + MLDSA44_SIG), 12)[2] - 7.4) < 0.05
# PQC.md §4's own figure, recomputed here as a cross-check of the method.
assert frag_cost(MLDSA44_SIG, 12)[0] == 48, 'PQC.md says 48 frames for one ML-DSA-44 signature'
assert abs(frag_cost(MLDSA44_SIG, 12)[2] - 3.72) < 0.05, 'PQC.md says 3.72 h'
print('all CRYPTO_MIGRATION.md figures reproduced, and PQC.md 48 frames / 3.72 h cross-checked')
