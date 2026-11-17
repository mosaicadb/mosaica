# The run stage for one (dataset, system): build, then every query type x filter x setting x parameter the suite gives
# it, one query at a time on one pinned core. run.py starts this under taskset on the build cores.
# usage: python3 -I runsys.py SUITE DATASET SYSTEM --core C [--data DIR] [--types exact,threshold,approx]
#        [--filters none,random-1] [--step 1] [--reps 5] [--build-threads T]
# Protocol (as ../threshold-db): the build may use several threads and is recorded apart (builds.csv). Queries: one
# thread pinned to --core (the system's own thread settings at 1), warm (everything in memory), each (query, parameter)
# --reps times; a row per query holds the median and the minimum. Each answer is compared with the f64 truth (exact
# types: items differing; approximate: recall) and dumped for check.py.
# Rows go to results/<suite>/<dataset>/<system>.csv, replacing the rows of the cells this run covers.
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
from common import Prep, data_dir, dump_dir, prep_dir, results_dir, summarise, work_dir, share_label  # noqa: E402
import suites as S  # noqa: E402
from systems import REGISTRY, Unavailable  # noqa: E402
from systems.base import pin_thread  # noqa: E402

COLS = ["dataset", "type", "filter", "system", "role", "setting", "param", "query", "median_ms", "min_ms",
        "returned", "expected", "nonmatch", "recall"]


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
    a = ap.parse_args()
    data = data_dir(a.data)
    suite = S.SUITES[a.suite]
    ds, sname = a.dataset, a.system
    prep = Prep(prep_dir(data, ds))
    reps = a.reps or suite["reps"]
    want_t = a.types.split(",")
    want_f = a.filters.split(",") if a.filters else suite["filters"]
    cls = REGISTRY[sname]
    plan = []  # (qtype, role, filter, parameters)
    for qt in ("exact", "threshold", "approx"):
        if qt not in want_t or qt not in cls.types:
            continue
        roles = [r for r, s in S.systems_for(suite, ds, qt) if s == sname]
        if not roles:
            continue
        for f, p in S.cells(suite, ds, qt):
            if f in want_f:
                plan.append((qt, roles[0], f, p))
    if not plan:
        log(f"{sname} has nothing to run on {ds} in {a.suite}")
        return
    work = work_dir(data, ds, sname)
    params = suite["params"].get(sname, {})
    sysobj = cls(prep, work, params, core=a.core)
    threads = a.build_threads or len(os.sched_getaffinity(0))
    os.makedirs(results_dir(a.suite, ds), exist_ok=True)
    try:
        run(a, suite, ds, sname, prep, cls, sysobj, plan, threads, reps, data)
    finally:
        sysobj.close()
        if not a.keep_work:
            shutil.rmtree(work, ignore_errors=True)


def run(a, suite, ds, sname, prep, cls, sysobj, plan, threads, reps, data):
    rdir = results_dir(a.suite, ds)
    t = time.time()
    try:
        extra = sysobj.build(threads) or {}
    except Unavailable as e:
        log(f"{sname} unavailable: {e}")
        merge_csv(os.path.join(rdir, "builds.csv"), ["system", "status", "build_s", "threads", "index_gib",
                                                       "peak_rss_gib", "detail"],
                  [dict(system=sname, status="unavailable", detail=str(e))], lambda r: r["system"] == sname)
        return
    build_s = time.time() - t
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2 ** 20
    log(f"{sname} on {ds}: built in {build_s:.0f} s on {threads} threads; peak RSS {rss:.2f} GiB; {extra}")
    merge_csv(os.path.join(rdir, "builds.csv"), ["system", "status", "build_s", "threads", "index_gib",
                                                   "peak_rss_gib", "detail"],
              [dict(system=sname, status="ok", build_s=f"{build_s:.1f}", threads=threads,
                    index_gib=f"{sysobj.index_bytes() / 2 ** 30:.3f}", peak_rss_gib=f"{rss:.2f}",
                    detail=json.dumps(extra))], lambda r: r["system"] == sname)

    pin_thread(a.core)
    sysobj.single_thread()
    Q32 = prep.queries().astype(np.float32)
    qis = list(range(0, prep.nq, a.step))
    ddir = dump_dir(data, a.suite, ds, sname)
    os.makedirs(ddir, exist_ok=True)
    rows, setup_rows, done = [], [], set()
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
            with open(os.path.join(ddir, base + ".json"), "w") as fo:
                json.dump(dict(params=params_list, rounding=cls.rounding, queries=qis), fo)
            dump = open(os.path.join(ddir, base + ".bin"), "wb")
            for pi, par in enumerate(params_list):
                meds, bad, recs = [], 0, []
                for qi in qis:
                    if qt == "threshold":
                        si = shares.index(par["share"])
                        m = prep.answer_size(f, par["share"])
                        exp = np.asarray(order[qi, :m])
                        call = lambda: sysobj.range(Q32[qi], T[qi, si])
                    elif qt == "exact":
                        exp = np.asarray(order[qi, :par["k"]])
                        call = lambda: sysobj.knn(Q32[qi], par["k"])
                    else:
                        exp = np.asarray(order[qi, :par["k"]])
                        g = {x: y for x, y in par.items() if x != "k"}
                        call = lambda: sysobj.approx(Q32[qi], par["k"], g)
                    sysobj.qi = qi
                    ts = []
                    for _ in range(reps):
                        t0 = time.perf_counter()
                        got = call()
                        ms = (time.perf_counter() - t0) * 1e3
                        ts.append(sysobj.own_ms if sysobj.own_ms is not None else ms)
                    got = np.asarray(got, dtype=np.int64)
                    got = got[got >= 0]
                    hit = len(np.intersect1d(got, exp))
                    nonmatch = len(got) + len(exp) - 2 * hit
                    rec = hit / len(exp) if len(exp) else 1.0
                    bad += nonmatch
                    recs.append(rec)
                    meds.append(float(np.median(ts)))
                    dump.write(np.array([qi, pi, len(got)], "<u4").tobytes() + np.sort(got).astype("<u4").tobytes())
                    rows.append(dict(dataset=ds, type=qt, filter=f, system=sname, role=role, setting=setting,
                                     param=json.dumps(par, separators=(",", ":")), query=qi,
                                     median_ms=f"{np.median(ts):.4f}", min_ms=f"{min(ts):.4f}", returned=len(got),
                                     expected=len(exp), nonmatch=nonmatch, recall=f"{rec:.5f}"))
                med, p90 = summarise(meds)
                log(f"{sname} {qt} {f} {setting} {par}: median {med:.3f} ms, p90 {p90:.3f} ms, "
                    f"{'mean recall %.4f' % np.mean(recs) if qt == 'approx' else 'items differing %d' % bad}")
            dump.close()
    merge_csv(os.path.join(rdir, f"{sname}.csv"), COLS, rows, lambda r: (r["type"], r["filter"]) in done)
    merge_csv(os.path.join(rdir, f"{sname}_setup.csv"), ["type", "filter", "setting", "set_visible_ms", "setting_ms"],
              setup_rows, lambda r: (r["type"], r["filter"]) in done)
    log(f"{sname} on {ds}: done; peak RSS {resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2 ** 20:.2f} GiB")


if __name__ == "__main__":
    main()
