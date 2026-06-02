"""
OASIS Recon-10 — habituation analysis on REAL drone_bridge.exe Rust kernel signals.

Fear values come from the Rust EmotionalState (not a Python reimplementation).
Entropy values come from Rust HyperState Shannon calculation.
inspected/loops come from Rust WorldModel target hit tracking.
"""
import csv, glob, os, statistics

ROOT = "C:/dev/oasis/webots"
GLOB = os.path.join(ROOT, "recon10v2_metrics_*.csv")
OUT_REPORT = os.path.join(ROOT, "recon10v2_habituation_report.md")
OUT_CURVE = os.path.join(ROOT, "recon10v2_habituation_curve.csv")
OUT_MISSION = os.path.join(ROOT, "recon10v2_mission_summary.csv")

def parse_drone(path):
    rows = []
    with open(path) as f:
        for r in csv.DictReader(f):
            try:
                rows.append({
                    "tick": int(r["tick"]),
                    "x": float(r["x"]), "y": float(r["y"]), "alt": float(r["alt"]),
                    "fear": float(r["fear"]), "entropy": float(r["entropy"]),
                    "dvx": float(r["dvx"]), "dvy": float(r["dvy"]),
                    "wind_x": float(r["wind_x"]), "wind_y": float(r["wind_y"]),
                    "min_obs": float(r["min_obs"]),
                    "inspected": int(r["inspected"]), "loops": int(r["loops"]),
                    "abort": int(r["abort"]),
                    "phase": r["env_phase"], "phase_idx": int(r["phase_idx"]),
                })
            except Exception:
                pass
    return rows

