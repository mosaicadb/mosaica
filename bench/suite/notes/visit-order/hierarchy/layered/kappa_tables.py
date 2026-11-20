"""Tables of layered.md 8.8: counted bytes per query against the number of groups G, from the studies run on fits made
at G = 8, 16, 64 (results/kappa_DS_G*_{knn,exact}_SET.txt). usage: python3 kappa_tables.py results/

Bytes as in kernel_tables.py: a row entering a level its codes since the level before (+ 2 B key at the first, 0.5 B
norm code at a deterministic level, 5.5 B then 2 B of scalars at a randomised one); a stored vector 2n; a cluster
opened its tables (96 B per 4-bit unit of each level, for at most as many clusters as rows entered it); a group set
up n^2, and the rotations (4 bytes per entry of the four blocks' squares) when a randomised level is read. Left out
(the same at every G): the centres, the sigmas, the support table and the cut.
"""
import os
import re
import sys

import numpy as np

SHAPE = {"yahoo": (384, 677305, [0, 48, 112, 240, 384]), "laion": (512, 1000448, [0, 64, 144, 320, 512])}
LB = [16, 32, 56, 96, 160, 256]
NDL = 2
TARGETS = (0.9, 0.95, 0.99)


def mb(x):
    return f"{x / 1e6:.2f}" if x < 99.5e6 else f"{x / 1e6:.0f}"


