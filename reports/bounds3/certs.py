# Certificates for the worst-case constants of consts.json: for each configuration the smallest X on a 0.001 lattice,
# starting from exact sup + 0.008, that the adaptive grid certificate accepts.
import json, math, sys, time, numpy as np
from cert2 import certify
C = json.load(open("consts.json"))
only = sys.argv[1:] 
res = json.load(open("certs.json")) if False else {}
for key, row in C.items():
    if only and not any(key.startswith(o) for o in only): continue
    ds = row["ds"]; lnd = -row["l1"]
    big = sum(ds) > 1100
    X = math.ceil((row["exact_sup"] + 0.008) * 1000) / 1000
    t0 = time.time()
    for attempt in range(4):
        ok, mx, K = certify(ds, lnd, round(X, 3), Kmax=4096, eta=0.001, M=131072 if big else 65536, k0=800 if big else 400, verbose=False)
        print(f"{key}: ds {ds} ln delta {lnd:.4f} exact sup {row['exact_sup']:.4f} X {X:.3f} certified {ok} (level K {K}, max ln tail {mx:.5f}) [{time.time()-t0:.0f}s]", flush=True)
        if ok: break
        X += 0.002
    res[key] = dict(X=round(X, 3), ok=bool(ok), K=K, maxlntail=mx)
    json.dump(res, open(f"certs_{'_'.join(only) if only else 'all'}.json", "w"), indent=1)
