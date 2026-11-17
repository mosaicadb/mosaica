# The run stage for one (dataset, system): build, then every query type x filter x setting x parameter the suite gives
# it, one query at a time on one pinned core. run.py starts this under taskset on the build cores.
# usage: python3 -I runsys.py SUITE DATASET SYSTEM --core C [--data DIR] [--types exact,threshold,approx]
#        [--filters none,random-1] [--step 1] [--reps 5] [--build-threads T] [--cap build|uncapped|FRACTION]
# Protocol (as ../threshold-db): the build may use several threads and is recorded apart (builds.csv). Queries: one
# thread pinned to --core (the system's own thread settings at 1), warm (everything in memory), each (query, parameter)
# --reps times; a row per query holds the median and the minimum. Each answer is compared with the f64 truth (exact
# types: items differing; approximate: recall) and dumped for check.py.
# Rows go to results/<suite>/<dataset>/<system>.csv, replacing the rows of the cells this run covers.
#
# The memory-cap axis (--cap; the suite's `caps`): `build` builds a disk system and keeps its index in the work
# directory (uncapped, several threads). `uncapped` or a fraction of the dataset's size (N x d x 4 bytes) then runs the
# cap axis's cells in a process that first moves into a memory cgroup of its own (capmem.py), opens the kept index
# (load), drops the system's files from the page cache, sets the limit (none for `uncapped`) with no swap, and runs.
# Each cell first times one cold query (query 0 once, after evict_cold: the full vectors or the page cache dropped, the
# codes or the graph kept), then the warm repetitions; after each cell the cgroup's counters are logged. Rows go to
# results/<suite>/<dataset>/caps/<system>.cap-<label>.csv, the counters to <system>.cap-<label>.mem.csv, and the run's
# status (ok, or does not fit and why) to caps/status.csv. A cap the kernel refuses, or one below the anonymous memory
# the system holds, is recorded as not fitting; an OOM kill is recorded by run.py.
#
# At scale (the suite's `scale`): no dumps; exact k-NN and approximate recall are compared on the prep's truth queries
# only, and threshold answers are recorded by size (their thresholds are estimates from a sample).
import argparse
import csv
import json
import os
import resource
import shutil
import sys
import time

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import Prep, data_dir, dump_dir, prep_dir, results_dir, summarise, work_dir  # noqa: E402
import capmem  # noqa: E402
import suites as S  # noqa: E402
from systems import REGISTRY, Unavailable  # noqa: E402
from systems.base import pin_thread  # noqa: E402

COLS = ["dataset", "type", "filter", "system", "role", "setting", "param", "query", "median_ms", "min_ms",
        "returned", "expected", "nonmatch", "recall"]
CAP_COLS = COLS + ["cap", "kind", "read_kb", "stats"]
MEM_COLS = ["cap", "cell", "usage_mib", "anon_mib", "file_mib", "peak_mib", "limit_mib", "failcnt", "oom_kill",
            "major_faults", "read_mib", "detail"]
STATUS_COLS = ["system", "cap", "cap_bytes", "status", "detail"]
BUILD_COLS = ["system", "status", "build_s", "threads", "index_gib", "peak_rss_gib", "detail"]


def log(*a):
    print(f"[{time.strftime('%H:%M:%S')}]", *a, flush=True)


