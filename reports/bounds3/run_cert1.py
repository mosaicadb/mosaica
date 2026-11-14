import sys, math, time, numpy as np
from cert import certify
ds = [int(x) for x in sys.argv[1].split(",")]
lnd = float(sys.argv[2]); X = float(sys.argv[3]); K = int(sys.argv[4])
eta = float(sys.argv[5]) if len(sys.argv) > 5 else 0.001
M = int(sys.argv[6]) if len(sys.argv) > 6 else 65536
k0 = int(sys.argv[7]) if len(sys.argv) > 7 else 400
t0 = time.time()
ok, mx, arg = certify(ds, lnd, X, K, eta=eta, M=M, k0=k0)
print(f"ds {ds} ln delta {lnd:.6f} X {X} K {K} eta {eta}: certified {ok}; max ln tail {mx:.6f}; argmax shares {np.array(arg)/K}; {time.time()-t0:.0f}s")
