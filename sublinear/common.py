# Shared helpers: chunked reads of X.npy without a memory map (keeps resident memory low), the timing lock, queries.
import numpy as np, os, time, json
V = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(V, "sublinear")
N, n = 1_000_000, 768
CH = 25_000
_t0 = time.time()
def log(*a): print(f"[{time.time()-_t0:6.0f}s]", *a, flush=True)
def wait_lock():
    w = False
    while os.path.exists(os.path.join(V, "TIMING.lock")):
        if not w: log("TIMING.lock present; waiting"); w = True
        time.sleep(5)
def chunks(ch=CH):
    """Yield (start, float32 array) over the rows of X.npy."""
    with open(os.path.join(V, "X.npy"), "rb") as f:
        ver = np.lib.format.read_magic(f)
        shape, fo, dt = (np.lib.format.read_array_header_1_0 if ver == (1, 0) else np.lib.format.read_array_header_2_0)(f)
        assert shape == (N, n) and dt == np.float32 and not fo
        for i in range(0, N, ch):
            wait_lock()
            yield i, np.fromfile(f, dtype=np.float32, count=min(ch, N - i) * n).reshape(-1, n)
def rows(idx):
    """Rows of X.npy by sorted index (small sets only)."""
    idx = np.asarray(idx); out = np.empty((len(idx), n), np.float32)
    with open(os.path.join(V, "X.npy"), "rb") as f:
        ver = np.lib.format.read_magic(f)
        (np.lib.format.read_array_header_1_0 if ver == (1, 0) else np.lib.format.read_array_header_2_0)(f)
        off = f.tell()
        for j, i in enumerate(idx):
            f.seek(off + int(i) * n * 4); out[j] = np.fromfile(f, dtype=np.float32, count=n)
    return out
def queries():
    qi = np.load(os.path.join(V, "item_q_idx.npy"))
    Qt = np.load(os.path.join(V, "text_q.npy")).astype(np.float32); Qt /= np.linalg.norm(Qt, axis=1, keepdims=True)
    Qi = rows(qi)
    kinds = [p[0] for p in json.load(open(os.path.join(V, "text_q.json")))]
    return np.concatenate([Qt, Qi]), ["text"] * len(Qt) + ["item"] * len(Qi), kinds + ["item"] * len(Qi), qi
NQ = 469
def score_row(q):
    """Exact float32 scores of query q against all N rows (from S.f32, query-major)."""
    return np.fromfile(os.path.join(OUT, os.environ.get("SFILE", "S.f32")), dtype=np.float32, count=N, offset=q * N * 4)
SIZES = [31_250, 62_500, 125_000, 250_000, 500_000, 999_900]
