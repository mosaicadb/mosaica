# The fixture of src/certify.rs's tests (testdata/certify_fixture.json): scipy's values of the special functions the
# port implements (gammaln, betainc, betaincinv, certify.py's sphere_constant) and certify.py's X on a grid of layouts
# (the fit's four-block layouts of several dimensions, two- and three-block subsets of them, each at the budgets of
# N = 2^10, 2^20 and 2^30). The layouts and budgets come from the Rust binary (`vexact cert-layouts`), so the layout
# rule lives only there. The tests then compare the port with these values without Python.
#
# usage: python3 -I certify_fixture.py VEXACT_BINARY [OUT.json]
import json, math, os, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import certify  # noqa: E402
from scipy.special import betainc, betaincinv, gammaln  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))


def layouts(vexact, dims):
    out = subprocess.run([vexact, "cert-layouts", "--dims", ",".join(map(str, dims))], check=True, capture_output=True, text=True).stdout
    return {d["n0"]: d for d in map(json.loads, out.splitlines())}


def main():
    vexact = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "..", "testdata", "certify_fixture.json")
    lg = [k / 2 for k in range(1, 60)] + [k / 2 for k in range(61, 9000, 37)] + [0.1, 0.3, 0.7, 1.3, 7.77, 12.99, 13.01, 999.5, 1000.5, 4095.5, 1e6 + 0.5, 3e8]
    fx = dict(lgamma=[[x, float(gammaln(x))] for x in lg])
    bi, bv = [], []
    for a in [1.5, 7.0, 47.0, 95.5, 383.0, 767.5, 2047.0]:
        for x in [0.01, 0.1, 0.5, 0.8, 0.9, 0.95, 0.99, 0.999]:
            bi.append([a, 0.5, x, float(betainc(a, 0.5, x))])
        for y in [1e-22, 1e-18, 1e-15, 1e-10, 1e-5, 0.01, 0.3, 0.7]:
            bv.append([a, 0.5, y, float(betaincinv(a, 0.5, y))])
    for a, b, x in [(2.0, 3.0, 0.4), (10.0, 10.0, 0.5), (0.5, 0.5, 0.2), (30.0, 2.5, 0.97)]:
        bi.append([a, b, x, float(betainc(a, b, x))])
    fx["betainc"], fx["betaincinv"] = bi, bv
    fx["sphere"] = [[D, lnd, certify.sphere_constant(D, lnd)] for D in [62, 98, 382, 576, 770, 1538, 4098] for lnd in [-27.65, -35.97, -45.68]]
    L = layouts(vexact, [64, 128, 384, 768, 1536])
    grid = []
    for n0, pick in [(64, None), (384, None), (768, None), (1536, None), (768, slice(0, 3)), (384, slice(1, 4)), (768, slice(0, 2)), (128, slice(2, 4)), (1536, slice(0, 3))]:
        ds = L[n0]["ds"] if pick is None else L[n0]["ds"][pick]
        for p, lnd in L[n0]["lnd"]:
            if p in (10, 20, 30):
                t = time.time()
                X = certify.certify_layout(ds, lnd, 1e6, log=lambda s: print(s, file=sys.stderr))
                grid.append(dict(n0=n0, ds=ds, lnd=lnd, X=X, seconds=round(time.time() - t, 1)))
    fx["certs"] = grid
    with open(out, "w") as f:
        json.dump(fx, f, indent=1)
    print(f"wrote {out}: {len(grid)} certificates")


if __name__ == "__main__":
    main()