def merge_csv(path, cols, rows, drop):
    """Writes rows to path, keeping the existing rows for which drop(row) is false."""
    old = []
    if os.path.exists(path):
        with open(path) as f:
            old = [r for r in csv.DictReader(f) if not drop(r)]
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path + ".part", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=cols)
        w.writeheader()
        w.writerows(old)
        w.writerows(rows)
    os.replace(path + ".part", path)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("suite", choices=list(S.SUITES))
    ap.add_argument("dataset")
    ap.add_argument("system", choices=list(REGISTRY))
    ap.add_argument("--core", type=int, required=True)
    ap.add_argument("--data", default=None)
    ap.add_argument("--types", default="exact,threshold,approx")
    ap.add_argument("--filters", default="")
    ap.add_argument("--step", type=int, default=1, help="time every step-th query only")
    ap.add_argument("--reps", type=int, default=0, help="repetitions per (query, parameter); 0: the suite's")
    ap.add_argument("--build-threads", type=int, default=0, help="0: the cores this process may use")
    ap.add_argument("--keep-work", action="store_true", help="keep the work directory (indexes) after the run")
    ap.add_argument("--cap", default="", help="build | uncapped | a fraction of the dataset's size (the cap axis)")
    a = ap.parse_args()
    data = data_dir(a.data)
    suite = S.SUITES[a.suite]
    ds, sname = a.dataset, a.system
    prep = Prep(prep_dir(data, ds))
    cap = bool(a.cap)
    cls = REGISTRY[sname]
    params = suite["params"].get(sname, {})
    # a system's own step and repetitions (the scans, which take seconds a query under a cap or at scale)
    reps = a.reps or (params.get("cap_reps", suite["caps"]["reps"]) if cap else params.get("reps", suite["reps"]))
    a.step = max(a.step, params.get("cap_step", suite["caps"]["step"]) if cap else params.get("step", 1))
    want_t = a.types.split(",")
    want_f = a.filters.split(",") if a.filters else (suite["caps"]["filters"] if cap else suite["filters"])
    plan = []  # (qtype, role, filter, parameters)
    for qt in ("exact", "threshold", "approx"):
        if qt not in want_t or qt not in cls.types:
            continue
        roles = [r for r, s in S.systems_for(suite, ds, qt, cap=cap) if s == sname]
        if not roles:
            continue
        for f, p in S.cells(suite, ds, qt, cap=cap):
            if f in want_f and (cls.unfiltered if f == "none" else cls.filters):
                plan.append((qt, roles[0], f, p))
    if not plan:
        log(f"{sname} has nothing to run on {ds} in {a.suite}")
        return
    work = work_dir(data, ds, sname)
    sysobj = cls(prep, work, params, core=a.core)
    threads = a.build_threads or len(os.sched_getaffinity(0))
    os.makedirs(results_dir(a.suite, ds), exist_ok=True)
    keep = a.keep_work or a.cap not in ("",)
    cg = None
    try:
        if a.cap in ("", "build"):
            if not build(a, ds, sname, sysobj, threads, "builds.csv" if not a.cap else "caps/builds.csv"):
                return
            if a.cap == "build":
                return
        else:
            cg = enter_cap(a, ds, sname, sysobj, prep)
            if cg is None:
                return
        run(a, suite, ds, sname, prep, cls, sysobj, plan, reps, data, cg)
    finally:
        sysobj.close()
        if cg is not None:
            cg.leave()
        if not keep:
            shutil.rmtree(work, ignore_errors=True)


DIFF_COLS = ["dataset", "system", "filter", "k", "query", "id", "side", "s", "t", "s_minus_t", "allowance",
             "beyond_rounding", "same_vector_as"]


def scale_diffs(prep, sysobj, ds, f, k, qi, ti, got, exp):
    """At scale, the items of an exact k-NN answer that differ from the streaming truth, each with its similarity s
    (f64, the stored f16 vector and the f64 query), the boundary t (the midpoint between the k-th and (k+1)-th
    similarity, prep_scale.py, computed in f32) and the allowance of check.py for the system's rounding (f16: 2^-11 on
    the vector and on the query; f32: 2^-24 on each and n 2^-24 of accumulation), plus n 2^-24 for the f32 truth;
    `same_vector_as`: an answer item with a bit-identical stored vector (a duplicate, an exact tie)."""
    X = prep.corpus()
    q = prep.queries()[qi]
    t = float(np.load(prep.f("knn_thr.npy" if f == "none" else f"knn_thr_{f}.npy"))[ti, (
        prep.meta["k"] if f == "none" else prep.meta["filters"][f]["k"]).index(k)])
    n = prep.d
    ux, uq, uacc = (2.0 ** -11, 2.0 ** -11, 0.0) if sysobj.rounding == "f16" else (2.0 ** -24, 2.0 ** -24, n * 2.0 ** -24)
    rows = []
    gs, es = set(got.tolist()), set(exp.tolist())
    for i in sorted(gs ^ es):
        x = np.asarray(X[i], dtype=np.float64)
        sv = float(x @ q)
        allow = float(np.abs(x * q).sum() * (ux + uq + uacc + n * 2.0 ** -24)) + 1e-12
        others = [j for j in (es if i in gs else gs) if np.array_equal(np.asarray(X[j]), np.asarray(X[i]))]
        rows.append(dict(dataset=ds, system=sysobj.name, filter=f, k=k, query=qi, id=i,
                         side="returned, not in the truth" if i in gs else "in the truth, not returned",
                         s=f"{sv:.9f}", t=f"{t:.9f}", s_minus_t=f"{sv - t:.3g}", allowance=f"{allow:.3g}",
                         beyond_rounding=int(abs(sv - t) > allow), same_vector_as=" ".join(map(str, others))))
    return rows


