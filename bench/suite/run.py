# The suite's entry point.
# usage: python3 -I bench/suite/run.py quick|full [--datasets A,B] [--systems S,T] [--types exact,threshold,approx]
#        [--filters none,random-1] [--stages prep,run,check,report] [--data DIR] [--core 3] [--build-cores 0-2]
#        [--step 1] [--reps 5] [--hdf5-dirs D1,D2] [--no-f16] [--drop-hdf5] [--drop-corpus] [--keep-work]
#        [--bioclip-raw DIR --bioclip-text DIR]
#        python3 -I bench/suite/run.py quick|full --stages public [--vibe DIR] [--annb DIR] [--datasets ...]
#        python3 -I bench/suite/run.py quick|full --stages prep,cap,report [--caps 0.5,0.25,0.1] [--datasets ...]
#        python3 -I bench/suite/run.py scale|scale-test [--stages prep,run,report] [--datasets laion-10m,...]
# Stages, per dataset in turn: prep (prep.py), run (runsys.py once per system, each in its own process started under
# taskset on --build-cores, its queries pinned to --core), check (check.py); then report (report.py) over every
# dataset with results. `public` runs unfiltered approximate k-NN through VIBE and ANN-Benchmarks (public.py); it is
# not in the default stages, since it needs those checkouts and their images.
# --drop-corpus removes the dataset's corpus files after its check (the truth and visible sets stay), to keep one large
# dataset on disk at a time.
# `cap` (the memory-cap axis, on the suite's `caps` datasets; not a default stage): for each system the axis names, one
# uncapped process builds its index (runsys.py --cap build), then one process per cap runs the axis's cells in a
# memory cgroup of its own: "uncapped" first, then each fraction of the dataset's size (runsys.py --cap F). On cgroup
# v2 each is started under `systemd-run --user --scope -p MemoryMax=infinity -p MemorySwapMax=0`; on v1 it makes its
# own child group. A process the OOM killer stops is recorded as not fitting; the index is removed after the last cap.
# The scale suites prep with prep_scale.py (streamed from LAION's files; no check stage).
import argparse
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import HERE, data_dir, prep_dir  # noqa: E402
import suites as S  # noqa: E402
import prep as P  # noqa: E402


def log(*a):
    print(f"[run {time.strftime('%H:%M:%S')}]", *a, flush=True)


def busy_cores(sec=1.0):
    """How many cores were busy over the last `sec` seconds (from /proc/stat)."""
    def read():
        v = [int(x) for x in open("/proc/stat").readline().split()[1:]]
        return sum(v), v[3] + v[4]
    t0, i0 = read()
    time.sleep(sec)
    t1, i1 = read()
    return (os.cpu_count() or 1) * (1 - (i1 - i0) / max(1, t1 - t0))


def systems_of(suite, ds, types):
    out = []
    for qt in types:
        for _, s in S.systems_for(suite, ds, qt):
            if s not in out:
                out.append(s)
    return out


def runsys_cmd(a, ds, s, types, extra=()):
    """The command that runs one system: under taskset on the build cores, with the system's own Python."""
    from systems import REGISTRY
    py = os.environ.get(REGISTRY[s].python_env, "") if REGISTRY[s].python_env else ""
    cmd = ["taskset", "-c", a.build_cores, py or sys.executable, "-I", os.path.join(HERE, "runsys.py"),
           a.suite, ds, s, "--core", str(a.core), "--data", a.data, "--types", ",".join(types),
           "--step", str(a.step), "--reps", str(a.reps)] + list(extra)
    if a.filters:
        cmd += ["--filters", a.filters]
    return cmd


