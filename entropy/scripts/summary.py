import json, numpy as np, math, os
L = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy/logs/"
for d in ("bioclip768", "bioclip1024", "glove100", "nyt256", "ada1536", "te3_1536", "te3_3072"):
    r = json.load(open(L + f"res_{d}.json")); KS = np.array(r["KS"]); n = r["n"]; g0 = np.array(r["g:gauss"])
    J = {m: g0 - np.array(r[m]) for m in ("g:t", "g:hist", "s:gauss", "c:gauss", "g:ellip", "c:ellip", "g:escape")}
    best = np.maximum.reduce([J["c:ellip"], J["g:ellip"], J["c:gauss"], J["g:hist"]])
    print(f"== {d}: N {r['N']} n {n} K {r['K']} G {r['G']}; kurtosis by band {np.round(r['stats']['kurt'], 2).tolist()}, beyond 3.94 sigma {100 * r['stats']['beyond394']:.3f}%, max {r['stats']['maxabs']:.1f} sigma")
    print("   differential J lower bounds (bits/item) at top-k:  k = " + " ".join(f"{k:7d}" for k in KS))
    for m in J: print(f"     {m:10s}                                      " + " ".join(f"{x:7.1f}" for x in J[m]))
    print("     best/k (bits per coordinate)                    " + " ".join(f"{x:7.3f}" for x in best / KS))
    print("     floor factor 2^(-J/k)                           " + " ".join(f"{2 ** (-x):7.3f}" for x in best / KS))
    nn = json.load(open(L + f"nn_{d}.json")) if os.path.exists(L + f"nn_{d}.json") else None
    print("   matched distortion (same quantiser cells): bits under the Gaussian model, bits saved, share of bits")
    for i, (key, q) in enumerate(r["quant"].items()):
        s = q["gauss"] - q["cluster+scale"]; kth = q["k_theta"]; jk = float(np.interp(min(kth, KS[-1]), KS, best))
        a = 0.7 if q["target_bits"] / 8 * 768 / n <= 96 else 1.3
        line = (f"     {key:>6s} target {q['target_bits']:6.0f} b ({q['target_bits'] / 8:5.0f} B) k_theta {kth:6.0f}{'*' if kth > KS[-1] else ' '} gauss {q['gauss']:7.0f} | t {q['gauss'] - q['t']:5.1f} cluster {q['gauss'] - q['cluster']:5.1f} "
                f"item-scale {q['gauss'] - q['gauss+scale']:5.1f} both {s:6.1f} = {100 * s / q['gauss']:4.1f}% | J(top k_theta) {jk:6.1f} floor x{2 ** (-jk / min(kth, KS[-1])):.3f} | half-width at fixed bytes x{(1 + s / q['gauss']) ** (-a):.3f}")
        if nn and i < 4:
            o = nn[[k for k in nn if k.startswith("rd_global")][0]]
            line += f" | neighbour net: exact {o['net_saving_exact'][i]:6.1f} lossy {o['net_saving_lossy'][i]:6.1f} (bin alone {o['bin_only_saving'][i]:5.1f})"
        print(line)
    if nn:
        v = nn["variants"]; print("   nearest neighbour: " + "; ".join(f"{k}: energy x{x['mean_ratio']:.2f}, cos>0.99 {100 * x['cos99']:.1f}%, >0.999 {100 * x['cos999']:.1f}%, identical {100 * x['exact']:.1f}%" for k, x in v.items()))
