# Shared helpers for the probe-order replay: recall@10 against rows scanned for a cluster order.
import numpy as np

SHARES = [0.005, 0.008, 0.01, 0.016, 0.02, 0.03, 0.04, 0.063, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25, 0.3]
TABLE = [0.005, 0.016, 0.03, 0.063, 0.125, 0.25]
FGRID = [0.0, 0.1, 0.2, 0.35, 0.5, 0.75]


def curve(score, gtcl, sizes):
    """score: nq x K cluster keys (probe in descending order, ties by lower cluster id, as Rust's stable sort);
    gtcl: nq x k cluster of each true neighbour. Returns rec (nq x K: recall after p = 1..K clusters) and
    items (nq x K: rows scanned after p clusters)."""
    nq, K = score.shape
    k = gtcl.shape[1]
    ordr = np.argsort(-score, axis=1, kind="stable")
    rank = np.empty_like(ordr)
    rank[np.arange(nq)[:, None], ordr] = np.arange(K)[None, :]
    items = np.cumsum(sizes[ordr], axis=1).astype(np.float64)
    gr = rank[np.arange(nq)[:, None], gtcl]
    found = np.zeros((nq, K))
    np.add.at(found, (np.repeat(np.arange(nq), k), gr.ravel()), 1.0 / k)
    return np.cumsum(found, axis=1), items


def summarise(rec, items, N, shares=SHARES, nprobes=(16, 64, 256)):
    """Fixed-nprobe curve (mean recall vs mean share scanned, as nprobe works), interpolated at the shares; the
    per-query-budget variant (each query probes whole clusters while its rows scanned stay within the share); the
    recall at given nprobes; and the number of queries with recall 0 at the nprobe whose mean share is nearest 1.6%."""
    mr = rec.mean(0)
    ms = items.mean(0) / N
    fixed = {f"{s:g}": float(np.interp(s, np.r_[0, ms], np.r_[0, mr])) for s in shares}
    budget = {}
    for s in shares:
        ok = items <= s * N
        p = ok.sum(1)  # clusters within budget (prefix, since items increase)
        r = np.where(p > 0, rec[np.arange(len(rec)), np.maximum(p - 1, 0)], 0.0)
        budget[f"{s:g}"] = float(r.mean())
    at_np = {str(p): dict(recall=float(mr[p - 1]), share=float(ms[p - 1])) for p in nprobes if p <= rec.shape[1]}
    p16 = int(np.argmin(np.abs(ms - 0.016)))
    zero = dict(nprobe=p16 + 1, share=float(ms[p16]), n_zero=int((rec[:, p16] < 1e-9).sum()),
                n_below_half=int((rec[:, p16] < 0.5 - 1e-9).sum()), recall=float(mr[p16]))
    return dict(fixed=fixed, budget=budget, nprobe=at_np, zero16=zero)


def fit_objective(score_fn, gtcl, sizes, grid=FGRID, target=0.95):
    """Approx::fit: for each f, the fewest clusters p (same for every query) with mean recall@10 >= target, and the
    mean rows scanned there; the f with the fewest rows (first on ties)."""
    res = []
    K = len(sizes)
    for f in grid:
        rec, items = curve(score_fn(f), gtcl, sizes)
        mr = rec.sum(0) / len(rec)
        hit = np.where(mr >= target)[0]
        p = int(hit[0]) if len(hit) else K - 1
        res.append(dict(f=f, p95=p + 1, rows=float(items[:, p].mean())))
    best = min(res, key=lambda r: r["rows"])  # min keeps the first of equal values, as the Rust fold with <
    return best["f"], res
