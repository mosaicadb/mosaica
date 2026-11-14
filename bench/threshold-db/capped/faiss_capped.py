# faiss under a memory cap, in half precision (sqflat, sqivf) or f32 (flat, ivf): a flat range search from a
# memory-mapped index file (IndexScalarQuantizer QT_fp16 or IndexFlatIP, IO_FLAG_MMAP_IFC), and IVF4096 range search
# with its inverted lists on disk (IndexIVFScalarQuantizer QT_fp16 or IndexIVFFlat, OnDiskInvertedLists, mapped).
# usage: python3 -I faiss_capped.py build sqflat|sqivf|flat|ivf PREP_DIR ANN_DIR [--threads 2]
#        python3 -I faiss_capped.py query sqflat|sqivf|flat|ivf PREP_DIR ANN_DIR OUT_CSV [--vis all|vis_1] [--shares 0,1,...]
#               [--nprobe P] [--step 10] [--reps 5] [--cold]
# The queries, thresholds and timing are those of ../ann.py: one query at a time, 1 thread, from the call to the numpy
# ids; the per-query median of --reps runs. --cold first runs query 0 once per share after this cgroup's page cache is
# reclaimed (memory.reclaim; the cap's scope has no swap, so only file pages go).
import argparse, os, resource, sys, time
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("cmd", choices=["build", "query"])
ap.add_argument("method", choices=["sqflat", "sqivf", "flat", "ivf"])
ap.add_argument("prep")
ap.add_argument("ann")
ap.add_argument("out", nargs="?")
ap.add_argument("--threads", type=int, default=1)
ap.add_argument("--vis", default="all")
ap.add_argument("--shares", default="")
ap.add_argument("--nprobe", type=int, default=1)
ap.add_argument("--step", type=int, default=10)
ap.add_argument("--reps", type=int, default=5)
ap.add_argument("--cold", action="store_true")
a = ap.parse_args()
import faiss

X = np.load(f"{a.prep}/corpus32.npy", mmap_mode="r")
N, n = X.shape


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
        pass  # EAGAIN once nothing more can be reclaimed


def read_bytes():
    for l in open("/proc/self/io"):
        if l.startswith("read_bytes:"):
            return int(l.split()[1])


if a.cmd == "build":
    faiss.omp_set_num_threads(a.threads)
    t = time.time()
    if a.method in ("flat", "sqflat"):
        ix = faiss.IndexFlatIP(n) if a.method == "flat" else faiss.IndexScalarQuantizer(n, faiss.ScalarQuantizer.QT_fp16, faiss.METRIC_INNER_PRODUCT)
        for i in range(0, N, 100_000):
            ix.add(np.ascontiguousarray(X[i:i + 100_000]))
        faiss.write_index(ix, f"{a.ann}/{a.method}.bin")
    else:
        # IVF4096 trained as ../ann.py trains it, then the lists moved to an OnDiskInvertedLists file
        q = faiss.IndexFlatIP(n)
        ix = faiss.IndexIVFFlat(q, n, 4096, faiss.METRIC_INNER_PRODUCT) if a.method == "ivf" else \
            faiss.IndexIVFScalarQuantizer(q, n, 4096, faiss.ScalarQuantizer.QT_fp16, faiss.METRIC_INNER_PRODUCT)
        rng = np.random.default_rng(1)
        ix.train(np.ascontiguousarray(X[np.sort(rng.choice(N, 200_000, replace=False))]))
        for i in range(0, N, 100_000):
            ix.add(np.ascontiguousarray(X[i:i + 100_000]))
        od = faiss.OnDiskInvertedLists(ix.nlist, ix.code_size, f"{a.ann}/{a.method}_ondisk.ivfdata")
        v = faiss.InvertedListsPtrVector()
        v.push_back(ix.invlists)
        od.merge_from_multiple(v.data(), v.size(), False)
        ix.replace_invlists(od, True)
        od.this.disown()
        faiss.write_index(ix, f"{a.ann}/{a.method}_ondisk.bin")
    print(f"{a.method}: built in {time.time() - t:.0f} s; peak RSS {resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2**20:.2f} GiB")
    sys.exit()

