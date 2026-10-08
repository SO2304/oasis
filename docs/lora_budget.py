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
