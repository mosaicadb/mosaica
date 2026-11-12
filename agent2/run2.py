# Deterministic fixes in the existing basis: extra stored weightings, optimised mean coefficient, banded error norms, mixture-fitted bits.
import numpy as np, json, sys
from harness import *
I = load(); X = I["X"]; Qi = I["Qi"]
P = json.load(open("text_q.json")); T = np.load("text_q.npy"); qi = np.load("item_q_idx.npy"); Ttr = np.load("text_train.npy")
R = unit(np.random.default_rng(11).standard_normal((30, X.shape[1])))
Q = np.concatenate([X[qi], T, R]); cls = np.array(["item"] * len(qi) + ["text"] * len(T) + ["random"] * len(R))
thr = thresholds(X, Q); Qt = text_profile(I, Ttr); Qmix = ((np.sqrt(Qi) + np.sqrt(Qt)) / 2) ** 2
B9 = [0, 4, 8, 16, 32, 64, 128, 256, 512, 768]; B4 = [0, 32, 128, 384, 768]
cfgs = [dict(name="base", bits=Qi, weights=[Qi], alpha=False, extra=0),
        dict(name="w2", bits=Qi, weights=[Qi, Qt], alpha=False, extra=1),
        dict(name="w2+alpha", bits=Qi, weights=[Qi, Qt], alpha=True, extra=1),
        dict(name="band9+alpha", bits=Qi, weights=[Qi], alpha=True, bands=B9, extra=8),
        dict(name="band4+alpha", bits=Qi, weights=[Qi], alpha=True, bands=B4, extra=3),
        dict(name="mixbits+band4+alpha", bits=Qmix, weights=[Qmix], alpha=True, bands=B4, extra=3)]
sel = sys.argv[1:] or [c["name"] for c in cfgs]
for c in cfgs:
    if c["name"] not in sel: continue
    r = run(I, Q, thr, c); np.savez(f"res2_{c['name']}.npz", cls=cls, **r)
    print(c["name"], "violations", int(r["viol"].sum()), "pairs x levels", len(Q) * r["N"] * len(LEVELS))
    show(report(r, cls, ["item", "text", "random"]), c["name"] + " (no surcharge for extra scalars)")
    if c["extra"]:
        rep = report(r, cls, ["item", "text", "random"], extra=c["extra"])
        print(f"   with {c['extra']} extra scalar bytes per level read, d=0 read %:", {k: [round(100 * v["read"][ti][0], 2) for ti in range(4)] for k, v in rep.items()})
        for st in (4, 6):
            rep = report(r, cls, ["item", "text", "random"], extra=c["extra"], start=st)
            print(f"   same, every item starts at the {LEVELS[st]} B level:", {k: [round(100 * v["read"][ti][0], 2) for ti in range(4)] for k, v in rep.items()})
