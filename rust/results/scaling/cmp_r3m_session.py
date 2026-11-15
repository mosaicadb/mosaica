# R3M at 2,000 against 4,000 timed in the same session (t_r3m_2000_{1,2}, t_r3m_4000_{3,4}), and that session's 4,000
# against the earlier session's (t_r3m_4000_{1,2}). usage: cmp_r3m_session.py DATA_DIR
import csv, math, re, sys, statistics as st
R = sys.argv[1] + "/res"
def e2e(t):
    return {(r["class"], r["what"], float(r["answer_frac"])): float(r["median_of_runs_ms"])
            for r in csv.DictReader(open(f"{R}/{t}/e2e2.csv")) if r["threads"] == "1"}
two, four, old = [[e2e(f"t_r3m_{s}_{i}") for i in reps] for s, reps in (("2000", (1, 2)), ("4000", (3, 4)), ("4000", (1, 2)))]
m = lambda runs, k: st.mean(x[k] for x in runs)
lbl = lambda k: f"{k[0]} {'thr %g%%' % (k[2] * 100) if k[1] == 'index2_thr' else 'k-NN k = %d' % k[2]}"
print("## R3M: 2,000 against 4,000 in the same session (ms, mean of two processes [each])\n")
print("| cell | 4,000 | 2,000 | 2,000 against 4,000 | earlier session's 4,000 | this session against the earlier |\n|---|---|---|---|---|---|")
keys = sorted(four[0], key=lambda k: (k[0], k[1] != "index2_thr", k[2]))
for k in keys:
    f = lambda runs: f"{m(runs, k):.1f} [{'/'.join('%.1f' % x[k] for x in runs)}]"
    print(f"| {lbl(k)} | {f(four)} | {f(two)} | {100 * (m(two, k) / m(four, k) - 1):+.0f}% | {f(old)} | {100 * (m(four, k) / m(old, k) - 1):+.0f}% |")
gm = lambda ks: 100 * (math.exp(st.mean(math.log(m(two, k) / m(four, k)) for k in ks)) - 1)
thr = [k for k in keys if k[1] == "index2_thr"]
print(f"\nGeometric mean of 2,000 against 4,000: threshold cells {gm(thr):+.1f}% (item {gm([k for k in thr if k[0] == 'item']):+.1f}%, "
      f"text {gm([k for k in thr if k[0] == 'text']):+.1f}%); exact k-NN {gm([k for k in keys if k[1] != 'index2_thr']):+.1f}%")
print("\n## Counts per query, first process of each (clusters on the index path / on full vectors / items entering the first level / groups / full reads)\n")
print("| cell | 2,000 | 4,000 |\n|---|---|---|")
pat = r"breakdown2 counts \S+ (\S+) (\S+): per query ([\d.]+) clusters on the index path, ([\d.]+) on full vectors, ([\d.]+) items entering the first level, [\d.]+ item-levels, ([\d.]+) groups, ([\d.]+) full reads"
cnt = {s: {(mm[1], mm[2]): mm.groups()[2:] for mm in (re.match(pat, l) for l in open(f"{R}/t_r3m_{s}.out")) if mm} for s in ("2000_1", "4000_3")}
for k in cnt["4000_3"]:
    print(f"| {k[0]} {float(k[1]) * 100:g}% | {' / '.join(cnt['2000_1'][k])} | {' / '.join(cnt['4000_3'][k])} |")
