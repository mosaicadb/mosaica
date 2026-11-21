"""Tables for NOTEBOOK.md from out/replay_*.json (replay_np.py). usage: python3 tables.py > out/tables.md"""
import json, math, os
from scipy.stats import norm

here = os.path.dirname(os.path.abspath(__file__))
D = {}
for ds in ("yahoo", "laion", "glove"):
    p = f"{here}/../out/replay_{ds}.json"
    if os.path.exists(p):
        D[ds] = json.load(open(p))
r0 = lambda x, n=0: "-" if x is None else (f"{x:,.0f}" if n == 0 else f"{x:.{n}f}")

print("## T1. The variance profile: share f of the row scores' variance along q explained by each partition, and its entropy position g = ln(cells)/ln N\n")
print("| dataset | N | groups: g, f | clusters: g, f | a4: g, f | a16: g, f | a64: g, f | sup over the measured points of g - f |")
print("|---|---|---|---|---|---|---|---|")
for ds, d in D.items():
    pr = d["profile"]
    print(f"| {ds} | {d['N']:,} | " + " | ".join(f"{pr[k]['g']:.3f}, {pr[k]['f_mean']:.3f} ({pr[k]['f_p10']:.2f}-{pr[k]['f_p90']:.2f})" for k in ("groups", "clusters", "a4", "a16", "a64")) + f" | {max(pr[k]['g'] - pr[k]['f_mean'] for k in pr):.3f} |")

print("\n## T2. The top-10 level in units of the total spread of the scores, against the Gaussian (REM) level\n")
print("| dataset | N: measured (p10-p90), Gaussian | N/4 | N/16 | N/64 | growth per e-fold of N: measured, Gaussian |")
print("|---|---|---|---|---|---|")
for ds, d in D.items():
    z = d["zT"]
    g = (z["N/1"]["zT_mean"] - z["N/64"]["zT_mean"]) / math.log(64); gg = (z["N/1"]["rem_level"] - z["N/64"]["rem_level"]) / math.log(64)
    print(f"| {ds} | {z['N/1']['zT_mean']:.2f} ({z['N/1']['zT_p10']:.2f}-{z['N/1']['zT_p90']:.2f}), {z['N/1']['rem_level']:.2f} | " + " | ".join(f"{z[k]['zT_mean']:.2f}, {z[k]['rem_level']:.2f}" for k in ("N/4", "N/16", "N/64")) + f" | {g:.2f}, {gg:.2f} |")

print("\n## T3. Regression to the mean at the cluster level: the standardised centre score of a true neighbour's cluster\n")
print("| dataset | mean measured | bridge f z_T | residual sd measured | model sqrt(f(1-f)) | standardised residual 1 / 5 / 50 / 95 / 99 pct | centre scores: excess kurtosis | corr(mu_c, sigma_c) within a query | cv of sigma_c |")
print("|---|---|---|---|---|---|---|---|---|")
for ds, d in D.items():
    a = d["ancestor"]; c = d["centres"]; q = a["std_resid_q"]
    print(f"| {ds} | {a['mean_measured']:.2f} | {a['mean_bridge']:.2f} | {a['resid_sd_measured']:.2f} | {a['resid_sd_model']:.2f} | {q[0]:.1f} / {q[1]:.1f} / {q[3]:.1f} / {q[5]:.1f} / {q[6]:.1f} | {c['A_excess_kurtosis']:.1f} | {c['corr_mu_sigma']:.2f} | {c['sigma_cv_within_query']:.2f} |")