def interp(pts, tg):
    pts = sorted(pts, key=lambda p: p[0])
    rec = [p[0] for p in pts]
    i = int(np.searchsorted(rec, tg - 1e-12))
    if i >= len(pts):
        return None
    if i == 0:
        return np.array(pts[0][1])
    f = (tg - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return np.array(pts[i - 1][1]) + f * (np.array(pts[i][1]) - np.array(pts[i - 1][1]))


def row_bytes(enter, opened, groups, full, n, rot, levels=None):
    """(codes, vectors, group setups, tables) for rows entering the first `levels` levels (all when None)."""
    L = len(LB) if levels is None else levels
    per = [LB[l] - (LB[l - 1] if l else 0) + (2 if l == 0 else 0) + (0.5 if l < NDL else 5.5 if l == NDL else 2) for l in range(L)]
    units = [2 * (LB[l] - (LB[l - 1] if l else 0)) for l in range(L)]
    code = sum(e * b for e, b in zip(enter, per))
    tab = sum(min(opened, e) * u * 96 for e, u in zip(enter, units))
    grp = groups * (n * n + (rot if L > NDL and len(enter) > NDL and enter[NDL] > 0 else 0))
    return code, full * 2 * n, grp, tab


def main():
    d = sys.argv[1]
    for ds, (n, N, bounds) in SHAPE.items():
        rot = 4 * sum((b - a) ** 2 for a, b in zip(bounds, bounds[1:]))
        Gs = [G for G in (8, 16, 64) if os.path.exists(f"{d}/kappa_{ds}_G{G}_exact_random-25.txt")]
        if not Gs:
            continue
        print(f"\n### {ds} (n {n}, N {N:,}): G = {Gs}; rows per basis / n^2 (kappa) = " + ", ".join(f"{N / G / n / n:.2f}" for G in Gs) + "\n")
        # (c) tightness and rho
        print("| G | certified half-width / sigma at 16, 32, 56 B (median) | rho derived, 16 B: min / median / max over groups |")
        print("|---|---|---|")
        for G in Gs:
            t = open(f"{d}/kappa_{ds}_G{G}_exact_none.txt").read()
            tight = re.search(r"tightness.*: ([\d. ]+)$", t, re.M).group(1)
            rho = re.search(r"level 0 .*min / median / max ([\d. /]+);", open(f"{d}/kappa_{ds}_G{G}_knn_none.txt").read()).group(1)
            print(f"| {G} | {tight} | {rho} |")
        # (a) approximate
        tot_a = {}
        for st in ("none", "random-25"):
            print(f"\nApproximate k-NN, k = 10, visible set {st}: to recall 0.9 / 0.95 / 0.99, per query.\n")
            print("| G | mode | clusters read | groups set up | rows entering 16 B / 32 B | stored vectors | MB: codes / vectors / group setups / tables | MB total |")
            print("|---|---|---|---|---|---|---|---|")
            for G in Gs:
                cur = {}
                for ln in open(f"{d}/kappa_{ds}_G{G}_knn_{st}.txt"):
                    m = re.match(r"psi (\S+) (\S+)\s+recall (\S+) \| (\S+) (\S+) (\S+) \| ([\d ]+) \| (\d+)", ln)
                    if not m:
                        continue
                    mode, rec = m.group(2), float(m.group(3))
                    read, opened, groups = float(m.group(4)), float(m.group(5)), float(m.group(6))
                    enter = [float(x) for x in m.group(7).split()]
                    full = float(m.group(8))
                    if mode == "statistical":
                        # the proposal: the rows surviving 32 B are scored from stored vectors; no randomised level
                        c = row_bytes(enter[:NDL], opened, groups, enter[NDL], n, rot, NDL)
                        cur.setdefault("rule 1 (two levels)", []).append((rec, [read, groups, enter[0], enter[1], enter[NDL], *c]))
                    else:
                        c = row_bytes(enter, opened, groups, full, n, rot)
                        cur.setdefault("today, " + ("direct reads" if mode == "direct" else "certified levels"), []).append((rec, [read, groups, enter[0], enter[1], full, *c]))
                for mode, pts in cur.items():
                    vs = [interp(pts, tg) for tg in TARGETS]
                    f = lambda i, fmt="{:,.0f}": " / ".join("-" if v is None else fmt.format(v[i]) for v in vs)
                    parts = " ; ".join("-" if v is None else " / ".join(mb(x) for x in v[5:9]) for v in vs)
                    tots = [None if v is None else float(sum(v[5:9])) for v in vs]
                    tot_a[(st, G, mode)] = tots
                    print(f"| {G} | {mode} | {f(0, '{:.0f}')} | {f(1, '{:.1f}')} | {f(2)} ; {f(3)} | {f(4)} | {parts} | " + " / ".join("-" if x is None else mb(x) for x in tots) + " |")
        # (b) exact
        tot_e = {}
        for st in ("none", "random-25"):
            print(f"\nExact queries, visible set {st}: per query.\n")
            print("| G | query | rows entering each level | stored vectors | clusters opened / taken in / dropped by the support table | groups set up | MB: codes / vectors / group setups / tables | MB total |")
            print("|---|---|---|---|---|---|---|---|")
            for G in Gs:
                for ln in open(f"{d}/kappa_{ds}_G{G}_exact_{st}.txt"):
                    p = [x.strip() for x in ln.split("|")]
                    if len(p) < 8 or not (p[0].startswith("knn") or p[0].startswith("threshold")):
                        continue
                    enter = [float(x) for x in p[1].split()]
                    full = float(p[2])
                    opened, cin, cout = (float(x) for x in p[3].split())
                    groups = float(p[5])
                    c = row_bytes(enter, opened, groups, full, n, rot)
                    tot_e[(st, G, p[0])] = sum(c)
                    print(f"| {G} | {p[0]} | {' / '.join(f'{e:,.0f}' for e in enter)} | {full:,.0f} | {opened:.0f} / {cin:.0f} / {cout:.0f} | {groups:.1f} | {' / '.join(mb(x) for x in c)} | {mb(sum(c))} |")
        # totals against G
        print(f"\nTotal counted MB per query against G ({ds}); the smallest of each row in bold.\n")
        print("| query | visible set | " + " | ".join(f"G = {G}" for G in Gs) + " |")
        print("|---|---|" + "---|" * len(Gs))

        def line(name, st, vals):
            ok = [v for v in vals if v is not None]
            cells = ["-" if v is None else (f"**{mb(v)}**" if v == min(ok) else mb(v)) for v in vals]
            print(f"| {name} | {st} | " + " | ".join(cells) + " |")

        for st in ("none", "random-25"):
            for mode in sorted({k[2] for k in tot_a if k[0] == st}):
                for ti, tg in enumerate(TARGETS):
                    line(f"approximate, {mode}, recall {tg}", st, [tot_a[(st, G, mode)][ti] for G in Gs])
            for q in sorted({k[2] for k in tot_e if k[0] == st}):
                line(f"exact, {q}", st, [tot_e[(st, G, q)] for G in Gs])


if __name__ == "__main__":
    main()