def classify(rows):
    if not rows: return "EMPTY"
    # Use second-half stats (after warmup)
    tail = rows[len(rows)//2:]
    mean_alt = statistics.mean(r["alt"] for r in tail)
    max_alt = max(r["alt"] for r in tail)
    mean_abort = statistics.mean(r["abort"] for r in tail)
    if max_alt > 4.0: return "LOST"
    if mean_alt < 0.3: return "GROUND"
    if mean_abort > 0.5: return "ABORT"
    return "FLY"

def per_phase_stats(rows):
    """For each EXTREME phase idx, return fear peak across ticks."""
    by = {}
    for r in rows:
        by.setdefault((r["phase_idx"], r["phase"]), []).append(r)
    out = []
    for (idx, phase), recs in sorted(by.items()):
        if phase == "EXTREME":
            fears = [r["fear"] for r in recs]
            entropies = [r["entropy"] for r in recs]
            winds = [(r["wind_x"]**2 + r["wind_y"]**2)**0.5 for r in recs]
            out.append({
                "phase_idx": idx,
                "fear_peak": max(fears),
                "fear_mean": statistics.mean(fears),
                "entropy_mean": statistics.mean(entropies),
                "wind_std": statistics.pstdev(winds) if len(winds) > 1 else 0.0,
                "n_ticks": len(recs),
            })
    return out

def main():
    files = sorted(glob.glob(GLOB))
    if not files: print(f"No files at {GLOB}"); return
    print(f"Found {len(files)} drone CSVs")

    mission_rows = []; curve_rows = []
    cycle_pool = {}  # phase_idx -> list of peaks (flying drones only)

    for fp in files:
        name = os.path.basename(fp).replace("recon10v2_metrics_", "").replace(".csv", "")
        rows = parse_drone(fp)
        if not rows: continue
        cls = classify(rows)
        last = rows[-1]
        phases = per_phase_stats(rows)
        peak_vals = [p["fear_peak"] for p in phases]
        first3 = peak_vals[:3] if len(peak_vals) >= 3 else peak_vals
        last3 = peak_vals[-3:] if len(peak_vals) >= 3 else peak_vals
        med_first = statistics.median(first3) if first3 else 0.0
        med_last = statistics.median(last3) if last3 else 0.0
        drop = ((med_first - med_last) / med_first * 100) if med_first > 0 else 0.0

        mission_rows.append({
            "drone": name,
            "class": cls,
            "last_tick": last["tick"],
            "last_inspected": last["inspected"],
            "last_loops": last["loops"],
            "last_entropy": round(last["entropy"], 3),
            "extreme_phases": len(phases),
            "fear_peak_first3": round(med_first, 3),
            "fear_peak_last3": round(med_last, 3),
            "habit_drop_pct": round(drop, 1),
        })
        for p in phases:
            curve_rows.append({"drone": name, "class": cls, **{k: (round(v,3) if isinstance(v,float) else v) for k,v in p.items()}})
            if cls == "FLY":
                cycle_pool.setdefault(p["phase_idx"], []).append(p["fear_peak"])

    with open(OUT_CURVE, "w", newline="") as f:
        fn = ["drone", "class", "phase_idx", "fear_peak", "fear_mean", "entropy_mean", "wind_std", "n_ticks"]
        w = csv.DictWriter(f, fieldnames=fn); w.writeheader(); w.writerows(curve_rows)
    with open(OUT_MISSION, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=mission_rows[0].keys())
        w.writeheader(); w.writerows(mission_rows)

    # Per-cycle stats across FLYING drones only
    cycle_stats = []
    for idx in sorted(cycle_pool.keys()):
        peaks = cycle_pool[idx]
        if not peaks: continue
        cycle_stats.append({
            "idx": idx, "n": len(peaks),
            "median": round(statistics.median(peaks), 3),
            "mean": round(statistics.mean(peaks), 3),
            "min": round(min(peaks), 3), "max": round(max(peaks), 3),
        })

    # Aggregate
    flying = [m for m in mission_rows if m["class"] == "FLY"]
    cls_counts = {}
    for m in mission_rows: cls_counts[m["class"]] = cls_counts.get(m["class"], 0) + 1
    total_inspected = sum(m["last_inspected"] for m in mission_rows)
    total_loops = sum(m["last_loops"] for m in mission_rows)
    avg_drop_flying = statistics.mean(m["habit_drop_pct"] for m in flying if m["fear_peak_first3"] > 0) if flying else 0.0
    n_habit_flying = sum(1 for m in flying if m["habit_drop_pct"] > 10)

    # Habituation across cycles
    habit_cycle = 0.0
    if len(cycle_stats) >= 2:
        first = cycle_stats[0]["median"]; last = cycle_stats[-1]["median"]
        if first > 0: habit_cycle = (first - last) / first * 100

    # Write report
    L = []
    L.append("# OASIS Recon-10 — Habituation Report (REAL Rust kernel via drone_bridge.exe)\n")
    L.append("## Setup\n")
    L.append("- 10 drones, each running `drone_bridge.exe` subprocess (22-module Rust kernel, 130 unit tests)")
    L.append("- Fear signal: `EmotionalState::fear` (Mechanism 5) from Rust, not Python")
    L.append("- Entropy signal: HyperState Shannon (Mechanism 2) from Rust")
    L.append("- Wind: injected Python-side onto dvx/dvy before PID (CALM ±0.01, EXTREME ±0.08)\n")

    L.append("## Swarm classification\n")
    L.append(f"- FLY (stable): **{cls_counts.get('FLY', 0)} / {len(mission_rows)}**")
    L.append(f"- GROUND (never took off): {cls_counts.get('GROUND', 0)}")
    L.append(f"- LOST (alt > 4 m): {cls_counts.get('LOST', 0)}")
    L.append(f"- ABORT (bridge triggered abort majority of run): {cls_counts.get('ABORT', 0)}\n")

    L.append("## Mission outcomes\n")
    L.append(f"- Total inspected targets (swarm sum): **{total_inspected}**")
    L.append(f"- Total full coverage loops: **{total_loops}**\n")

    L.append("## Habituation — FLYING drones only\n")
    L.append(f"- Drones with >3 EXTREME phases: {len(flying)}")
    L.append(f"- Mean habituation drop (first-3 vs last-3 per drone): **{avg_drop_flying:.1f}%**")
    L.append(f"- Swarm median habituation drop (phase 1 vs last): **{habit_cycle:.1f}%**")
    L.append(f"- Drones with >10% drop: {n_habit_flying} / {len(flying)}\n")

    L.append("### Per-cycle fear peak (FLYING drones)\n")
    L.append("| Phase idx | N | Median | Mean | Min | Max |")
    L.append("|-----------|---|--------|------|-----|-----|")
    for cs in cycle_stats:
        L.append(f"| {cs['idx']} | {cs['n']} | {cs['median']} | {cs['mean']} | {cs['min']} | {cs['max']} |")

    L.append("\n### Per-drone summary\n")
    L.append("| Drone | Class | Inspected | Loops | Final entropy | EXTREME phases | First-3 peak | Last-3 peak | Drop % |")
    L.append("|-------|-------|-----------|-------|---------------|----------------|--------------|-------------|--------|")
    for m in mission_rows:
        L.append(f"| {m['drone']} | {m['class']} | {m['last_inspected']} | {m['last_loops']} | "
                 f"{m['last_entropy']} | {m['extreme_phases']} | "
                 f"{m['fear_peak_first3']} | {m['fear_peak_last3']} | {m['habit_drop_pct']} |")

    # Verdict
    pass_fly = cls_counts.get("FLY", 0) >= 6
    pass_habit = avg_drop_flying >= 10.0 or habit_cycle >= 10.0
    pass_insp = total_inspected >= 20

    L.append("\n## Verdict\n")
    L.append(f"- Physics reliability (≥6/10 FLY): **{'PASS' if pass_fly else 'FAIL'}** ({cls_counts.get('FLY', 0)}/10)")
    L.append(f"- Habituation (drop ≥10% in flyers OR swarm cycle drop ≥10%): **{'PASS' if pass_habit else 'FAIL'}**")
    L.append(f"- Mission progress (≥20 target inspections): **{'PASS' if pass_insp else 'FAIL'}** ({total_inspected})")

    with open(OUT_REPORT, "w", encoding="utf-8") as f:
        f.write("\n".join(L))

    print(f"Wrote {OUT_REPORT}")
    print(f"Wrote {OUT_CURVE}")
    print(f"Wrote {OUT_MISSION}")
    print("\n--- VERDICT ---")
    print(f"Classes: {cls_counts}")
    print(f"Inspected: {total_inspected}, Loops: {total_loops}")
    print(f"Habit drop (flyers): {avg_drop_flying:.1f}% | Cycle median drop: {habit_cycle:.1f}%")

if __name__ == '__main__':
    main()
