# Tables for orders4.py: calibration coefficients, recall at 0.5/1.6/6.3/25% per dataset, distance to the clairvoyant
# ceiling, worst-case loss against the oracle, the collapse check (E2) and the global stopping rule (E7).
# usage: python -I table4.py RESDIR OUT.txt MAIN1,... [EXTRA1,...]
import sys, os, json
import numpy as np

rd, outp = sys.argv[1], sys.argv[2]
main = sys.argv[3].split(",")
extra = sys.argv[4].split(",") if len(sys.argv) > 4 else []
R = {n: json.load(open(os.path.join(rd, f"{n}.json"))) for n in main + extra}
SH = ["0.005", "0.016", "0.063", "0.25"]
SHOW = ["zeta cal-mixed r16 model row", "zeta cal-mixed r16 model probe", "lin cal-mixed r16 mu+3.3sigma", "zeta cal-mixed r16 running row",
        "zeta cal-plain r16 model row", "lin cal-plain r16 mu+3.3sigma",
        "zeta cal-mixed r4 model row", "zeta cal-mixed r4 model probe", "lin cal-mixed r4 mu+3.3sigma",
        "zeta cal-plain r4 model row",
        "zeta raw r16 model row", "lin raw r16 mu+3.3sigma", "zeta exact model row",
        "H r16", "H r4", "O2 f'=0.4", "oracle", "faiss nlist=K", "zeta exact true row", "clairvoyant N_c/n_c"]
L = []
p = L.append
get = lambda r, k: r["orders"][f"rad f={r['oracle']}"] if k == "oracle" else r["orders"][k]
p("score: recall@10 at 0.5 / 1.6 / 6.3 / 25% scanned, and the mean over 0.5%..25% ('avg')")
p("\ncalibration sigma~^2 = a sigmahat_r^2 + b sd(q)^2 rad^2 + c0 sd(q)^2 (log-domain fit; variance-domain NNLS in brackets); "
  "relative error of sigma on plain / shifted pseudo-queries, calibrated (raw)")
seen = set()
for n in main + extra:
    ds = R[n]["dataset"]
    if ds in seen:
        continue
    seen.add(ds)
    for k, v in R[n]["calib"].items():
        p(f"  {ds:28s} {k:10s} a {v['log'][0]:.3f} b {v['log'][1]:.4f} c0 {v['log'][2]:.4f}  [a {v['var'][0]:.3f} b {v['var'][1]:.4f} "
          f"c0 {v['var'][2]:.4f}]  err plain {v['sigma_relerr_plain']:.3f} ({v['sigma_relerr_raw_plain']:.3f}), shifted "
          f"{v['sigma_relerr_shifted']:.3f} ({v['sigma_relerr_raw_shifted']:.3f})")
for n in main + extra:
    r = R[n]
    cl = r["orders"]["clairvoyant N_c/n_c"]
    p(f"\n{n} (K {r['K']}, nq {r['nq']}, oracle f {r['oracle']})                      gap to clairvoyant at 1.6% / 6.3% / avg")
    for k in SHOW:
        o = get(r, k)
        p(f"  {k:32s} " + " ".join(f"{o['fixed'][s]:.3f}" for s in SH) + f"   avg {o['avg']:.3f}   "
          f"{cl['fixed']['0.016'] - o['fixed']['0.016']:+.3f} {cl['fixed']['0.063'] - o['fixed']['0.063']:+.3f} {cl['avg'] - o['avg']:+.3f}")


def losses(sets, title):
    p(f"\n{title}: loss against the oracle (avg / 1.6% / 6.3%, worst over {', '.join(sets)}; negative = better), and against H r16 / H r4 (avg, worst)")
    keys = [k for k in R[sets[0]]["orders"] if not k.startswith("rad f=")]
    rows = []
    for k in keys:
        la = [get(R[n], "oracle")["avg"] - get(R[n], k)["avg"] for n in sets]
        l16 = [get(R[n], "oracle")["fixed"]["0.016"] - get(R[n], k)["fixed"]["0.016"] for n in sets]
        l63 = [get(R[n], "oracle")["fixed"]["0.063"] - get(R[n], k)["fixed"]["0.063"] for n in sets]
        h16 = [get(R[n], "H r16")["avg"] - get(R[n], k)["avg"] for n in sets]
        h4 = [get(R[n], "H r4")["avg"] - get(R[n], k)["avg"] for n in sets]
        rows.append((max(la), k, la, l16, l63, h16, h4))
    for w, k, la, l16, l63, h16, h4 in sorted(rows):
        p(f"  {k:32s} {max(la):+.3f} / {max(l16):+.3f} / {max(l63):+.3f}   per set " + " ".join(f"{x:+.3f}" for x in la) +
          f"   vs H r16 worst {max(h16):+.3f} mean {np.mean(h16):+.3f}; vs H r4 worst {max(h4):+.3f} mean {np.mean(h4):+.3f}")


losses(main, "main sets")
if extra:
    losses(extra, "extra sets")
p("\nE2 collapse: share of a cluster's rows above tau_true, by zeta(tau_true) with exact sigma (bins centred at 0.75 .. 4.75);"
  " Gaussian tail for reference; split by rad, n_c and kurtosis terciles")
for n in main:
    c = R[n]["collapse"]
    g = dict((round(b, 2), v) for b, v in R[n]["gaussian_tail"])
    cols = [0.75, 1.75, 2.75, 3.75, 4.75]
    p(f"  {n}: zeta          " + " ".join(f"{z:>9.2f}" for z in cols))
    p(f"    {'Gaussian':18s} " + " ".join(f"{g[z]:9.2e}" for z in cols))
    for k in ["all", "rad low", "rad high", "n_c low", "n_c high", "kurtosis low", "kurtosis high"]:
        d = {round(b[0], 2): b for b in c[k]}
        p(f"    {k:18s} " + " ".join(f"{d[z][2]:9.2e}" if d[z][1] > 20 else f"{'-':>9s}" for z in cols))
p("\nE7 stopping rule: recall at equal mean rows scanned; global threshold on P(score > tau_model) vs fixed nprobe vs fixed rows per query")
for n in main + extra:
    st = R[n]["stopping"]
    for sname in ("cal-mixed r16", "cal-mixed r4", "exact"):
        if sname in st:
            p(f"  {n:20s} {sname:14s} " + "  ".join(f"{float(s) * 100:g}%: {st[sname][s]['global_lambda']:.3f} / {st[sname][s]['fixed_nprobe']:.3f} / "
                                                   f"{st[sname][s]['per_query_rows']:.3f}" for s in ("0.005", "0.016", "0.063", "0.25")))
txt = "\n".join(L)
print(txt)
open(outp, "w").write(txt + "\n")