Q = np.load(f"{a.prep}/queries.npy").astype(np.float32)
T = np.load(f"{a.prep}/thresholds.npy")
ks = [round(s * N) for s in (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)]
FLAT = a.method in ("flat", "sqflat")
shares = [int(x) for x in a.shares.split(",")] if a.shares else list(range(7 if FLAT else 3))
qis = list(range(0, len(Q), a.step))
faiss.omp_set_num_threads(a.threads)
vis = a.vis
mask = None
if vis != "all":
    ids = np.load(f"{a.prep}/{vis}.npy")
    mask = np.zeros(N, bool)
    mask[ids] = True
# the exact answers, read before timing so that order.npy's pages are not in the cache while timing
order = np.load(f"{a.prep}/order.npy", mmap_mode="r")
exact = {}
for si in shares:
    for qi in qis:
        e = np.array(order[qi, :ks[si]])
        exact[si, qi] = e[mask[e]] if mask is not None else e
del order
t = time.time()
if FLAT:
    ix = faiss.read_index(f"{a.ann}/{a.method}.bin", faiss.IO_FLAG_MMAP_IFC)
else:
    # OnDiskInvertedLists maps its data file itself; IO_FLAG_MMAP on top crashed the first search (faiss 1.16.0)
    ix = faiss.read_index(f"{a.ann}/{a.method}_ondisk.bin")
sel = None
if mask is not None:
    bits = np.packbits(mask, bitorder="little")
    sel = faiss.IDSelectorBitmap(N, faiss.swig_ptr(bits))
# the files may sit in the page cache charged to another cgroup (the build's): drop them first
for f in os.listdir(a.ann):
    if f.startswith(a.method):
        fd = os.open(f"{a.ann}/{f}", os.O_RDONLY)
        os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        os.close(fd)
reclaim()
print(f"{a.method}: opened in {time.time() - t:.1f} s; {cg_report()}", flush=True)


def one(qi, si):
    q = Q[qi:qi + 1]
    if FLAT:
        ps = faiss.SearchParameters(sel=sel) if sel is not None else None
    else:
        ps = faiss.SearchParametersIVF(nprobe=a.nprobe, sel=sel) if sel is not None else faiss.SearchParametersIVF(nprobe=a.nprobe)
    _, D, I = ix.range_search(q, float(T[qi, si]), params=ps)
    return I


rows = []
what = f"faiss_{a.method}" + ("" if FLAT else f"_np{a.nprobe}")
for si in shares:
    if a.cold:
        reclaim()
        b0 = read_bytes()
        t0 = time.perf_counter()
        got = one(qis[0], si)
        ms = (time.perf_counter() - t0) * 1e3
        e = exact[si, qis[0]]
        rec = len(np.intersect1d(got, e)) / len(e) if len(e) else float(len(got) == 0)
        rows.append(f"{what},1,{vis},{si},{qis[0]},{ms:.4f},{ms:.4f},{len(got)},{rec:.5f},{(read_bytes() - b0) / 1024:.1f},cold")
    res = []
    b0, tb = read_bytes(), time.time()
    for qi in qis:
        ts = []
        for _ in range(a.reps):
            t0 = time.perf_counter()
            got = one(qi, si)
            ts.append((time.perf_counter() - t0) * 1e3)
        e = exact[si, qi]
        rec = len(np.intersect1d(got, e)) / len(e) if len(e) else float(len(got) == 0)
        res.append((float(np.median(ts)), rec))
        rows.append(f"{what},1,{vis},{si},{qi},{np.median(ts):.4f},{min(ts):.4f},{len(got)},{rec:.5f},,warm")
    print(f"{what} {vis} share {si}: median {np.median([r[0] for r in res]):.1f} ms, mean recall {np.mean([r[1] for r in res]):.4f}; "
          f"read {(read_bytes() - b0) / 2**20:.0f} MiB in {time.time() - tb:.0f} s; {cg_report()}", flush=True)
new = not os.path.exists(a.out)
with open(a.out, "a") as f:
    f.write(("what,threads,vis,share_index,query,median_ms,min_ms,count,recall,disk_kb,kind\n" if new else "") + "\n".join(rows) + "\n")
