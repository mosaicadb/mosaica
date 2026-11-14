# Tables from measure.py output: mean over queries of the share of items (percent) left undecided.
import json, sys, numpy as np
R = json.load(open(sys.argv[1])); N = R["N"]; E = R["edges"]; NB = len(E) - 1
def pct(v): return " / ".join(f"{100 * x / N:5.1f}" if 100 * x / N >= 9.95 else f"{100 * x / N:5.2f}" for x in v)
for L, r in R["levels"].items():
    print(f"\n=== level {L}: K = {r['K']}, median group size {r['median_size']}, rms residual {r['rms_residual']:.4f}, "
          f"half log-det bits (top 16/64/256) {r['half_logdet_bits']}, violations {r['violations']}")
    for kind in ("text", "item"):
        pq = [p for p in r["per_query"] if p["kind"] == kind]
        if not pq: continue
        m = lambda key: np.mean([p["counts"][key] for p in pq], axis=0)
        d = lambda key: float(np.mean([p[key] for p in pq]))
        print(f"\n-- {kind} queries ({len(pq)}); answer sizes 0.01 / 0.1 / 1 / 5 %; thresholds {np.mean([p['th'] for p in pq], axis=0).round(3)}")
        print(f"   between-cluster share of score variance {d('between_share'):.3f}; sd total {d('sd_total'):.4f}, within {d('sd_within'):.4f}; "
              f"threshold minus median {(np.mean([p['th'] for p in pq], axis=0) - d('median')).round(3)}")
        print(f"   |q_perp| {d('qperp'):.3f}; plain Cauchy-Schwarz slack {d('cs_mean'):.3f}; block slack {d('blockcs_mean'):.3f}; "
              f"rms cosine(q_perp, x_perp) {d('cos_rms'):.4f} (effective dimension {1 / d('cos_rms') ** 2:.0f})")
        print("   offset energy by block  ", np.round(np.mean([p["dn2_block"] for p in pq], axis=0), 4).tolist())
        print("   residual energy by block", np.round(r["lam_block"], 4).tolist())
        print(f"   sorted range only          {pct(m('annulus'))}")
        if len(pq[0]["counts"]["table"]): print(f"   range + support table      {pct(m('table'))}")
        cl = m("cluster"); ca = m("cluster_ann"); me = m("member")
        print("   group summary: first k local coordinates' extremes exact (ideal), rest by block norms; with sorted range")
        for j in range(NB + 1): print(f"     k = {E[j]:4d}                 {pct(ca[:, j])}     (groups alone {pct(cl[:, j])})")
        print("   same information per member: first k coordinates exact, rest by block norms")
        for j in range(NB + 1): print(f"     k = {E[j]:4d}                 {pct(me[:, j])}")
        print(f"   oracle, extremes to +-0.05 {pct(m('oracle05'))}")
        print(f"   oracle, extremes to +-0.02 {pct(m('oracle02'))}")
        print(f"   oracle, exact extremes     {pct(m('oracle'))}")

# Cost in coordinates per item of an adaptive per-member read (each item reads blocks until decided),
# plus the centres, at each level.
print("\n=== adaptive per-member read: mean coordinates read per item (plus n*K/N for the centres)")
n = R["n"]
for L, r in R["levels"].items():
    row = []
    for kind in ("text", "item"):
        pq = [p for p in r["per_query"] if p["kind"] == kind]
        me = np.mean([p["counts"]["member"] for p in pq], axis=0) / N          # answer size x ladder
        area = sum((E[j + 1] - E[j]) * me[:, j] for j in range(NB))
        row.append(f"{kind}: " + " / ".join(f"{a:6.1f}" for a in area))
    print(f"level {L} (K = {r['K']:6d}, centres {n * r['K'] / N:5.1f}): " + "   ".join(row))
