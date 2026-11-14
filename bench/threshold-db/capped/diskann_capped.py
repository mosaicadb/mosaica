# DiskANN (diskannpy 0.7.0, its own venv) under a memory cap: a static disk index, PQ codes in RAM, the graph and the
# full f32 vectors on disk, inner product (mips; the vectors are unit). Top-m, as hnswlib in ../ann.py: m = the exact
# answer size, search list L = m x mult, beam width W.
# usage: python3 -I diskann_capped.py build PREP_DIR DIR [--ram-gb 0.05] [--threads 2]
#        python3 -I diskann_capped.py query PREP_DIR DIR OUT_CSV [--vis all] [--shares 0,1,2] [--mults 1,2,...]
#               [--beam 2] [--step 10] [--reps 5] [--cold] [--cap-ms 5000] [--ladder]
# --ladder walks mult up the list until mean recall reaches 0.99 or a median query exceeds --cap-ms, untimed;
# otherwise each mult in --mults is timed.
import argparse, os, resource, sys, time
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("cmd", choices=["build", "query"])
ap.add_argument("prep")
ap.add_argument("dir")
ap.add_argument("out", nargs="?")
ap.add_argument("--ram-gb", type=float, default=0.05)
ap.add_argument("--threads", type=int, default=2)
ap.add_argument("--vis", default="all")
ap.add_argument("--shares", default="0,1,2")
ap.add_argument("--mults", default="1")
ap.add_argument("--beam", type=int, default=2)
ap.add_argument("--step", type=int, default=10)
ap.add_argument("--reps", type=int, default=5)
ap.add_argument("--cold", action="store_true")
ap.add_argument("--ladder", action="store_true")
ap.add_argument("--cap-ms", type=float, default=5000)
a = ap.parse_args()
import diskannpy as dap

N, n = 999950, 768
os.makedirs(a.dir, exist_ok=True)


def cgroup():
    return "/sys/fs/cgroup" + open("/proc/self/cgroup").read().strip().split("::", 1)[1]


def cg_report():
    d = cgroup()
    st = dict(l.split() for l in open(f"{d}/memory.stat"))
    return (f"cgroup current {int(open(f'{d}/memory.current').read()) / 2**20:.1f} MiB, anon {int(st['anon']) / 2**20:.1f}, "
            f"file {int(st['file']) / 2**20:.1f}, max {open(f'{d}/memory.max').read().strip()}, "
            f"events {' '.join(open(f'{d}/memory.events').read().split())}")


def reclaim():
    try:
        with open(f"{cgroup()}/memory.reclaim", "w") as f:
            f.write(str(16 << 30))
    except OSError:
        pass


if a.cmd == "build":
    binf = f"{a.dir}/corpus32.bin"
    if not os.path.exists(binf):
        X = np.load(f"{a.prep}/corpus32.npy", mmap_mode="r")
        with open(binf, "wb") as f:
            f.write(np.array([X.shape[0], X.shape[1]], "<i4").tobytes())
            for i in range(0, X.shape[0], 100_000):
                f.write(np.ascontiguousarray(X[i:i + 100_000], dtype="<f4").tobytes())
    t = time.time()
    dap.build_disk_index(binf, "mips", a.dir, complexity=100, graph_degree=64, search_memory_maximum=a.ram_gb,
                         build_memory_maximum=12, num_threads=a.threads, vector_dtype=np.float32)
    print(f"diskann: built in {time.time() - t:.0f} s on {a.threads} threads; peak RSS "
          f"{resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2**20:.2f} GiB", flush=True)
    for f in sorted(os.listdir(a.dir)):
        print(f, os.path.getsize(f"{a.dir}/{f}"))
    sys.exit()

Q = np.load(f"{a.prep}/queries.npy").astype(np.float32)
ks = [round(s * N) for s in (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)]
shares = [int(x) for x in a.shares.split(",")]
qis = list(range(0, len(Q), a.step))
order = np.load(f"{a.prep}/order.npy", mmap_mode="r")
exact = {(si, qi): np.array(order[qi, :ks[si]]) for si in shares for qi in qis}
del order
t = time.time()
ix = dap.StaticDiskIndex(a.dir, num_threads=1, num_nodes_to_cache=0, distance_metric="mips", vector_dtype=np.float32,
                         dimensions=n)
reclaim()
print(f"diskann: opened in {time.time() - t:.1f} s; {cg_report()}", flush=True)


def one(qi, m, L):
    return ix.search(Q[qi], k_neighbors=m, complexity=L, beam_width=a.beam).identifiers


def evaluate(si, mult, reps):
    res = []
    for qi in qis:
        e = exact[si, qi]
        m = len(e)
        L = int(np.ceil(m * mult))
        ts = []
        for _ in range(reps):
            t0 = time.perf_counter()
            got = one(qi, m, L)
            ts.append((time.perf_counter() - t0) * 1e3)
        res.append((qi, L, float(np.median(ts)), min(ts), len(np.intersect1d(got, e)) / m))
    return res


rows = []
for si in shares:
    if a.ladder:
        for mult in [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32]:
            r = evaluate(si, mult, 1)
            rec, ms = np.mean([x[4] for x in r]), np.median([x[2] for x in r])
            print(f"diskann ladder share {si} beam {a.beam}: L = {mult} m, mean recall {rec:.4f}, median {ms:.1f} ms", flush=True)
            if rec >= 0.99 or ms > a.cap_ms:
                break
        continue
    for mult in [float(x) for x in a.mults.split(",")]:
        if a.cold:
            reclaim()
            t0 = time.perf_counter()
            e = exact[si, qis[0]]
            got = one(qis[0], len(e), int(np.ceil(len(e) * mult)))
            ms = (time.perf_counter() - t0) * 1e3
            rows.append(f"diskann,x{mult:g},{a.vis},{si},{qis[0]},{ms:.4f},{ms:.4f},{len(np.intersect1d(got, e)) / len(e):.5f},cold")
        r = evaluate(si, mult, a.reps)
        for qi, L, ms, mn, rec in r:
            rows.append(f"diskann,x{mult:g},{a.vis},{si},{qi},{ms:.4f},{mn:.4f},{rec:.5f},warm")
        print(f"diskann share {si} L = {mult:g} m, beam {a.beam}: median {np.median([x[2] for x in r]):.2f} ms, "
              f"mean recall {np.mean([x[4] for x in r]):.4f}; {cg_report()}", flush=True)
if rows:
    new = not os.path.exists(a.out)
    with open(a.out, "a") as f:
        f.write(("what,setting,vis,share_index,query,median_ms,min_ms,recall,kind\n" if new else "") + "\n".join(rows) + "\n")
