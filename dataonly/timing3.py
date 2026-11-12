# Bytes and estimated decode time per item (averaged over the corpus, like B/item), from run2 outputs, using the review's
# single-thread measurements: escape code 72/161/245/492 ns for a 4/8/16/32 B prefix (piecewise linear, last slope beyond);
# table-decodable fixed-rate bytes (fixed-rate deterministic code, randomised levels, scalars) 0.58 ns/B (byte table) or
# 0.22 ns/B (nibble table); exact f16 dot 76 ns at n = 768, scaled by n.  usage: timing.py NAME TAG [TAG ...]
import sys, json, numpy as np
sys.argv = [sys.argv[0]] + sys.argv[1:]
import report2 as R2
EB = np.array([0, 4, 8, 16, 32]); EN = np.array([0, 72, 161, 245, 492.0])
def fesc(b): return np.interp(b, EB, EN) if b <= 32 else 492 + (b - 32) * (492 - 245) / 16
for tag in sys.argv[2:]:
    R = R2.load(tag); cfg = json.loads(R["CFG"]); dcode = cfg.get("DCODE", "esc"); L0 = cfg["L0"]; NORM = cfg["NORM"]
    DLV = ([0] if L0 else []) + cfg["SCHED"]; full = 76 * R2.n / 768
    for cl in np.unique(R2.qcls):
        m = R2.qcls == cl; out = []
        for ti in (range(8) if cl == "item" else range(4)):
            e = R["enter"][:, ti][:, m].mean(1) / R2.N; ib = R["ib"][ti][m].mean() / R2.N
            res = {}
            for TB in (0.58, 0.22):
                ns = 0; bdet = 0; prev = 0
                for l, b in enumerate(DLV):
                    inc = (b - prev) + NORM / 8 + (2 if l == 0 else 0); bdet += e[l] * inc
                    code_ns = (fesc(b) - fesc(prev)) if dcode == "esc" else (b - prev) * TB
                    ns += e[l] * (code_ns + (inc - (b - prev)) * TB); prev = b
                brand = ib - bdet - e[-1] * 2 * R2.n
                ns += brand * TB + e[-1] * full; res[TB] = ns
            out.append((ib, res[0.58], res[0.22]))
        print(f"{R2.NAME} {tag:8s} {dcode:3s} {cl:4s} B/item " + " ".join(f"{o[0]:7.2f}" for o in out) + " | ns/item (byte tbl) " + " ".join(f"{o[1]:6.1f}" for o in out)
              + " | (nibble) " + " ".join(f"{o[2]:6.1f}" for o in out) + f" | flat f16 scan {full:.0f} ns/item")