def snap(sysobj):
    """Counters around one query (outside its timing): the system's own (vexact's reader reads), the storage bytes its
    processes read, and this process's major faults."""
    c = dict(sysobj.counters())
    c["proc_read_kb"] = capmem.io_read_bytes(sysobj.io_pids()) / 1024
    c["majflt"] = resource.getrusage(resource.RUSAGE_SELF).ru_majflt
    return c


def stats_of(sysobj, s0, s1):
    d = {k: round(s1[k] - s0[k], 1) for k in s1}
    d.update(sysobj.query_stats() or {})
    return json.dumps(d, separators=(",", ":"))


def cap_label(c):
    return "uncapped" if c == "uncapped" else f"{float(c) * 100:g}"


def status(a, ds, sname, nbytes, st, detail=""):
    merge_csv(os.path.join(results_dir(a.suite, ds), "caps", "status.csv"), STATUS_COLS,
              [dict(system=sname, cap=cap_label(a.cap), cap_bytes=nbytes, status=st, detail=detail)],
              lambda r: r["system"] == sname and r["cap"] == cap_label(a.cap))


def build(a, ds, sname, sysobj, threads, fname):
    rdir = results_dir(a.suite, ds)
    t = time.time()
    try:
        extra = sysobj.build(threads) or {}
    except Unavailable as e:
        log(f"{sname} unavailable: {e}")
        merge_csv(os.path.join(rdir, fname), BUILD_COLS, [dict(system=sname, status="unavailable", detail=str(e))],
                  lambda r: r["system"] == sname)
        return False
    build_s = time.time() - t
    # the larger of this process's peak and its children's (fit.py, acorn_bench)
    rss = max(resource.getrusage(w).ru_maxrss for w in (resource.RUSAGE_SELF, resource.RUSAGE_CHILDREN)) / 2 ** 20
    log(f"{sname} on {ds}: built in {build_s:.0f} s on {threads} threads; peak RSS {rss:.2f} GiB; {extra}")
    merge_csv(os.path.join(rdir, fname), BUILD_COLS,
              [dict(system=sname, status="ok", build_s=f"{build_s:.1f}", threads=threads,
                    index_gib=f"{sysobj.index_bytes() / 2 ** 30:.3f}", peak_rss_gib=f"{rss:.2f}",
                    detail=json.dumps(extra))], lambda r: r["system"] == sname)
    return True


def enter_cap(a, ds, sname, sysobj, prep):
    """Moves into a cgroup, opens the kept index, drops its files from the page cache and sets the cap. None (and a
    status row) when it does not fit."""
    nbytes = 0 if a.cap == "uncapped" else int(round(float(a.cap) * prep.dataset_bytes()))
    if not sysobj.disk and nbytes:
        status(a, ds, sname, nbytes, "does not fit", "in memory: it holds the f32 vectors (100% of the dataset's size) "
               "in anonymous memory")
        return None
    cg = capmem.Cgroup().enter()
    log(f"{sname} on {ds}: in {cg.dir} (cgroup v{cg.v}); {cg.report()}")
    try:
        if sysobj.disk:
            extra = sysobj.load() or {}
        else:  # an in-memory system: built here, in the group, and run uncapped
            if not build(a, ds, sname, sysobj, len(os.sched_getaffinity(0)), "caps/builds.csv"):
                cg.leave()
                return None
            extra = {}
    except Unavailable as e:
        status(a, ds, sname, nbytes, "unavailable", str(e))
        cg.leave()
        return None
    n = capmem.evict_files(sysobj.data_files())
    cg.reclaim()
    log(f"{sname}: opened {extra}; {n} files dropped from the page cache; {cg.report()}")
    if nbytes:
        why = cg.cap(nbytes)
        if why:
            log(f"{sname} does not fit under the cap of {nbytes / 2 ** 20:.0f} MiB: {why}")
            status(a, ds, sname, nbytes, "does not fit", why)
            cg.leave()
            return None
    log(f"{sname}: cap {cap_label(a.cap)} ({nbytes / 2 ** 20:.0f} MiB of a dataset of "
        f"{prep.dataset_bytes() / 2 ** 20:.0f} MiB); {cg.report()}")
    status(a, ds, sname, nbytes, "running", json.dumps(extra))
    return cg


