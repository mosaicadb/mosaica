# Step 4: Phi on the measured spectra beside Theorems 1, 2, 3, the conjectured floor and the widths of the built codes.
# Spectra: agent1/spec_v1.json (n = 768), the two largest groups (12 and 2), as agent1/theory.py and theory.log use them.
# Bits are counted as theory.py counts them: B = 8 * (code bytes) + 48 (three stored 16-bit scalars); H = B ln 2 nats.
# Widths: agent1/theory.log columns "uniform" and "escape" at code bits 0, 32, 64, 128, 256, 768 = 0, 4, 8, 16, 32, 96 bytes
# (Monte Carlo on synthetic Gaussian data with these spectra, independent Gaussian offsets; computed by agent1/theory.py).
# Also: a scan of every byte count 0..96 on every group of all three spectrum files, for the stop condition.
import math, json, re
import numpy as np
from common import Phi, T1, T2, T3, conj_floor, hybrid_T3_phi, logkappa, WFloor, standard_run_header
from scipy import optimize


def F2_claim(gam, H, ks=(1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768)):
    """The PI's claim F2: max over S = top-k (k in ks, plus the water-level k) of
    min_u [kappa_k GM_S e^{-(H-u)/k} + W_{S^c}(u)]. Returns (value, best k)."""
    g = np.sort(gam)[::-1]; n = len(g); lgc = np.cumsum(np.log(g)); Wf = WFloor(g)
    lt = (Wf.L - 2 * H) / Wf.ks; nxt = np.concatenate([Wf.lg[1:], [-np.inf]])
    kw = int(np.argmax((lt <= Wf.lg + 1e-12) & (lt >= nxt - 1e-12))) + 1
    best = (-1.0, None)
    for k in sorted(set(k for k in ks if k <= n) | {kw}):
        Wt = WFloor(g[k:]) if k < n else None
        lk = logkappa(k) + 0.5 * lgc[k - 1] / k
        f = lambda u: math.exp(lk - (H - u) / k) + (Wt(u) if Wt is not None else 0.0)
        r = optimize.minimize_scalar(f, bounds=(0, H), method="bounded", options={"xatol": 1e-12 * max(1, H)})
        v = min(r.fun, f(0), f(H))
        if v > best[0]: best = (v, k)
    return best

standard_run_header("measured.py")
SCAL = 48
BYTES = [0, 4, 8, 16, 32, 96]

# widths from agent1/theory.log
widths = {}
cur = None
for line in open("../../../agent1/theory.log"):
    m = re.match(r"group (\d+):", line)
    if m:
        cur = m.group(1); widths[cur] = {}; continue
    m = re.match(r"\s+(\d+)\s+\|(.*)", line)
    if m and cur is not None and "|" in line:
        f = [x.strip() for x in m.group(2).split("|")]
        try:
            widths[cur][int(m.group(1))] = (float(f[5]), float(f[6]))     # uniform, escape
        except (ValueError, IndexError):
            pass
    if line.startswith("power-law"):
        cur = None

spec = json.load(open("../../../agent1/spec_v1.json"))
for g in ["12", "2"]:
    gam = np.sort(np.array(spec[g]["lam"]) * np.array(spec[g]["Qe"]))[::-1]
    print(f"\ngroup {g} (spec_v1, {spec[g]['items']} items, n = {len(gam)}, sum sqrt(gamma) = {np.sqrt(gam).sum():.4f})")
    print("| code bytes | bits B | Thm 1 | Thm 2 | Thm 3 | best proved | Phi(B ln 2) | Phi/Thm 3 | Phi/best proved | W = conjectured floor = F1 | Phi/W | F2 (best k) | Thm 3 head + Phi tail (conditional) | uniform code width | escape code width | escape/Phi | escape/W | escape/F2 |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for b in BYTES:
        B = 8 * b + SCAL; H = B * math.log(2)
        t1, t2, t3 = T1(gam, H), T2(gam, H), T3(gam, H)
        bp = max(t1, t2, t3)
        ph = Phi(gam, H); cj = conj_floor(gam, H); hy = hybrid_T3_phi(gam, H)
        wu, we = widths.get(g, {}).get(8 * b, (float("nan"), float("nan")))
        f2, kf = F2_claim(gam, H)
        print(f"| {b} | {B} | {t1:.4f} | {t2:.4f} | {t3:.4f} | {bp:.4f} | {ph:.4f} | {ph / t3:.2f} | {ph / bp:.3f} | {cj:.4f} | {ph / cj:.3f} | {f2:.4f} ({kf}) | {hy:.4f} | {wu:.4f} | {we:.4f} | {we / ph:.2f} | {we / cj:.2f} | {we / f2:.2f} |")

print("\nScan over code bytes 0..96 (B = 8 bytes + 48) on every group of the three spectrum files:")
print("  file             groups  min over groups and 4..32 B of Phi/Thm3   max of Phi/Thm1 at 4 B   first byte count where Phi < Thm1 (range over groups)")
for fn in ["spec_v1.json", "spec2_v2.json", "spec2_n128.json"]:
    sp = json.load(open("../../../agent1/" + fn))
    mins, at4, cross = [], [], []
    for g in sp:
        gam = np.sort(np.array(sp[g]["lam"]) * np.array(sp[g]["Qe"]))[::-1]
        gam = gam[gam > 0]
        r3, r1 = [], []
        first = None
        for b in range(0, 97):
            H = (8 * b + SCAL) * math.log(2)
            ph = Phi(gam, H)
            t1 = T1(gam, H)
            if 4 <= b <= 32:
                r3.append(ph / T3(gam, H))
            if b == 4:
                at4.append(ph / t1)
            if first is None and ph < t1:
                first = b
        mins.append(min(r3)); cross.append(first if first is not None else 999)
    print(f"  {fn:17s} {len(sp):4d}    {min(mins):.3f}                                     {max(at4):.3f} (min {min(at4):.3f})          {min(cross)}..{max(cross)} bytes")

print("\nWhere Phi stands against Theorem 1 at high rate (one group, spec_v1 group 12):")
gam = np.sort(np.array(spec["12"]["lam"]) * np.array(spec["12"]["Qe"]))[::-1]
for b in [0, 4, 8, 12, 16, 24, 32, 48, 64, 96, 192, 384, 576]:
    H = (8 * b + SCAL) * math.log(2)
    ph, t1, t3, cj = Phi(gam, H), T1(gam, H), T3(gam, H), conj_floor(gam, H)
    print(f"  {b:4d} B: Phi/Thm1 {ph / t1:.3f}  Phi/Thm3 {ph / t3:.3f}  Phi/conj {ph / cj:.3f}  best proved/conj {max(t1, t3) / cj:.3f}  max(Phi, best proved)/conj {max(ph, t1, t3) / cj:.3f}")
