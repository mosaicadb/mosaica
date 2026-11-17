# The shipped certificate table (src/certs_table.json): the randomised levels' constant X for the fit's layout of each
# common embedding dimension, at the budget of every index size N = 2^10 .. 2^34, so that a fit of such vectors
# records its certificate without certifying (src/fit.rs; tools/fit.py reads the table too).
#
# The layouts and budgets come from the Rust binary (`vexact cert-layouts`), so the layout rule lives only in
# src/fit.rs; every entry is certified twice, by certify.py and by the Rust port (`vexact certify-layout`), and the
# table keeps an entry only where both give the same X (or both none: X null, a layout whose certificate the search
# does not find, which the fit then does not look for). Any disagreement stops the run, naming the entry.
#
# Results are cached in OUT.progress.jsonl as they finish, so an interrupted run continues where it stopped.
#
# usage: python3 -I certs_table.py VEXACT_BINARY [--dims 64,96,..] [--every-other-n DIM,DIM,..] [--jobs 3]
#        [--time-cap 100000] [--out src/certs_table.json]
import argparse, json, math, os, subprocess, sys, time
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("OMP_NUM_THREADS", "1")
import certify  # noqa: E402

DIMS = [64, 96, 128, 192, 256, 384, 512, 768, 1024, 1152, 1536, 2048, 3072, 4096]
EPS = 1e-9


def py_x(ds, lnd, cap):
    t = time.time()
    return certify.certify_layout(ds, lnd, cap, log=lambda s: None), time.time() - t


def rust_x(vexact, ds, lnd, cap, threads):
    t = time.time()
    env = dict(os.environ, RAYON_NUM_THREADS=str(threads))
    out = subprocess.run([vexact, "certify-layout", "--ds", ",".join(map(str, ds)), "--lnd", repr(lnd), "--time-cap", str(cap)],
                         check=True, capture_output=True, text=True, env=env).stdout
    r = json.loads(out)
    assert r["ds"] == ds and r["lnd"] == lnd, (r, ds, lnd)
    return r["X"], time.time() - t


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vexact")
    ap.add_argument("--dims", default=",".join(map(str, DIMS)))
    ap.add_argument("--every", default="", help="DIM:STEP,..: only every STEP-th power of two of N for these dimensions")
    ap.add_argument("--jobs", type=int, default=3, help="certify.py processes at once")
    ap.add_argument("--rust-threads", type=int, default=1)
    ap.add_argument("--time-cap", type=float, default=100000)
    ap.add_argument("--out", default=os.path.join(HERE, "..", "src", "certs_table.json"))
    a = ap.parse_args()
    every = {int(k): int(v) for k, v in (e.split(":") for e in a.every.split(",") if e)}
    out = subprocess.run([a.vexact, "cert-layouts", "--dims", a.dims], check=True, capture_output=True, text=True).stdout
    todo = {}
    for lay in map(json.loads, out.splitlines()):
        if "error" in lay:
            print(f"n0 {lay['n0']}: skipped ({lay['error']})", flush=True)
            continue
        for p, lnd in lay["lnd"]:
            if (p - 10) % every.get(lay["n0"], 1):
                continue
            # the budget as certify.py computes it for an index of 2^p items
            assert lnd == -math.log(2 ** p * lay["levels"] / EPS), (lay, p, lnd)
            todo.setdefault((tuple(lay["ds"]), lnd), []).append((lay["n0"], p))
    prog = a.out + ".progress.jsonl"
    done = {}
    if os.path.exists(prog):
        for r in map(json.loads, open(prog)):
            done[(tuple(r["ds"]), r["lnd"])] = r
    keys = [k for k in sorted(todo, key=lambda k: (sum(k[0]), k[0], -k[1])) if k not in done]
    print(f"{len(todo)} entries, {len(todo) - len(keys)} done before; {len(keys)} to certify", flush=True)
    t0 = time.time()
    with ProcessPoolExecutor(a.jobs) as pp, ThreadPoolExecutor(1) as rp, open(prog, "a") as pf:
        fut = [(k, pp.submit(py_x, list(k[0]), k[1], a.time_cap), rp.submit(rust_x, a.vexact, list(k[0]), k[1], a.time_cap, a.rust_threads)) for k in keys]
        for i, (k, fp, fr) in enumerate(fut):
            (xp, tp), (xr, tr) = fp.result(), fr.result()
            ds, lnd = list(k[0]), k[1]
            print(f"[{time.time() - t0:7.0f}s] {i + 1}/{len(keys)} {ds} ln delta {lnd:.4f} (n0, log2 N {todo[k]}): certify.py {xp} in {tp:.0f}s, Rust {xr} in {tr:.0f}s", flush=True)
            if xp != xr:
                for _, f2, g2 in fut:
                    f2.cancel(); g2.cancel()
                sys.exit(f"DISAGREEMENT at {ds} ln delta {lnd!r}: certify.py {xp}, Rust {xr}; the table is not written")
            pf.write(json.dumps(dict(ds=ds, lnd=lnd, X=xp, py_s=round(tp, 1), rust_s=round(tr, 1))) + "\n")
            pf.flush()
            done[k] = dict(ds=ds, lnd=lnd, X=xp)
    table = [dict(ds=list(k[0]), lnd=k[1], X=done[k]["X"]) for k in sorted(todo, key=lambda k: (sum(k[0]), k[0], -k[1]))]
    with open(a.out + ".tmp", "w") as f:
        f.write("[\n" + ",\n".join(json.dumps(e) for e in table) + "\n]\n")
    os.replace(a.out + ".tmp", a.out)
    print(f"wrote {a.out}: {len(table)} entries ({sum(e['X'] is None for e in table)} without a certificate) in {time.time() - t0:.0f}s")


if __name__ == "__main__":
    main()