print("\n## T4. The rank-share law. Share of rows (%) in cells keyed at or above a true neighbour's cell, 0.9 and 0.95 quantiles over neighbours; key = centre score | key = mu + 3 sigma | Gaussian law at the Gaussian level | Gaussian law at the measured level\n")
for ds, d in D.items():
    print(f"\n**{ds}**\n")
    print("| partition (cells) | rows | 0.9: centre, mu+3sigma, law (Gaussian level), law (measured level) | 0.95: the same |")
    print("|---|---|---|---|")
    for k, v in d["rank_share"].items():
        print(f"| {k} ({v['cells']:,}) | {v['rows']:,} | " + " | ".join(f"{100 * v['U_centre_q'][r]:.2f}, {100 * v['U_mu3sigma_q'][r]:.2f}, {100 * v['law_at_rem_level'][r]:.2f}, {100 * v['law_at_measured_zT'][r]:.3f}" for r in ("0.9", "0.95")) + " |")
    # exponents at a fixed number of cells (N varies) and at fixed N (cells vary), centre key and law at the Gaussian level
    rs = d["rank_share"]
    def slope(keys, x, y):
        import numpy as np
        xs = [math.log(x(rs[k])) for k in keys]; ys = [math.log(y(rs[k])) for k in keys]
        return float(np.polyfit(xs, ys, 1)[0])
    for r in ("0.9", "0.95"):
        line = f"- recall {r}: "
        for part in ("clusters", "a16"):
            keys = [f"{part} N/{f}" for f in (1, 4, 16, 64)]
            line += f"{part}, cells fixed, d ln share / d ln N: centre {slope(keys, lambda v: v['rows'], lambda v: v['U_centre_q'][r]):+.2f}, mu+3sigma {slope(keys, lambda v: v['rows'], lambda v: v['U_mu3sigma_q'][r]):+.2f}, law {slope(keys, lambda v: v['rows'], lambda v: v['law_at_rem_level'][r]):+.2f}; "
        keys = [f"{p} N/1" for p in ("clusters", "a4", "a16", "a64")]
        line += f"N fixed, -d ln share / d ln cells (gamma): centre {-slope(keys, lambda v: v['cells'], lambda v: v['U_centre_q'][r]):.2f}, mu+3sigma {-slope(keys, lambda v: v['cells'], lambda v: v['U_mu3sigma_q'][r]):.2f}, law {-slope(keys, lambda v: v['cells'], lambda v: v['law_at_rem_level'][r]):.2f}; "
        diag = [("a16 N/1", "a4 N/4", "clusters N/16"), ("a64 N/1", "a16 N/4", "a4 N/16", "clusters N/64")]
        for kk in diag:
            line += f"cell size fixed ({kk[0].split()[0]} at N down to {kk[-1]}), d ln rows / d ln N (eta): centre {slope(kk, lambda v: v['rows'], lambda v: v['rows'] * v['U_centre_q'][r]):.2f}, mu+3sigma {slope(kk, lambda v: v['rows'], lambda v: v['rows'] * v['U_mu3sigma_q'][r]):.2f}, law {slope(kk, lambda v: v['rows'], lambda v: v['rows'] * v['law_at_rem_level'][r]):.2f}; "
        print(line)

print("\n## T5. The pooled shape of the standardised within-cluster scores, P(z > x)\n")
print("| x | normal | " + " | ".join(f"{ds}: test queries, corpus rows as queries" for ds in D) + " |")
print("|---|---|" + "---|" * len(D))
zs = next(iter(D.values()))["shape"]["z"]
for j, z in enumerate(zs):
    print(f"| {z} | {norm.sf(z):.2e} | " + " | ".join(f"{d['shape']['test_queries'][j]:.2e}, {d['shape']['corpus_rows_as_queries'][j]:.2e}" for d in D.values()) + " |")
print("\nlocal decay rate -d ln P / dx between x = 3 and 6: " + "; ".join(f"{ds}: test {math.log(d['shape']['test_queries'][3] / d['shape']['test_queries'][8]) / 3:.2f}, corpus rows {math.log(d['shape']['corpus_rows_as_queries'][3] / d['shape']['corpus_rows_as_queries'][8]) / 3:.2f}" for ds, d in D.items()) + " (normal: 4.7)")

