# Export a query-offset profile (G x n, per group, in the local basis) as one .npy file for the Rust build.
# Every profile-dependent build parameter (bit allocation, certificate weights, randomised whitening, block membership,
# rates, alpha) is derived from this file in Rust.
# usage: python3 profile.py NAME KIND OUT.npy
#   KIND: qmix (item/text mixture, text-tuned) | qi (corpus items as pseudo-queries) | h<w> (((1-w) sqrt Qi + w sqrt iso)^2,
#         iso isotropic with Qi's trace; h0.25 is the data-only "H25" profile) | npz:<file>:<key> (e.g. dataonly prof_m768.npz:Qi50)
import numpy as np, sys
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME, KIND, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
I = np.load(VEC + f"combined/idx_{NAME}/index.npz")
qi = I["Qi"].astype(np.float64)
if KIND == "qmix": P = I["Qmix"]
elif KIND == "qi": P = qi
elif KIND.startswith("h"):
    w = float(KIND[1:]); iso = np.repeat(qi.sum(1, keepdims=True) / qi.shape[1], qi.shape[1], 1)
    P = ((1 - w) * np.sqrt(qi) + w * np.sqrt(iso)) ** 2
elif KIND.startswith("npz:"):
    _, f, k = KIND.split(":"); P = np.load(f)[k]
else: raise SystemExit(f"unknown profile kind {KIND}; write qmix, qi, h<w> or npz:<file>:<key>")
np.save(OUT, np.ascontiguousarray(P.astype(np.float64)))
print("profile", KIND, P.shape, "mean trace", P.sum(1).mean())
