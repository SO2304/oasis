"""
OASIS Recon-20 — habituation + mission analysis.

Reads recon20_metrics_<name>.csv for each drone and produces:
  - habituation_report.md (markdown summary)
  - habituation_curve.csv (per-cycle fear peak per drone)
  - mission_summary.csv (per-drone outcome)

Habituation hypothesis: for each drone, fear_peak in EXTREME phase N
should be LESS than fear_peak in EXTREME phase 1 (baseline).
Pass criterion: median(peak_last_3) < median(peak_first_3) by >= 10%.
"""
import csv, glob, os, statistics

ROOT = "C:/dev/oasis/webots"
GLOB = os.path.join(ROOT, "recon20_metrics_*.csv")
OUT_REPORT = os.path.join(ROOT, "recon20_habituation_report.md")
OUT_CURVE = os.path.join(ROOT, "recon20_habituation_curve.csv")
OUT_MISSION = os.path.join(ROOT, "recon20_mission_summary.csv")

DISTANCE_SCALE = 10.0

def parse_drone(path):
    rows = []
    with open(path) as f:
        rd = csv.DictReader(f)
        for r in rd:
            try:
                rows.append({
                    "tick": int(r["tick"]),
                    "x": float(r["x"]), "y": float(r["y"]), "alt": float(r["alt"]),
                    "fear": float(r["fear"]),
                    "wind_x": float(r["wind_x"]), "wind_y": float(r["wind_y"]),
                    "wind_comp_mag": float(r.get("wind_comp_mag", 0)),
                    "dist": float(r["dist_actual"]),
                    "mission": r["mission"], "phase": r["env_phase"],
                    "phase_idx": int(r["phase_idx"]),
                })
            except Exception:
                pass
    return rows

def per_phase_peaks(rows):
    """Return [(phase_idx, phase, fear_peak, ticks_in_phase, wind_std, wc_mag_end), ...] for EXTREME phases."""
    by_phase = {}
    for r in rows:
        by_phase.setdefault((r["phase_idx"], r["phase"]), []).append(r)
    result = []
    for (idx, phase), recs in sorted(by_phase.items()):
        if phase == "EXTREME":
            fears = [r["fear"] for r in recs]
            winds = [(r["wind_x"] ** 2 + r["wind_y"] ** 2) ** 0.5 for r in recs]
            wstd = statistics.pstdev(winds) if len(winds) > 1 else 0.0
            wc_end = recs[-1].get("wind_comp_mag", 0.0)
            result.append((idx, phase, max(fears), len(recs), round(wstd, 4), round(wc_end, 4)))
    return result

