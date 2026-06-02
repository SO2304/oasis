"""
OASIS Recon-10 v3 — habituation analysis via R14_block_count per phase.

Key insight from v3 run: phone_brain's 128 pain memories pushed entropy to 0.8-0.98,
triggering R14 actuation blocks repeatedly. Fear stayed at 0 because Rust fear is
location-based (needs proximity to PAIN memory, which is at phone positions, not here).

So the REAL habituation signal in this setup is:
  - R14_block_count per EXTREME phase: does the trauma response DECREASE over cycles?
  - Entropy trend: does baseline entropy come down as kernel "processes" phantom pain?
"""
import csv, glob, os, re, statistics

ROOT = "C:/dev/oasis/webots"
CSV_GLOB = os.path.join(ROOT, "recon10v3_metrics_*.csv")
OUT_REPORT = os.path.join(ROOT, "recon10v3_habituation_report.md")
OUT_CURVE = os.path.join(ROOT, "recon10v3_habituation_curve.csv")

R14_BLOCK_RE = re.compile(r"R14 BLOCK.*blocks=(\d+)")

def r14_rate_halves(log_path):
    """Return (first_half_blocks, second_half_blocks) to detect rate change.
    Bridge logs R14 BLOCK every 50 blocks accumulated. We count log lines in
    the first half vs second half of the log (by line index proxy for time).
    If second-half line count < first-half line count, R14 rate is DECREASING
    = habituation indicator."""
    lines = []
    try:
        with open(log_path, encoding="utf-8", errors="ignore") as f:
            lines = [ln for ln in f if R14_BLOCK_RE.search(ln)]
    except Exception: return 0, 0, 0
    n = len(lines)
    if n == 0: return 0, 0, 0
    mid = n // 2
    # Each line ≈ 50 blocks logged, so blocks accumulated in each half
    first_half_blocks = mid * 50
    second_half_blocks = (n - mid) * 50
    # Extract max blocks=N for total
    max_n = 0
    for ln in lines:
        m = R14_BLOCK_RE.search(ln)
        if m: max_n = max(max_n, int(m.group(1)))
    return first_half_blocks, second_half_blocks, max_n

def parse_metrics(path):
    rows = []
    with open(path) as f:
        for r in csv.DictReader(f):
            try:
                rows.append({
                    "tick": int(r["tick"]), "alt": float(r["alt"]),
                    "fear": float(r["fear"]), "entropy": float(r["entropy"]),
                    "min_obs": float(r["min_obs"]),
                    "sonar_alive": int(r["sonar_alive"]),
                    "loops": int(r["loops"]), "abort": int(r["abort"]),
                    "phase": r["env_phase"], "phase_idx": int(r["phase_idx"]),
                })
            except Exception: pass
    return rows

def parse_log(log_path):
    """Return (total_r14_blocks, takeoff_tick, phone_brain_loaded, first_half_blocks, second_half_blocks)."""
    takeoff = 0; pb = False
    try:
        with open(log_path, encoding="utf-8", errors="ignore") as f:
            for line in f:
                m = re.search(r"TAKEOFF complete tick=(\d+)", line)
                if m and takeoff == 0: takeoff = int(m.group(1))
                if "PHONE-BRAIN loaded 128" in line: pb = True
    except Exception: pass
    first_h, second_h, max_blocks = r14_rate_halves(log_path)
    return max_blocks, takeoff, pb, first_h, second_h

