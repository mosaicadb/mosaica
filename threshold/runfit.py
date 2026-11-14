# Run vexact-bench's rust/tools/fit.py unchanged (the data-only build: clusters, groups, local bases, H25 profile).
import sys, os, runpy
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__))); import shim  # noqa
FIT = "/home/joe/code/vexact-bench/rust/tools/fit.py"; sys.argv = [FIT] + sys.argv[1:]
runpy.run_path(FIT, run_name="__main__")
