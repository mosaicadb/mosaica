# Builds (or opens) the default-layout and layout-(c) indexes of one dataset in the scratch directories of scratch.sh,
# timing the cost model once on core 3 for the default layout; layout (c)'s cache then takes the default layout's
# constants under its own key, so that both layouts' approximate queries choose paths and races by the same model and
# differ only in what their reads cost.
# usage: taskset -c 0-2 python3 -I build.py DATASET [DEST]
import sys
import time

import vexact_py

ds = sys.argv[1]
dest = sys.argv[2] if len(sys.argv) > 2 else "/home/user/scratch-sub"
for flavour, sb in (("def", 0), ("c", 16)):
    t = time.time()
    ix = vexact_py.Index(f"{dest}/{flavour}/{ds}", core=3, sub_build=sb)
    print(f"{ds} {flavour}: {ix.n} rows, {ix.clusters} clusters, open/build {time.time() - t:.1f}s; measured {ix.measured_costs}", flush=True)
    del ix
vals = [l for l in open(f"{dest}/def/{ds}/compute-constants.txt") if not l.startswith("key=")]
key = [l for l in open(f"{dest}/c/{ds}/compute-constants.txt") if l.startswith("key=")]
with open(f"{dest}/c/{ds}/compute-constants.txt", "w") as f:
    f.writelines(key + vals)
ix = vexact_py.Index(f"{dest}/c/{ds}", core=3, sub_build=16)
print(f"{ds} c: constants now {ix.measured_costs}", flush=True)
