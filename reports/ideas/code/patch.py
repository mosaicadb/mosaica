import sys
p = sys.argv[1]
s = open(p).read()
a = s.index("            scanned = rer = 0; rec = 0; rec_eps = 0")
b = s.index("            us = US_FIXED")
new = '''            scanned = rer = 0; rec = 0; rec_eps = 0
            for qi in range(NQ):
                idx = np.flatnonzero(crank[qi, a] < nprobe) if nprobe < K else np.arange(N)
                e = E[qi, idx]; ub = e + m * sd[idx]; sc = S[qi, idx]
                ns = 100 if m == 0.0 else 30
                seed = np.argpartition(-e, ns)[:ns]
                tau = np.sort(sc[seed])[-k]
                if m == 0.0:
                    surv = seed; cnt = len(seed)
                else:
                    surv = np.flatnonzero(ub >= tau)          # everything a bound-ordered walk could reach
                    tau1 = np.sort(sc[surv])[-k]
                    cnt = 30 + int((ub >= tau1).sum())        # what the walk reads once the threshold has risen
                bid = idx[surv[np.argsort(-sc[surv])[:k]]]
                scanned += len(idx); rer += cnt
                rec += len(np.intersect1d(bid, gt[qi])); rec_eps += int((S[qi, bid] >= tk[qi] - 1e-3).sum())
'''
s = s[:a] + new + s[b:]
s = s.replace("    sd = rn * np.sqrt(1 - cos * cos) / cos / np.sqrt(n - 1)\n", "    sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)\n    bad = ~np.isfinite(sd) | ~np.isfinite(fac); sd[bad] = 0; fac[bad] = 0; cos[bad] = 1\n")
s = s.replace("O = (R / rn[:, None]) @ P.T", "O = np.nan_to_num(R / np.maximum(rn, 1e-12)[:, None]) @ P.T")
s = s.replace("    Cd = code(O, b); cn = np.linalg.norm(Cd, axis=1)", "    Cd = code(O, b); cn = np.maximum(np.linalg.norm(Cd, axis=1), 1e-12)")
open(p, 'w').write(s)
