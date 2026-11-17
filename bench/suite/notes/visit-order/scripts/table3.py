# Tables for orders3.py: per dataset recall at 0.5/1.6/6.3/25% for the chosen orders, each order's worst-case loss
# against the oracle (and against H r16) across the main datasets, f' estimated vs best, and the tail fits.
# usage: python -I table3.py RESDIR OUT.txt MAIN1,MAIN2,... [EXTRA1,...]
import sys, os, json

rd, outp = sys.argv[1], sys.argv[2]
main = sys.argv[3].split(",")
extra = sys.argv[4].split(",") if len(sys.argv) > 4 else []
R = {n: json.load(open(os.path.join(rd, f"{n}.json"))) for n in main + extra}
SH = ["0.005", "0.016", "0.063", "0.25"]
SHOW = ["O2 f'=0.4", "H r4", "H r16", "O2 f'=est", "O3 r16", "O3-t r16",
        "thr-g r4 model row", "thr-g r4 model probe", "thr-g r4 running/1 row", "thr-g r4 running/8 row", "thr-g r4 true row",
        "thr-g r16 model row", "thr-g r16 model probe", "thr-g r16 running/1 row", "thr-g r16 running/1 probe",
        "thr-g r16 running/8 row", "thr-g r16 model->running/8 row", "thr-g r16 true row", "thr-g r16 true probe",
        "thr-g exact model row", "thr-g exact true row",
        "thr-t r16 model row", "thr-t r16 running/1 row", "thr-t r16 true row", "oracle", "faiss nlist=K"]
L = []
p = L.append
get = lambda r, k: r["orders"][f"rad f={r['oracle']}"] if k == "oracle" else r["orders"][k]
p("score: mean recall@10 over scanned shares 0.5%..25% ('avg'); recall at 0.5 / 1.6 / 6.3 / 25% scanned")
p("\ncorpus-only fits per dataset: nu (Student-t from the excess kurtosis on 32 corpus directions), f' estimated vs best")
for n in main + extra:
    r = R[n]
    nb = r["own_best_f2"]
    p(f"  {n:20s} K {r['K']:4d}  excess kurtosis median {r['exk_median']:+.2f} (90% {r['exk_p90']:+.2f}); nu median {r['nu']['median']:.0f} "
      f"(10/90% {r['nu']['p10']:.0f}/{r['nu']['p90']:.0f}, share at 3: {r['nu']['at3']:.2f}); "
      f"f' est {r['f_est']:.3f} (dirs 10/90% {r['f_est_p10']:.2f}/{r['f_est_p90']:.2f}); own best f' {nb}: "
      f"avg est {r['orders']['O2 f' + chr(39) + '=est']['avg']:.3f} / 0.4 {r['orders']['O2 f' + chr(39) + '=0.4']['avg']:.3f} / "
      f"best {r['orders'][f'O2 f' + chr(39) + f'={nb}']['avg']:.3f}; tau_model - tau_true (r16 g) {r['tau_model_minus_true']['r16 g']:+.4f}")
for n in main + extra:
    r = R[n]
    p(f"\n{n} (K {r['K']}, nq {r['nq']}, oracle f {r['oracle']})")
    for k in SHOW:
        o = get(r, k)
        p(f"  {k:32s} " + " ".join(f"{o['fixed'][s]:.3f}" for s in SH) + f"   avg {o['avg']:.3f}")
p("\nloss against the oracle (oracle - order: avg / 1.6% / 6.3%; negative = better), worst case over the main datasets; "
  "then loss against H r16 (avg)")
keys = [k for k in R[main[0]]["orders"] if not k.startswith("rad f=") and not (k.startswith("O2 f'=") and k not in ("O2 f'=0.4", "O2 f'=est"))]
rows = []
for k in keys:
    la = [get(R[n], "oracle")["avg"] - get(R[n], k)["avg"] for n in main]
    l16 = [get(R[n], "oracle")["fixed"]["0.016"] - get(R[n], k)["fixed"]["0.016"] for n in main]
    l63 = [get(R[n], "oracle")["fixed"]["0.063"] - get(R[n], k)["fixed"]["0.063"] for n in main]
    lh = [get(R[n], "H r16")["avg"] - get(R[n], k)["avg"] for n in main]
    rows.append((max(la), k, la, l16, l63, lh))
for w, k, la, l16, l63, lh in sorted(rows):
    p(f"  {k:34s} worst {max(la):+.3f} / {max(l16):+.3f} / {max(l63):+.3f}   per dataset avg " +
      " ".join(f"{x:+.3f}" for x in la) + "   vs H r16: worst " + f"{max(lh):+.3f}, mean {sum(lh) / len(lh):+.3f}")
p("  (datasets in order: " + ", ".join(main) + ")")
if extra:
    p("\nsame on the extra query sets (check):")
    rows = []
    for k in keys:
        la = [get(R[n], "oracle")["avg"] - get(R[n], k)["avg"] for n in extra]
        lh = [get(R[n], "H r16")["avg"] - get(R[n], k)["avg"] for n in extra]
        rows.append((max(la), k, la, lh))
    for w, k, la, lh in sorted(rows)[:25]:
        p(f"  {k:34s} worst vs oracle {w:+.3f}  per set " + " ".join(f"{x:+.3f}" for x in la) + f"   vs H r16 worst {max(lh):+.3f}")
    p("  (sets: " + ", ".join(extra) + ")")
txt = "\n".join(L)
print(txt)
open(outp, "w").write(txt + "\n")