print("\n## T6. The collapse table at the true 10th best, unfiltered, 100 queries: rows above it in clusters at zeta, against each tail's prediction\n")
for ds, d in D.items():
    c = d["collapse"]; e = c["zeta_edges"]
    print(f"\n**{ds}**\n")
    print("| zeta | rows (thousands) | true neighbours | normal | pooled shape, test queries | pooled shape, corpus rows | clusters after the first ten: true | predicted by the first ten's own tail |")
    print("|---|---|---|---|---|---|---|---|")
    for j in range(len(e) - 1):
        if c["rows"][j] == 0:
            continue
        lo = "-inf" if e[j] is None else f"{e[j]:g}"; hi = "inf" if e[j + 1] is None else f"{e[j + 1]:g}"
        print(f"| {lo} to {hi} | {c['rows'][j] / 1e3:,.0f} | {c['hits'][j]:.0f} | {c['normal'][j]:.1f} | {c['shapeQ'][j]:.0f} | {c['shapeP'][j]:.0f} | {c['far_hits'][j]:.0f} | {c['near_tail'][j]:.1f} |")
    print(f"| all | {sum(c['rows']) / 1e3:,.0f} | {sum(c['hits']):.0f} | {sum(c['normal']):.0f} | {sum(c['shapeQ']):.0f} | {sum(c['shapeP']):.0f} | {sum(c['far_hits']):.0f} | {sum(c['near_tail']):.0f} |")

print("\n## T7. Stops, replayed: mean rows read at mean recall 0.9 / 0.95 / 0.98 / 0.99 (whole clusters, order mu + 3 sigma, exact sigma, k = 10, 100 queries)\n")
for ds, d in D.items():
    print(f"\n**{ds}**\n")
    sets = list(d["sets"])
    print("| stop | " + " | ".join(sets) + " |")
    print("|---|" + "---|" * len(sets))
    for name in d["sets"][sets[0]]["rules"]:
        print(f"| {name} | " + " | ".join("/".join(r0(d["sets"][s]["rules"][name]["rows_at"][t]) for t in ("0.9", "0.95", "0.98", "0.99")) for s in sets) + " |")

print("\n## T8. What a nominal setting delivers: mean recall (share of queries below 0.9) [mean rows]\n")
for ds, d in D.items():
    print(f"\n**{ds}**\n")
    sets = list(d["sets"])
    print("| rule and setting | " + " | ".join(sets) + " |")
    print("|---|" + "---|" * len(sets))
    def cell(s, name, par):
        e = next((x for x in d["sets"][s]["rules"][name]["curve"] if abs(x["par"] - par) < 1e-9), None)
        return "-" if e is None else f"{e['recall']:.3f} ({e['below_0.9']:.2f}) [{e['rows']:,.0f}]"
    for name, par, lab in (("S0 psi (body moments)", 4.0, "psi 4"), ("S0 psi (body moments)", 4.5, "psi 4.5"), ("S0 psi (body moments)", 5.0, "psi 5"), ("S0 psi (body moments)", 6.0, "psi 6"),
                           ("S1 Gaussian, body moments", 1, "normal tail, 1 expected miss (nominal 0.9)"), ("S1 two upper quantiles", 1, "two upper quantiles, 1 miss"),
                           ("S1 generalised Pareto", 1, "generalised Pareto, 1 miss"), ("S1 exponential tail", 1, "exponential tail, 1 miss"),
                           ("S1 shape of test queries, body moments", 1, "shape of test queries, 1 miss"), ("S1 shape of corpus rows, body moments", 1, "shape of corpus rows, 1 miss"),
                           ("S1 shape of corpus rows, prior (0,1)", 1, "shape of corpus rows, no (delta, rho), 1 miss"),
                           ("S1 exponential tail", 0.1, "exponential tail, 0.1 miss (nominal 0.99)"), ("S1 shape of corpus rows, body moments", 0.1, "shape of corpus rows, 0.1 miss"), ("S1 generalised Pareto", 0.1, "generalised Pareto, 0.1 miss")):
        print(f"| {lab} | " + " | ".join(cell(s, name, par) for s in sets) + " |")

