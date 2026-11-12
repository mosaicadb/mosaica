# Damage measurement: the existing scheme (item-fitted bits and weights) on item queries and on real text queries.
import numpy as np, json, sys
from harness import *
I = load(); X = I["X"]
P = json.load(open("text_q.json")); T = np.load("text_q.npy"); qi = np.load("item_q_idx.npy")
Q = np.concatenate([X[qi], T]); cls = np.array(["item"] * len(qi) + [p[0] for p in P])
thr = thresholds(X, Q); np.save("thr_all.npy", thr)
for k in ["item"] + sorted(set(p[0] for p in P)): print(k, "mean thresholds", np.round(thr[cls == k].mean(0), 3))
names = ["item", "text(all)"] + sorted(set(p[0] for p in P))
r = run(I, Q, thr, dict(name="base", bits=I["Qi"], weights=[I["Qi"]], alpha=False))
np.savez("res_base.npz", cls=cls, **r)
print("violations per level", r["viol"].tolist(), "total", int(r["viol"].sum()), "pairs", len(Q) * r["N"])
print("which bound is tightest (plain, weighted) share per level:", np.round(r["which"] / r["which"].sum(1, keepdims=True), 2).tolist())
show(report(r, cls, names), "existing scheme (item-fitted)")