def main():
    files = sorted(glob.glob(GLOB))
    if not files:
        print(f"No CSV files matched {GLOB}")
        return
    print(f"Found {len(files)} drone CSVs")

    mission_rows = []
    curve_rows = []  # per (drone, phase_idx, peak)
    drones_summary = []

    for fp in files:
        name = os.path.basename(fp).replace("recon20_metrics_", "").replace(".csv", "")
        rows = parse_drone(fp)
        if not rows:
            continue
        # mission summary
        last = rows[-1]
        # was LANDED reached?
        landed_tick = next((r["tick"] for r in rows if r["mission"] == "LANDED"), None)
        rth_tick = next((r["tick"] for r in rows if r["mission"] == "RTH"), None)
        # per-drone fear peaks for EXTREME phases
        peaks = per_phase_peaks(rows)
        for (idx, phase, peak, nticks, wstd, wc_end) in peaks:
            curve_rows.append({"drone": name, "phase_idx": idx, "peak": round(peak, 4),
                               "nticks": nticks, "wind_std": wstd, "wind_comp_mag_end": wc_end})

        # peak medians
        peak_vals = [p for _, _, p, _, _, _ in peaks]
        first3 = peak_vals[:3] if len(peak_vals) >= 3 else peak_vals
        last3 = peak_vals[-3:] if len(peak_vals) >= 3 else peak_vals
        med_first = statistics.median(first3) if first3 else 0.0
        med_last = statistics.median(last3) if last3 else 0.0
        habit_drop = ((med_first - med_last) / med_first * 100) if med_first > 0 else 0.0

        mission_rows.append({
            "drone": name,
            "last_tick": last["tick"],
            "dist_actual_m": round(last["dist"], 1),
            "dist_virt_km": round(last["dist"] * DISTANCE_SCALE / 1000.0, 3),
            "final_state": last["mission"],
            "landed": "yes" if landed_tick else "no",
            "landed_tick": landed_tick or "",
            "rth_tick": rth_tick or "",
            "extreme_phases_seen": len(peaks),
            "fear_peak_first3_med": round(med_first, 3),
            "fear_peak_last3_med": round(med_last, 3),
            "habituation_drop_pct": round(habit_drop, 1),
        })
        drones_summary.append({
            "name": name, "peaks": peak_vals, "habit_drop": habit_drop,
            "final_state": last["mission"], "dist": last["dist"],
        })

    # Write CSVs
    with open(OUT_CURVE, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["drone", "phase_idx", "peak", "nticks", "wind_std", "wind_comp_mag_end"])
        w.writeheader(); w.writerows(curve_rows)

    with open(OUT_MISSION, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=mission_rows[0].keys())
        w.writeheader(); w.writerows(mission_rows)

    # Aggregate stats for report
    total_dist_virt_km = sum(m["dist_virt_km"] for m in mission_rows)
    landed_count = sum(1 for m in mission_rows if m["landed"] == "yes")
    rth_count = sum(1 for m in mission_rows if m["rth_tick"])
    avg_habit_drop = statistics.mean(m["habituation_drop_pct"] for m in mission_rows if m["fear_peak_first3_med"] > 0) if mission_rows else 0.0
    drones_with_habituation = sum(1 for m in mission_rows if m["habituation_drop_pct"] > 10.0)

    # Per-cycle group stats (across all drones)
    by_idx = {}
    for r in curve_rows:
        by_idx.setdefault(r["phase_idx"], []).append(r["peak"])
    cycle_stats = []
    by_idx_wind = {}; by_idx_wc = {}
    for r in curve_rows:
        by_idx_wind.setdefault(r["phase_idx"], []).append(r["wind_std"])
        by_idx_wc.setdefault(r["phase_idx"], []).append(r["wind_comp_mag_end"])
    for idx in sorted(by_idx.keys()):
        peaks = by_idx[idx]
        cycle_stats.append({
            "phase_idx": idx,
            "n_drones": len(peaks),
            "median_peak": round(statistics.median(peaks), 3),
            "mean_peak": round(statistics.mean(peaks), 3),
            "min_peak": round(min(peaks), 3),
            "max_peak": round(max(peaks), 3),
            "med_wind_std": round(statistics.median(by_idx_wind.get(idx, [0])), 4),
            "med_wc_mag_end": round(statistics.median(by_idx_wc.get(idx, [0])), 4),
        })

    # Report
    lines = []
    lines.append("# OASIS Recon-20 — Habituation & Mission Report\n")
    lines.append(f"## Mission summary\n")
    lines.append(f"- Drones analyzed: **{len(mission_rows)}**")
    lines.append(f"- Total virtual distance covered: **{total_dist_virt_km:.2f} km** (target: 50.00 km)")
    lines.append(f"- Drones that reached RTH: **{rth_count} / {len(mission_rows)}**")
    lines.append(f"- Drones that LANDED: **{landed_count} / {len(mission_rows)}**\n")

    lines.append(f"## Habituation (Mechanism 5 verification)\n")
    lines.append(f"Hypothesis: fear peak in EXTREME phase N decreases relative to phase 1 baseline.\n")
    lines.append(f"- Mean habituation drop (first-3 vs last-3 EXTREME peaks, per drone): **{avg_habit_drop:.1f}%**")
    lines.append(f"- Drones showing >10% drop: **{drones_with_habituation} / {len(mission_rows)}**\n")

    lines.append("### Per-cycle fear peak + confound controls (across swarm)\n")
    lines.append("| Phase idx | N drones | Median peak | Mean peak | Min | Max | Med wind_std | Med wind_comp_mag |")
    lines.append("|-----------|----------|-------------|-----------|-----|-----|--------------|--------------------|")
    for cs in cycle_stats:
        lines.append(f"| {cs['phase_idx']} | {cs['n_drones']} | {cs['median_peak']} | {cs['mean_peak']} | {cs['min_peak']} | {cs['max_peak']} | {cs['med_wind_std']} | {cs['med_wc_mag_end']} |")
    lines.append("\n*If `Med wind_std` is similar across phases AND `Med wind_comp_mag` grows AND peak drops, habituation is OASIS-attributable.*")

    lines.append("\n### Per-drone summary\n")
    lines.append("| Drone | Final | Dist actual (m) | Dist virtual (km) | Landed | EXTREME phases | First-3 med | Last-3 med | Habit drop (%) |")
    lines.append("|-------|-------|-----------------|-------------------|--------|----------------|-------------|------------|----------------|")
    for m in mission_rows:
        lines.append(f"| {m['drone']} | {m['final_state']} | {m['dist_actual_m']} | {m['dist_virt_km']} | {m['landed']} | {m['extreme_phases_seen']} | {m['fear_peak_first3_med']} | {m['fear_peak_last3_med']} | {m['habituation_drop_pct']} |")

    # Pass/fail
    pass_habit = avg_habit_drop >= 10.0
    pass_dist = total_dist_virt_km >= 25.0  # at least half of 50 km objective
    pass_rth = rth_count >= 10  # at least half of swarm reached RTH

    lines.append("\n## Verdict\n")
    lines.append(f"- Habituation criterion (mean drop >= 10%): **{'PASS' if pass_habit else 'FAIL'}** ({avg_habit_drop:.1f}%)")
    lines.append(f"- Distance criterion (swarm cumulative >= 25 km virtual): **{'PASS' if pass_dist else 'FAIL'}** ({total_dist_virt_km:.2f} km)")
    lines.append(f"- RTH criterion (>= 10 drones reached RTH): **{'PASS' if pass_rth else 'FAIL'}** ({rth_count} drones)")

    with open(OUT_REPORT, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Wrote {OUT_REPORT}")
    print(f"Wrote {OUT_CURVE}")
    print(f"Wrote {OUT_MISSION}")
    print(f"\n--- VERDICT ---")
    print(f"Habituation: {'PASS' if pass_habit else 'FAIL'} ({avg_habit_drop:.1f}%)")
    print(f"Distance:    {'PASS' if pass_dist else 'FAIL'} ({total_dist_virt_km:.2f} km virtual)")
    print(f"RTH:         {'PASS' if pass_rth else 'FAIL'} ({rth_count}/{len(mission_rows)} drones)")

if __name__ == '__main__':
    main()
