import csv, sys
rows = list(csv.DictReader(open(sys.argv[1], encoding='utf-8', errors='replace')))
print(list(rows[0].keys()))
for r in (0.9, 0.95, 0.99, 0.995, 0.999):
    best = {}
    for row in rows:
        try: rec = float(row['recall/ap']); q = float(row['qps'])
        except Exception: continue
        a = row['algorithm']
        if rec >= r and (a not in best or q > best[a][0]): best[a] = (q, rec)
    print('recall >=', r, ' '.join('%s %.0f(%.4f);' % (a, q, rec) for a, (q, rec) in sorted(best.items(), key=lambda kv: -kv[1][0])))
mx = {}
for row in rows:
    try: rec = float(row['recall/ap'])
    except Exception: continue
    a = row['algorithm']; mx[a] = max(mx.get(a, 0), rec)
print('max recall per algo', mx)
