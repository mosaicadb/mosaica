# Compare bases at one level: undecided share with no coordinates read, and mean coordinates read per item by an
# adaptive per-member read, by query kind and text sub-kind.
import json, sys, numpy as np
names = sys.argv[1::2]; files = sys.argv[2::2]
subs = ["text", "sci", "tax", "bare", "com", "higher", "free", "oos", "item"]
for name, f in zip(names, files):
    R = json.load(open(f)); N = R["N"]; E = R["edges"]; NB = len(E) - 1; r = R["levels"]["0"]
    print(f"\n{name}: violations {r['violations']}")
    for sub in subs:
        pq = [p for p in r["per_query"] if (p["kind"] == sub if sub in ("text", "item") else p["sub"] == sub)]
        me = np.mean([p["counts"]["member"] for p in pq], axis=0) / N; cl = np.mean([p["counts"]["cluster"] for p in pq], axis=0) / N
        area = sum((E[j + 1] - E[j]) * me[:, j] for j in range(NB))
        print(f"  {sub:6s} ({len(pq):3d})  coordinates per item " + " / ".join(f"{a:6.1f}" for a in area) +
              "   undecided at k=0, per member " + " / ".join(f"{100*a:5.1f}" for a in me[:, 0]) +
              "   groups alone " + " / ".join(f"{100*a:5.1f}" for a in cl[:, 0]) +
              f"   slack {np.mean([p['blockcs_mean'] for p in pq]):.3f}")
        if "norm1" in pq[0]["counts"]:
            g = lambda k: " / ".join(f"{100 * a / N:5.1f}" for a in np.mean([p["counts"][k] for p in pq], axis=0))
            print(f"         one norm: sorted range on it {g('norm1')}; with the annulus range too {g('norm1_ann')}; norm and m.r per member {g('norm2')}; group ball alone {g('ball')}; annulus {g('annulus')}")
