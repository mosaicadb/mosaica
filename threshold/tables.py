# Tables for REPORT.md from the three runs: local-basis stream (PCA and geometric-mean metric; lb_run.py outputs) and
# the flat global-rotation code (flat.py output). Restricted to the first NQ queries, which all three share.
# usage: tables.py WORKDIR [NQ]
import sys, glob, json, numpy as np
W = sys.argv[1]; NQ = int(sys.argv[2]) if len(sys.argv) > 2 else 100
SH = ["0.01%", "0.1%", "1%", "10%", "30%"]; DS = [0, 0.002, 0.005, 0.01]; n = 768; FULL = 2 * n
I = np.load(W + "/index.npz"); grp, cs = I["grp"], I["cstart"]; gsz = np.bincount(grp, weights=np.diff(cs), minlength=int(I["G"]))
def local(tag, track):
    D = [dict(np.load(f)) for f in sorted(glob.glob(f"{W}/out_bio{tag}_w*.npz"))]
    E = sum(d[f"{track}|enter"] for d in D)[:, :, :NQ]; IB = sum(d[f"{track}|ib"] for d in D)[:, :NQ]
    CB = sum(d[f"{track}|cb"] for d in D)[:, :NQ]; A = sum(d["A"] for d in D)[:, :NQ]; V = sum(d[f"{track}|viol"] for d in D)
    N = int(I["N"]); LV = list(D[0]["LV"]); LD = int(D[0]["LD"]); RLV = list(D[0]["RLV"])
    # stored cumulative bytes per level, scalars included, averaged over items by group
    sc = np.zeros(len(RLV)); gs = []
    for d in D: gs += list(d["groups"])
    sca = json.loads(str(D[0]["SCAL"]))["sub"]
    allsc = []; 
    for d in D: allsc += json.loads(str(d["SCAL"]))["sub"]
    sc = sum(np.array(s) * gsz[g] for s, g in zip(allsc, gs)) / gsz[gs].sum()
    inc = [3] + [LV[l] - LV[l - 1] + 1 for l in range(1, LV.index(LD) + 1)] + [RLV[i] - (RLV[i - 1] if i else LD) + sc[i] for i in range(len(RLV))]
    cum = np.cumsum(inc); names = [str(b) for b in LV[:LV.index(LD) + 1] + RLV]
    return dict(N=N, E=E, IB=IB, CB=CB, A=A, V=V, cum=cum, names=names)
def flat():
    F = np.load(W + "/flat_out.npz"); LV = int(F["LV"]); BYT = F["BYT"]; N = int(F["N"]); assert int(F["NQ"]) == NQ
    KS = (15, 150, 1500, 15000, 45000); out = {}
    for si, k in enumerate(KS):
        for di, d in enumerate(DS):
            v = F[f"{k}|allN" + ("" if d == 0 else f" d={d}")] / NQ; und = v[2 * LV:3 * LV]; rd = v[3 * LV] + v[3 * LV + 1]
            code = N * BYT[0] + sum(und[l] * BYT[l + 1] for l in range(LV - 1))
            out[si, di] = dict(und=und / v[-1], reads=rd / v[-1], code=code / N, total=(code + rd * FULL) / N, viol=F[f"{k}|allN" + ("" if d == 0 else f" d={d}")][3 * LV + 2:3 * LV + 4].sum())
    return out, np.cumsum(BYT)
def lrow(L, si, di):
    ti = di * 5 + si; m = L["A"][ti].sum(); E = L["E"][:, ti].sum(1)
    reads = E[-1]; code = (L["IB"][ti].sum() - FULL * reads) / NQ / L["N"]
    return dict(und=E[1:] / m, reads=reads / m, code=code, total=code + FULL * reads / NQ / L["N"], cb=L["CB"][ti].sum() / NQ / L["N"], viol=int(L["V"][ti]))
Fl, fcum = flat()
out = []
P = lambda *a: out.append(" ".join(str(x) for x in a))
P(f"## Undecided per true match after each level, d = 0 ({NQ} held-out queries)\n")
for nm, tag, tr in (("Local basis, PCA (V3sub)", "", "V3sub"), ("Local basis, PCA, with the annulus bound from the stored a (V3sub+C1)", "", "V3sub+C1"),
                    ("Local basis, geometric-mean metric (V3sub)", "gm", "V3sub"), ("Local basis, geometric-mean metric, with annulus (V3sub+C1)", "gm", "V3sub+C1")):
    L = local(tag, tr)
    P(f"\n### {nm}\n\nLevel (code B, stored B incl. scalars): " + ", ".join(f"{a} ({c:.0f})" for a, c in zip(L["names"], L["cum"])) + "\n")
    P("| Share | " + " | ".join(L["names"]) + " | exact reads/match | code B/item | total B/item | violations |"); P("|---" * (len(L["names"]) + 5) + "|")
    for si in range(5):
        r = lrow(L, si, 0); P(f"| {SH[si]} | " + " | ".join(f"{x:.3g}" for x in r["und"]) + f" | {r['reads']:.3f} | {r['code']:.2f} | {r['total']:.2f} | {r['viol']} |")
    P("\nTolerance d: exact reads per match / code B per item / total B per item\n")
    P("| Share | d = 0 | 0.002 | 0.005 | 0.01 |"); P("|---|---|---|---|---|")
    for si in range(5):
        P(f"| {SH[si]} | " + " | ".join(f"{lrow(L, si, di)['reads']:.3f} / {lrow(L, si, di)['code']:.1f} / {lrow(L, si, di)['total']:.1f}" for di in range(4)) + " |")
    P("\nViolations over all shares and tolerances: " + str(int(L["V"].sum())) + "; per-cluster bytes (centres, searches) per item at d = 0: " + ", ".join(f"{lrow(L, si, 0)['cb']:.3f}" for si in range(5)))
P(f"\n### Flat global rotation (levels s256, s512, 1, 2, 4, 6 bits; code B {', '.join(str(int(x)) for x in fcum)}; no scalars charged)\n")
P("| Share | " + " | ".join(str(int(x)) for x in fcum[:-1]) + " | exact reads/match | code B/item | total B/item | violations |"); P("|---" * (len(fcum) + 4) + "|")
for si in range(5):
    r = Fl[si, 0]; P(f"| {SH[si]} | " + " | ".join(f"{x:.3g}" for x in r["und"][:-1]) + f" | {r['reads']:.3f} | {r['code']:.2f} | {r['total']:.2f} | {int(r['viol'])} |")
P("\nTolerance d: exact reads per match / code B per item / total B per item\n")
P("| Share | d = 0 | 0.002 | 0.005 | 0.01 |"); P("|---|---|---|---|---|")
for si in range(5):
    P(f"| {SH[si]} | " + " | ".join(f"{Fl[si, di]['reads']:.3f} / {Fl[si, di]['code']:.1f} / {Fl[si, di]['total']:.1f}" for di in range(4)) + " |")
P("\nViolations over all shares and tolerances: " + str(int(sum(Fl[k]['viol'] for k in Fl))))
print("\n".join(out))