def run(a, suite, ds, sname, prep, cls, sysobj, plan, reps, data, cg):
    rdir = results_dir(a.suite, ds)
    pin_thread(a.core)
    sysobj.single_thread()
    Q32 = prep.queries().astype(np.float32)
    qis = list(range(0, prep.nq, a.step))
    scale = bool(prep.meta.get("scale"))
    tq = {q: i for i, q in enumerate(prep.meta.get("truth_queries", range(prep.nq)))}
    ddir = dump_dir(data, a.suite, ds, sname)
    dumps = not scale and cg is None
    if dumps:
        os.makedirs(ddir, exist_ok=True)
    label = cap_label(a.cap) if cg is not None else None
    out_csv = os.path.join(rdir, "caps", f"{sname}.cap-{label}.csv") if cg is not None else os.path.join(rdir, f"{sname}.csv")
    mem_csv = os.path.join(rdir, "caps", f"{sname}.cap-{label}.mem.csv")
    setup_rows, done, diff_rows = [], set(), []
    if cg is not None:
        merge_csv(mem_csv, MEM_COLS, [], lambda r: True)
    for qt, role, f, p in plan:
        filtered = f != "none"
        t0 = time.perf_counter()
        sysobj.set_visible(prep.vis(f), None if not filtered else f)
        vis_ms = (time.perf_counter() - t0) * 1e3
        order = prep.order(f)
        fk = prep.meta["k"] if not filtered else prep.meta["filters"][f]["k"]
        shares = prep.filter_shares(f)
        if qt == "threshold":
            T = prep.thresholds(f)
            params_list = [dict(share=s) for s in p["shares"]]
        elif qt == "exact":
            params_list = [dict(k=k) for k in p["k"]]
        else:
            params_list = [dict(k=k, **g) for k in p["k"] for g in sysobj.approx_grid()]
        for setting in sysobj.settings(qt, filtered):
            t0 = time.perf_counter()
            sysobj.use(setting)
            use_ms = (time.perf_counter() - t0) * 1e3
            setup_rows.append(dict(type=qt, filter=f, setting=setting, set_visible_ms=f"{vis_ms:.2f}",
                                   setting_ms=f"{use_ms:.2f}"))
            done.add((qt, f))
            base = f"{qt}__{f}__{setting}"
            if dumps:
                with open(os.path.join(ddir, base + ".json"), "w") as fo:
                    json.dump(dict(params=params_list, rounding=sysobj.rounding, queries=qis), fo)
                dump = open(os.path.join(ddir, base + ".bin"), "wb")
            rows = []
            for pi, par in enumerate(params_list):
                meds, bad, recs = [], 0, []

                def call_for(qi):
                    if qt == "threshold":
                        si = shares.index(par["share"])
                        return lambda: sysobj.range(Q32[qi], T[qi, si])
                    if qt == "exact":
                        return lambda: sysobj.knn(Q32[qi], par["k"])
                    g = {x: y for x, y in par.items() if x != "k"}
                    return lambda: sysobj.approx(Q32[qi], par["k"], g)

                def expected(qi):
                    """The exact answer for query qi (None where it is not known: at scale, off the truth queries
                    and for thresholds)."""
                    if scale and (qt == "threshold" or qi not in tq):
                        return None
                    m = prep.answer_size(f, par["share"]) if qt == "threshold" else par["k"]
                    return np.asarray(order[tq[qi] if scale else qi, :m])

                if cg is not None:  # one cold query: query 0 once, after what it reads is dropped
                    qi = qis[0]
                    sysobj.qi = qi
                    sysobj.evict_cold(cg)
                    b0 = capmem.io_read_bytes(sysobj.io_pids())
                    s0 = snap(sysobj)
                    t0 = time.perf_counter()
                    got = call_for(qi)()
                    ms = sysobj.own_ms if sysobj.own_ms is not None else (time.perf_counter() - t0) * 1e3
                    cold_stats = stats_of(sysobj, s0, snap(sysobj))
                    got = np.asarray(got, dtype=np.int64)
                    exp = expected(qi)
                    hit = len(np.intersect1d(got, exp)) if exp is not None else 0
                    rows.append(dict(dataset=ds, type=qt, filter=f, system=sname, role=role, setting=setting,
                                     param=json.dumps(par, separators=(",", ":")), query=qi, median_ms=f"{ms:.4f}",
                                     min_ms=f"{ms:.4f}", returned=len(got), expected="" if exp is None else len(exp),
                                     nonmatch="" if exp is None else len(got) + len(exp) - 2 * hit,
                                     recall="" if exp is None else f"{hit / max(1, len(exp)):.5f}", cap=label,
                                     kind="cold", read_kb=f"{(capmem.io_read_bytes(sysobj.io_pids()) - b0) / 1024:.0f}",
                                     stats=cold_stats))
                b0 = capmem.io_read_bytes(sysobj.io_pids()) if cg is not None else 0
                for qi in qis:
                    call = call_for(qi)
                    sysobj.qi = qi
                    ts = []
                    qstats = ""
                    for r in range(reps):
                        last = cg is not None and r == reps - 1
                        if last:
                            s0 = snap(sysobj)
                        t0 = time.perf_counter()
                        got = call()
                        ms = (time.perf_counter() - t0) * 1e3
                        ts.append(sysobj.own_ms if sysobj.own_ms is not None else ms)
                        if last:
                            qstats = stats_of(sysobj, s0, snap(sysobj))
                    got = np.asarray(got, dtype=np.int64)
                    got = got[got >= 0]
                    exp = expected(qi)
                    if exp is not None:
                        hit = len(np.intersect1d(got, exp))
                        nonmatch = len(got) + len(exp) - 2 * hit
                        rec = hit / len(exp) if len(exp) else 1.0
                        bad += nonmatch
                        recs.append(rec)
                        if scale and qt == "exact" and nonmatch:
                            diff_rows += scale_diffs(prep, sysobj, ds, f, par["k"], qi, tq[qi], got, exp)
                    meds.append(float(np.median(ts)))
                    if dumps:
                        dump.write(np.array([qi, pi, len(got)], "<u4").tobytes() + np.sort(got).astype("<u4").tobytes())
                    row = dict(dataset=ds, type=qt, filter=f, system=sname, role=role, setting=setting,
                               param=json.dumps(par, separators=(",", ":")), query=qi,
                               median_ms=f"{np.median(ts):.4f}", min_ms=f"{min(ts):.4f}", returned=len(got),
                               expected="" if exp is None else len(exp), nonmatch="" if exp is None else nonmatch,
                               recall="" if exp is None else f"{rec:.5f}")
                    if cg is not None:
                        row.update(cap=label, kind="warm", read_kb="", stats=qstats)
                    rows.append(row)
                med, p90 = summarise(meds)
                note = ""
                if cg is not None:
                    st = cg.stats()
                    rd = (capmem.io_read_bytes(sysobj.io_pids()) - b0) / 2 ** 20
                    cell = f"{qt} {f} {setting} {json.dumps(par, separators=(',', ':'))}"
                    merge_csv(mem_csv, MEM_COLS, [dict(cap=label, cell=cell, usage_mib=f"{st['usage'] / 2**20:.1f}",
                              anon_mib=f"{st['anon'] / 2**20:.1f}", file_mib=f"{st['file'] / 2**20:.1f}",
                              peak_mib=f"{st['max_usage'] / 2**20:.1f}",
                              limit_mib="" if st["limit"] < 0 or st["limit"] >= 1 << 60 else f"{st['limit'] / 2**20:.1f}",
                              failcnt=st["failcnt"], oom_kill=st["oom_kill"], major_faults=st["pgmajfault"],
                              read_mib=f"{rd:.1f}", detail="")], lambda r: False)
                    note = f"; cold {rows[-len(qis) - 1]['median_ms']} ms; read {rd:.0f} MiB; {cg.report()}"
                log(f"{sname} {qt} {f} {setting} {par}: median {med:.3f} ms, p90 {p90:.3f} ms, "
                    + (f"mean recall {np.mean(recs):.4f}" if qt == "approx" and recs else
                       f"items differing {bad}" if recs else
                       f"answer sizes median {np.median([int(r['returned']) for r in rows[-len(qis):]]):.0f}") + note)
            if dumps:
                dump.close()
            key = (qt, f, setting)
            merge_csv(out_csv, CAP_COLS if cg is not None else COLS, rows,
                      lambda r: (r["type"], r["filter"], r["setting"]) == key)
    merge_csv(os.path.join(rdir, "caps" if cg is not None else "", f"{sname}{'.cap-' + label if cg is not None else ''}_setup.csv"),
              ["type", "filter", "setting", "set_visible_ms", "setting_ms"], setup_rows,
              lambda r: (r["type"], r["filter"]) in done)
    if diff_rows:
        merge_csv(os.path.join(rdir, "scale_diffs.csv"), DIFF_COLS, diff_rows, lambda r: r["system"] == sname)
        beyond = sum(int(r["beyond_rounding"]) for r in diff_rows)
        log(f"{sname}: {len(diff_rows)} items differ from the streaming truth in exact k-NN; {beyond} beyond the "
            "rounding allowance (scale_diffs.csv)")
    if cg is not None:
        status(a, ds, sname, cg.limit or 0, "ok", cg.report())
    log(f"{sname} on {ds}: done; peak RSS {resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2 ** 20:.2f} GiB")


if __name__ == "__main__":
    main()