print("\n## T9. What the query holds when a stop on expected misses decides (first step with prediction below 1; 10 / 50 / 90 pct over queries), and the prediction against the count\n")
for ds, d in D.items():
    print(f"\n**{ds}**\n")
    print("| set | tail | pooled rows | pooled z > 2 | pooled z > 3 | zeta of the nearest unread cluster | at steps predicting 0.3 to 3: mean prediction, rows above the running 10th best in the 40 nearest unread, true misses |")
    print("|---|---|---|---|---|---|---|")
    f3 = lambda v: "-" if v is None else "/".join(f"{x:,.0f}" for x in v)
    f3z = lambda v: "-" if v is None else "/".join(f"{x:.1f}" for x in v)
    for s, o in d["sets"].items():
        for key, lab in (("M_G", "normal, body moments"), ("M_Q2", "two upper quantiles"), ("M_GPD", "generalised Pareto"), ("M_EXP", "exponential"), ("M_SP", "shape of corpus rows")):
            c = o["calibration"][key]; a = c["at_stop_r0.9"]
            print(f"| {s} | {lab} | {f3(a['pooled_rows_p10_p50_p90'])} | {f3(a['pooled_z_above_2_p10_p50_p90'])} | {f3(a['pooled_z_above_3_p10_p50_p90'])} | {f3z(a['zeta_of_nearest_unread_p10_p50_p90'])} | {r0(c['mean_pred'], 2)}, {r0(c['mean_rows_above_running_kth_in_40_nearest_unread'], 2)}, {r0(c['mean_true_misses'], 2)} |")

print("\n## T10. Random-energy quantities of the measured profiles (rank space: the top-k level taken at its Gaussian value)\n")
print("| dataset | slopes of A between the measured points (root, groups, clusters, a4, a16, a64, rows) | natural-speed level x_* / REM level = sum sqrt(df dg) | ground state x_s / REM level (concave hull) | exponent max_j (g_j - f_(j-1)): clusters only | clusters + a16 | clusters + a4 + a16 + a64 | groups + clusters |")
print("|---|---|---|---|---|---|---|---|")
def hull(pts):
    h = []
    for p in pts:
        while len(h) >= 2 and (h[-1][1] - h[-2][1]) * (p[0] - h[-2][0]) <= (p[1] - h[-2][1]) * (h[-1][0] - h[-2][0]):
            h.pop()
        h.append(p)
    return h
for ds, d in D.items():
    pr = d["profile"]
    pts = [(0.0, 0.0)] + [(pr[k]["g"], pr[k]["f_mean"]) for k in ("groups", "clusters", "a4", "a16", "a64")] + [(1.0, 1.0)]
    sl = [(b[1] - a[1]) / (b[0] - a[0]) for a, b in zip(pts, pts[1:])]
    xs = sum(math.sqrt(max(b[1] - a[1], 0) * (b[0] - a[0])) for a, b in zip(pts, pts[1:]))
    h = hull(pts)
    xh = sum(math.sqrt(max(b[1] - a[1], 0) * (b[0] - a[0])) for a, b in zip(h, h[1:]))
    def eta(names):
        g = [0.0] + [pr[k]["g"] for k in names] + [1.0]; f = [0.0] + [pr[k]["f_mean"] for k in names]
        t = [g[j] - f[j - 1] for j in range(1, len(g))]
        return f"{max(t):.3f} ({', '.join(f'{x:.2f}' for x in t)})"
    print(f"| {ds} | {', '.join(f'{x:.2f}' for x in sl)} | {xs:.3f} | {min(xh, 1.0):.3f} | {eta(['clusters'])} | {eta(['clusters', 'a16'])} | {eta(['clusters', 'a4', 'a16', 'a64'])} | {eta(['groups', 'clusters'])} |")