def run_caps(a, suite, ds, types):
    """The memory-cap axis on one dataset: a build, then one capped process per cap, for each system."""
    import shutil
    import capmem
    from common import results_dir, work_dir
    from runsys import STATUS_COLS, merge_csv
    c = suite.get("caps")
    if not c or ds not in c["datasets"]:
        log(f"{ds}: not on the {a.suite} suite's cap axis")
        return
    fracs = [x for x in (a.caps.split(",") if a.caps else [str(f) for f in c["fractions"]])]
    v, _ = capmem.Cgroup.detect()
    wrap = []
    if v == 2:
        if not shutil.which("systemd-run"):
            sys.exit("cgroup v2 without systemd-run: start each capped run in a cgroup of its own")
        wrap = ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=infinity", "-p", "MemorySwapMax=0", "--"]
    names = []
    for qt in types:
        for _, s in S.systems_for(suite, ds, qt, cap=True):
            if s not in names and (not a.systems or s in a.systems.split(",")):
                names.append(s)
    env = dict(os.environ, OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS="1", MKL_NUM_THREADS="1")
    for s in names:
        log(f"{ds}: {s}: build for the cap axis")
        r = subprocess.run(runsys_cmd(a, ds, s, types, ["--cap", "build"]), env=env)
        if r.returncode:
            log(f"{ds}: {s}: build failed ({r.returncode})")
            continue
        for f in ["uncapped"] + fracs:
            busy = busy_cores()
            if busy > 0.5:
                log(f"warning: {busy:.1f} cores busy before running {s} capped: is something else running?")
            log(f"{ds}: {s}: cap {f}")
            r = subprocess.run(wrap + runsys_cmd(a, ds, s, types, ["--cap", f]), env=env)
            capmem.cleanup_stale()
            if r.returncode in (-9, 137):
                label = "uncapped" if f == "uncapped" else f"{float(f) * 100:g}"
                merge_csv(os.path.join(results_dir(a.suite, ds), "caps", "status.csv"), STATUS_COLS,
                          [dict(system=s, cap=label, cap_bytes="", status="does not fit",
                                detail="killed (SIGKILL: the OOM killer) while running under the cap")],
                          lambda row: row["system"] == s and row["cap"] == label)
                log(f"{ds}: {s}: killed under cap {f}")
            elif r.returncode:
                log(f"{ds}: {s}: cap {f} failed ({r.returncode})")
        if not a.keep_work:
            shutil.rmtree(work_dir(a.data, ds, s), ignore_errors=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("suite", choices=list(S.SUITES))
    ap.add_argument("--datasets", default="")
    ap.add_argument("--systems", default="")
    ap.add_argument("--types", default="exact,threshold,approx")
    ap.add_argument("--filters", default="")
    ap.add_argument("--stages", default="prep,run,check,report")
    ap.add_argument("--core", type=int, default=3, help="the timing core")
    ap.add_argument("--build-cores", default="0-2", help="the cores builds may use (taskset list)")
    ap.add_argument("--step", type=int, default=1)
    ap.add_argument("--reps", type=int, default=0)
    ap.add_argument("--drop-corpus", action="store_true")
    ap.add_argument("--keep-work", action="store_true")
    ap.add_argument("--vibe", default="", help="a VIBE checkout (stage public)")
    ap.add_argument("--annb", default="", help="an ANN-Benchmarks checkout (stage public)")
    ap.add_argument("--caps", default="", help="the cap fractions to run (stage cap; default the suite's)")
    ap.add_argument("--download", default="", help="scale prep: where LAION's parts are downloaded, one at a time")
    P.add_args(ap)
    a = ap.parse_args()
    a.data = data_dir(a.data)
    suite = S.SUITES[a.suite]
    stages = a.stages.split(",")
    datasets = a.datasets.split(",") if a.datasets else suite["datasets"]
    types = a.types.split(",")
    for ds in datasets:
        if ds not in suite["datasets"]:
            log(f"note: {ds} is not in the {a.suite} suite's list; running it with the suite's parameters")
    if "public" in stages:
        import public
        public.run(suite, datasets, a)
    for ds in datasets:
        if "prep" in stages:
            if suite.get("scale"):
                import prep_scale
                prep_scale.prep(suite, ds, a)
            else:
                P.prep(suite, ds, a)
        if "cap" in stages:
            run_caps(a, suite, ds, types)
        if "run" in stages:
            names = systems_of(suite, ds, types)
            if a.systems:
                names = [s for s in names if s in a.systems.split(",")]
            for s in names:
                busy = busy_cores()
                if busy > 0.5:
                    log(f"warning: {busy:.1f} cores busy before running {s}: is something else running?")
                cmd = runsys_cmd(a, ds, s, types)
                if a.keep_work:
                    cmd += ["--keep-work"]
                env = dict(os.environ, OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS="1", MKL_NUM_THREADS="1")
                log(f"{ds}: {s}")
                r = subprocess.run(cmd, env=env)
                if r.returncode:
                    log(f"{ds}: {s} failed with exit code {r.returncode}")
        if "check" in stages and not suite.get("scale"):
            import check
            check.check(a.suite, ds, a.data)
        if a.drop_corpus:
            for f in ("corpus32.npy", "corpus16.npy", "learn.npy"):
                p = os.path.join(prep_dir(a.data, ds), f)
                if os.path.exists(p):
                    os.remove(p)
            log(f"{ds}: corpus files removed (the truth stays)")
    if "report" in stages:
        import report
        report.report(a.suite)


if __name__ == "__main__":
    main()
