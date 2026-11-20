"""Tables of cascade.py's results (markdown). usage: python3 cascade_tables.py results/cascade_*.json"""
import json
import sys

import numpy as np

SHOW = (0.9, 0.95, 0.99, 0.995)


def interp(points, tg):
    """points: [recall, values...] rows; the values at recall tg along the curve sorted by recall (None if not reached)."""
    p = sorted(points, key=lambda x: x[0])
    rec = np.array([x[0] for x in p])
    i = np.searchsorted(rec, tg - 1e-12)
    if i >= len(p):
        return None
    if i == 0:
        return p[0][1:]
    f = (tg - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return [a + f * (b - a) for a, b in zip(p[i - 1][1:], p[i][1:])]


def mb(x):
    return f"{x / 1e6:.2f}" if x < 99.5e6 else f"{x / 1e6:.0f}"


def main():
    for path in sys.argv[1:]:
        R = json.load(open(path))
        col = {c: i for i, c in enumerate(R["columns"])}
        T = R["targets"]
        ks = sorted({int(n.split()[0][2:]) for n in R["runs"]})
        for k in ks:
            runs = {n[len(f"k={k} "):]: e for n, e in R["runs"].items() if n.startswith(f"k={k} ")}
            print(f"\n### {R['dataset']}, visible set {R['set']}, k = {k} ({R['nq']} queries; K {R['K']}, n {R['n']}; the centre scan is {R['fixed_centre_bytes'] / 1e6:.2f} MB of every total)\n")
            print("Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.\n")
            print("| rule, stop | " + " | ".join(f"recall {t}" for t in SHOW) + " |")
            print("|---|" + "---|" * len(SHOW))

            def cell(v):
                if v is None:
                    return "-"
                return f"{mb(v[col['bytes']])} ({v[col['clusters']]:.0f}; {max(v[col[c]] for c in col if c.startswith('enter_l')):,.0f}; {v[col['full_rows']]:,.0f})"

            def row(name, getter):
                print(f"| {name} | " + " | ".join(cell(getter(T.index(t), t)) for t in SHOW) + " |")

            row("direct, same clusters for every query", lambda i, t: runs["direct"]["nprobe"][i])
            row("direct, the library's stop", lambda i, t: runs["direct"]["stop"][i])
            row("certified levels, the library's stop", lambda i, t: runs["cert"]["stop"][i])
            def fam(tau, suf):
                return [e for n, e in runs.items() if n.startswith(f"stat-{tau} ") and n.split(" L", 1)[1] == suf]

            for tau, lab in (("d", "h / sqrt(d)"), ("m", "rho measured")):
                row(f"statistical levels (16, 32 B), tau = {lab}, one dial", lambda i, t, tau=tau: interp([e["one_dial"] for e in fam(tau, "16+32")], t))

                def best(i, t, tau=tau):
                    c = [e["stop"][i] for e in fam(tau, "16+32") if e["stop"][i] is not None]
                    return min(c, key=lambda v: v[col["bytes"]]) if c else None

                row(f"statistical levels, tau = {lab}, best row psi per recall", best)
            sufs = sorted({n.split(" L", 1)[1] for n in runs if n.startswith("stat-m ")} - {"16+32"}, key=lambda x: (len(x), x))
            for suf in sufs:
                lab = ("levels " + suf.replace(" key", "").replace("+", ", ") + " B") + (", the key's range statistical too" if suf.endswith("key") else "")
                row(f"statistical (rho measured), one dial, {lab}", lambda i, t, suf=suf: interp([e["one_dial"] for e in fam("m", suf)], t))
            print("\nParts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:\n")
            print("| rule | 0.95 | 0.99 |")
            print("|---|---|---|")

            def parts(v):
                return "-" if v is None else " / ".join(mb(v[col[c]]) for c in ("code", "full", "group", "table", "sigma"))

            one = [e["one_dial"] for n, e in runs.items() if n.startswith("stat-m ") and n.endswith(" L16+32")]
            for name, g in (("direct", lambda t: runs["direct"]["stop"][T.index(t)]), ("certified", lambda t: runs["cert"]["stop"][T.index(t)]), ("statistical, one dial", lambda t: interp(one, t))):
                print(f"| {name} | {parts(g(0.95))} | {parts(g(0.99))} |")
            pts = sorted((float(n.split("psi=")[1].split()[0]), e["one_dial"]) for n, e in runs.items() if n.startswith("stat-m ") and n.endswith(" L16+32"))
            print(f"\nrho measured per level {[round(x, 4) for x in R['rho_measured']]}; 1 / sqrt(d) = {R['d'] ** -0.5:.4f}")
            print("\n| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |")
            print("|---|---|---|---|---|---|---|---|---|")
            for psi, v in pts:
                w = v[1:]
                print(f"| {psi:g} | {v[0]:.4f} | {mb(w[col['bytes']])} | {w[col['clusters']]:.1f} | {w[col['rows']]:,.0f} | {w[col['in_range']]:,.0f} | {w[col[f"enter_l{R['base'][0]}"]]:,.0f} | {w[col[f"enter_l{R['base'][1]}"]]:,.0f} | {w[col['full_rows']]:,.0f} |")
            pts = runs["direct"]["stop_points"]
            print("\n| psi (stop), direct reads | recall | MB | clusters | rows |")
            print("|---|---|---|---|---|")
            for p in pts:
                w = p[2:]
                print(f"| {p[0]:g} | {p[1]:.4f} | {mb(w[col['bytes']])} | {w[col['clusters']]:.1f} | {w[col['rows']]:,.0f} |")


if __name__ == "__main__":
    main()
