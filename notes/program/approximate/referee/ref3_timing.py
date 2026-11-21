"""Referee 3: time to recall 0.90 / 0.95 / 0.98 in the reader, from the timing session's files (log-linear in time
between the two psi that bracket the recall; unfiltered rows only). Each build is given as a ratio to the derived-width
build 1fc22a5 timed in the same run, so two runs are compared only through that common reference.
usage: python3 ref3_timing.py"""
import re

V = "/home/joe/code/vexact-sampled/bench/suite/notes/visit-order/results"
DS = ["yahoo-minilm-384-normalized", "laion-clip-512-normalized", "inaturalist-resnet-2048-cosine"]
pat = re.compile(r"^none\s+adaptive\s+psi=([\d.]+): ([\d.]+)\s+\d+ rows\s+([\d.]+) us")


def curve(path):
    pts = []
    for line in open(path):
        m = pat.match(line)
        if m:
            pts.append((float(m.group(2)), float(m.group(3)), float(m.group(1))))
    return pts


def at(pts, tg):
    import math
    for (r0, t0, _), (r1, t1, _) in zip(pts, pts[1:]):
        if r0 <= tg <= r1 and r1 > r0:
            return math.exp(math.log(t0) + (math.log(t1) - math.log(t0)) * (tg - r0) / (r1 - r0))
    return None


for ds in DS:
    runs = {
        "as built (7ac09e9) / derived": (f"{V}/sampled-width/ab-knn-{ds}-7ac09e9.txt", f"{V}/sampled-width/ab-knn-{ds}-1fc22a5.txt"),
        "level psi 2 (0a4a009) / derived": (f"{V}/level-psi/ab-knn-{ds}-0a4a009-lp2.txt", f"{V}/level-psi/ab-knn-{ds}-1fc22a5.txt"),
        "level psi 3 (0a4a009) / derived": (f"{V}/level-psi/ab-knn-{ds}-0a4a009-lp3.txt", f"{V}/level-psi/ab-knn-{ds}-1fc22a5.txt"),
    }
    print(f"## {ds}")
    for name, (a, b) in runs.items():
        pa, pb = curve(a), curve(b)
        cells = []
        for tg in (0.90, 0.95, 0.98):
            x, y = at(pa, tg), at(pb, tg)
            cells.append("-" if x is None or y is None else f"{x / y:.2f} ({x:.0f} / {y:.0f} us)")
        print(f"  {name}: recall 0.90 / 0.95 / 0.98: " + "  ".join(cells) + f"; recall at psi 9: {pa[-1][0]:.3f}")
