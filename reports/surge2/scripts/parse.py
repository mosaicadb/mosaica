import re, sys
for path in sys.argv[1:]:
    txt = open(path, encoding='utf-8', errors='replace').read()
    a = txt.index('<h3>Recall/Queries per second (1/s)</h3>')
    b = txt.index('<h3>Recall/Build time (s)</h3>')
    seg = txt[a:b]
    pts = []
    cur = None
    for line in seg.splitlines():
        m = re.search(r'^\s*label: "([^"]*)",\s*$', line)
        if m: cur = m.group(1); continue
        m = re.search(r'\{ x: ([0-9.eE+-]+) , y: ([0-9.eE+-]+), label: "([^"]*)" \}', line)
        if m: pts.append((float(m.group(1)), float(m.group(2)), cur, m.group(3)))
    print(path.split('/')[-1], len(pts), 'points', len(set(p[2] for p in pts)), 'algos')
    for r in (0.9, 0.95, 0.99, 0.999):
        best = {}
        for x, y, alg, lab in pts:
            if x >= r and (alg not in best or y > best[alg][0]): best[alg] = (y, x, lab)
        top = sorted(best.items(), key=lambda kv: -kv[1][0])[:5]
        print(' recall >=', r)
        for alg, (y, x, lab) in top: print('   %-28s qps %9.0f at recall %.4f  %s' % (alg, y, x, lab[:90]))
    bf = [p for p in pts if 'rute' in (p[2] or '')]
    for p in bf: print(' brute:', p[2], round(p[1],1), p[0])
