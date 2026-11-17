# Unfiltered approximate k-NN through the public suites, unchanged: bench/vibe/run.sh on a VIBE checkout and
# bench/annb/run.sh on an ANN-Benchmarks checkout, with the suite's algorithms (ALGOS) and, for the quick suite, a
# shorter parameter grid. The grid is shortened by an overlay: for each algorithm the suite names, the run_groups of its
# entry in the checkout's config.yml are replaced by the suite's (written in that suite's own format) for the length of
# the run, and the checkout's file is put back afterwards (also at the start of the next run, if one was interrupted).
# Nothing in the public suites' code changes; their results, plots and exports stay in the checkouts.
# usage (through run.py): python3 -I run.py quick --stages public --vibe VIBE_DIR --annb ANNB_DIR [--datasets ...]
#        python3 -I public.py SUITE vibe|annb CHECKOUT --show    (print the overlaid entries and restore, no run)
# The checkouts need their images built first (bench/vibe/README.md, bench/annb/README.md); VIBE needs Apptainer.
import contextlib
import glob
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import REPO  # noqa: E402

CONFIG_GLOB = {"vibe": "vibe/algorithms/*/config.yml", "annb": "ann_benchmarks/algorithms/*/config.yml"}
BACKUP = ".suite-orig"


def log(*a):
    print("[public]", *a, flush=True)


def entries(cfg):
    """Every algorithm entry of a config.yml (point type -> metric -> [entries])."""
    for metrics in (cfg or {}).values():
        if isinstance(metrics, dict):
            for lst in metrics.values():
                if isinstance(lst, list):
                    for e in lst:
                        if isinstance(e, dict) and "name" in e:
                            yield e


@contextlib.contextmanager
def overlay(checkout, kind, ov):
    import yaml
    files = sorted(glob.glob(os.path.join(checkout, CONFIG_GLOB[kind])))
    for f in files:  # an interrupted run left its overlay in place
        if os.path.exists(f + BACKUP):
            shutil.move(f + BACKUP, f)
    touched, found = [], set()
    try:
        for f in files:
            with open(f) as fh:
                cfg = yaml.safe_load(fh)
            hit = False
            for e in entries(cfg):
                if e["name"] in ov:
                    e["run_groups"] = ov[e["name"]]
                    hit = True
                    found.add(e["name"])
            if hit:
                shutil.copy2(f, f + BACKUP)
                touched.append(f)
                with open(f, "w") as fh:
                    yaml.safe_dump(cfg, fh, default_flow_style=None, sort_keys=False)
        missing = set(ov) - found
        if missing:
            log(f"warning: no entry in {checkout} for {sorted(missing)}; their grids are the checkout's own")
        yield touched
    finally:
        for f in touched:
            shutil.move(f + BACKUP, f)


def run(suite, datasets, a):
    pub = suite["public"]
    for kind, checkout in (("vibe", a.vibe), ("annb", a.annb)):
        dss = [d for d in datasets if d in pub["datasets"][kind]]
        if not dss:
            continue
        if not checkout:
            log(f"{kind}: no checkout given (--{kind}); skipping {dss}")
            continue
        spec = pub[kind]
        env = dict(os.environ)
        if spec.get("algos"):
            env["ALGOS"] = " ".join(spec["algos"])
        script = os.path.join(REPO, "bench", kind, "run.sh")
        with overlay(checkout, kind, spec.get("overlay", {})) as touched:
            log(f"{kind}: overlaid {touched}; running {script} on {dss} with ALGOS={env.get('ALGOS', '(default)')}")
            r = subprocess.run(["bash", script, checkout] + dss, env=env)
            if r.returncode:
                log(f"{kind}: run.sh exited with {r.returncode}")


def main():
    import suites as S
    suite, kind, checkout = S.SUITES[sys.argv[1]], sys.argv[2], sys.argv[3]
    import yaml
    with overlay(checkout, kind, suite["public"][kind].get("overlay", {})) as touched:
        for f in touched:
            for e in entries(yaml.safe_load(open(f))):
                if e["name"] in suite["public"][kind]["overlay"]:
                    print(f"# {f}\n" + yaml.safe_dump({e["name"]: e["run_groups"]}, default_flow_style=None))
    print("restored:", touched)


if __name__ == "__main__":
    main()