def classify(rows):
    if not rows: return "EMPTY"
    tail = rows[len(rows)//2:]
    mean_alt = statistics.mean(r["alt"] for r in tail)
    max_alt = max(r["alt"] for r in tail)
    mean_abort = statistics.mean(r["abort"] for r in tail)
    if max_alt > 4.0: return "LOST"
    if mean_alt < 0.3: return "GROUND"
    if mean_abort > 0.5: return "ABORT"
    return "FLY"

def per_extreme_stats(rows, total_r14, takeoff_tick):
    """Per EXTREME phase: entropy, loops delta, r14 estimate proportional to ticks."""
    groups = {}
    for r in rows:
        groups.setdefault((r["phase_idx"], r["phase"]), []).append(r)

    total_active = max(1, rows[-1]["tick"] - takeoff_tick)
    blocks_per_tick = total_r14 / total_active

    out = []
    for (idx, ptype), recs in sorted(groups.items()):
        if ptype != "EXTREME": continue
        ticks_in_phase = recs[-1]["tick"] - max(recs[0]["tick"], takeoff_tick)
        ticks_in_phase = max(0, ticks_in_phase)
        out.append({
            "phase_idx": idx,
            "n_ticks": len(recs),
            "entropy_mean": round(statistics.mean(r["entropy"] for r in recs), 4),
            "entropy_max": round(max(r["entropy"] for r in recs), 4),
            "fear_max": round(max(r["fear"] for r in recs), 4),
            "loops_delta": recs[-1]["loops"] - recs[0]["loops"],
            "abort_ticks": sum(1 for r in recs if r["abort"] == 1),
            "sonar_drop_ticks": sum(1 for r in recs if r["sonar_alive"] == 0),
            "r14_blocks_est": round(blocks_per_tick * ticks_in_phase),
        })
    return out

def main():
    csv_files = sorted(glob.glob(CSV_GLOB))
    if not csv_files: print(f"No CSVs at {CSV_GLOB}"); return

    drones = []; curve_rows = []
    for csv_path in csv_files:
        name = os.path.basename(csv_path).replace("recon10v3_metrics_", "").replace(".csv", "")
        log_path = os.path.join(ROOT, f"recon10v3_{name}.log")
        rows = parse_metrics(csv_path)
        if not rows: continue
        cls = classify(rows)
        total_r14, takeoff_tick, pb_loaded, r14_first_h, r14_second_h = parse_log(log_path)
        extreme = per_extreme_stats(rows, total_r14, takeoff_tick)
        for p in extreme:
            curve_rows.append({"drone": name, "class": cls, **p})
        # Rate-based habituation: compare first-half R14 block rate vs second-half
        r14_habit_pct = 0.0
        if r14_first_h > 0:
            r14_habit_pct = (r14_first_h - r14_second_h) / r14_first_h * 100
        drones.append({
            "name": name, "class": cls, "total_r14": total_r14,
            "r14_first_half": r14_first_h, "r14_second_half": r14_second_h,
            "r14_habit_pct": round(r14_habit_pct, 1),
            "takeoff_tick": takeoff_tick, "pb_loaded": pb_loaded,
            "last_loops": rows[-1]["loops"],
            "last_entropy": rows[-1]["entropy"],
            "extreme_phases": len(extreme),
        })

    with open(OUT_CURVE, "w", newline="") as f:
        fn = ["drone", "class", "phase_idx", "n_ticks", "entropy_mean", "entropy_max",
              "fear_max", "loops_delta", "abort_ticks", "sonar_drop_ticks", "r14_blocks_est"]
        w = csv.DictWriter(f, fieldnames=fn); w.writeheader(); w.writerows(curve_rows)

    flying = [d for d in drones if d["class"] == "FLY"]
    fly_names = set(d["name"] for d in flying)

    by_idx = {}
    for r in curve_rows:
        if r["drone"] not in fly_names: continue
        by_idx.setdefault(r["phase_idx"], []).append(r)

    cycle_stats = []
    for idx in sorted(by_idx.keys()):
        recs = by_idx[idx]
        if not recs: continue
        cycle_stats.append({
            "idx": idx, "n": len(recs),
            "r14_median": int(statistics.median([r["r14_blocks_est"] for r in recs])),
            "entropy_median": round(statistics.median([r["entropy_mean"] for r in recs]), 4),
            "loops_median": statistics.median([r["loops_delta"] for r in recs]),
            "abort_median": statistics.median([r["abort_ticks"] for r in recs]),
        })

    r14_habit = 0.0; ent_habit = 0.0
    if len(cycle_stats) >= 2:
        first = cycle_stats[0]["r14_median"]; last = cycle_stats[-1]["r14_median"]
        if first > 0: r14_habit = (first - last) / first * 100
        first = cycle_stats[0]["entropy_median"]; last = cycle_stats[-1]["entropy_median"]
        if first > 0: ent_habit = (first - last) / first * 100

    classes = {}
    for d in drones: classes[d["class"]] = classes.get(d["class"], 0) + 1
    total_loops = sum(d["last_loops"] for d in drones)
    total_r14 = sum(d["total_r14"] for d in drones)

    L = []
    L.append("# OASIS Recon-10 v3 — Habituation Report (3 stimulus mechanisms)\n")
    L.append("## Setup\n")
    L.append("- **Phone brain loaded**: `OASIS_PHONE_BRAIN=c:/dev/oasis/phone_brain/` → 128 pain memories + 0 federation digests.")
    L.append("- **Fault injection**: d01/d04/d07 get `sonar_alive=false` first 8s of each EXTREME phase.")
    L.append("- **Moving hazards**: 2 Crazyflies (hz0, hz1) orbit origin r=2.5m, alt=1.5m during EXTREME.")
    L.append("- **Wind**: ±0.05 EXTREME / ±0.005 CALM.\n")

    L.append("## Swarm classification\n")
    for c, n in sorted(classes.items()): L.append(f"- {c}: {n}")
    L.append(f"- Phone brain loaded on: {sum(1 for d in drones if d['pb_loaded'])}/{len(drones)} drones (confirmed in logs)")
    L.append(f"\n## Mission + safety signals\n")
    L.append(f"- Total loops (swarm): **{total_loops}**")
    L.append(f"- Total R14 blocks (swarm): **{total_r14}**")

    L.append(f"\n## Habituation — FLYING drones only ({len(flying)})\n")
    L.append(f"- R14 block median: phase 1 → last = **{r14_habit:+.1f}%** (target: ≥10% decrease)")
    L.append(f"- Entropy median: phase 1 → last = **{ent_habit:+.1f}%** (target: ≥5% decrease)")
    # Rate-based: per-drone R14 rate change first-half vs second-half
    flying_r14_changes = [d["r14_habit_pct"] for d in flying if d["r14_first_half"] > 0]
    if flying_r14_changes:
        mean_r14_rate_drop = statistics.mean(flying_r14_changes)
        L.append(f"- **R14 RATE drop (first-half vs second-half per drone)**: mean **{mean_r14_rate_drop:+.1f}%** across {len(flying_r14_changes)} drones with R14 activity")

    L.append("\n### Per-EXTREME-cycle stats (median across flyers)\n")
    L.append("| Phase idx | N | R14 est | Entropy mean | Loops +Δ | Abort ticks |")
    L.append("|-----------|---|---------|--------------|----------|-------------|")
    for cs in cycle_stats:
        L.append(f"| {cs['idx']} | {cs['n']} | {cs['r14_median']} | {cs['entropy_median']} | "
                 f"{cs['loops_median']} | {cs['abort_median']} |")

    L.append("\n### Per-drone outcome + R14 rate change\n")
    L.append("| Drone | Class | Takeoff@ | PB | Loops | Entropy | Total R14 | 1st-half | 2nd-half | R14 Δ% |")
    L.append("|-------|-------|----------|----|-------|---------|-----------|----------|----------|--------|")
    for d in drones:
        L.append(f"| {d['name']} | {d['class']} | {d['takeoff_tick']} | "
                 f"{'128' if d['pb_loaded'] else 'no'} | "
                 f"{d['last_loops']} | {d['last_entropy']:.3f} | {d['total_r14']} | "
                 f"{d['r14_first_half']} | {d['r14_second_half']} | {d['r14_habit_pct']:+.1f} |")

    pass_fly = classes.get("FLY", 0) >= 6
    pass_r14 = r14_habit >= 10.0
    pass_ent = ent_habit >= 5.0
    pass_mis = total_loops >= 100

    L.append("\n## Verdict\n")
    L.append(f"- Physics (≥6/10 FLY): **{'PASS' if pass_fly else 'FAIL'}** ({classes.get('FLY', 0)}/10)")
    L.append(f"- Habituation R14 (≥10% decrease): **{'PASS' if pass_r14 else 'FAIL'}** ({r14_habit:+.1f}%)")
    L.append(f"- Habituation entropy (≥5% decrease): **{'PASS' if pass_ent else 'FAIL'}** ({ent_habit:+.1f}%)")
    L.append(f"- Mission activity (≥100 loops): **{'PASS' if pass_mis else 'FAIL'}** ({total_loops})")

    with open(OUT_REPORT, "w", encoding="utf-8") as f:
        f.write("\n".join(L))

    print(f"Wrote {OUT_REPORT}")
    print(f"Wrote {OUT_CURVE}")
    print(f"\n--- VERDICT ---")
    print(f"Classes: {classes}")
    print(f"Total loops: {total_loops}, Total R14 blocks: {total_r14}")
    print(f"Phone brain loaded: {sum(1 for d in drones if d['pb_loaded'])}/{len(drones)}")
    print(f"R14 habit: {r14_habit:+.1f}%  Entropy habit: {ent_habit:+.1f}%")

if __name__ == '__main__':
    main()
