import re, sys, zlib
data = open(sys.argv[1], 'rb').read()
out = []
for m in re.finditer(rb'stream\r?\n', data):
    s = m.end(); e = data.find(b'endstream', s)
    if e < 0: continue
    try: d = zlib.decompress(data[s:e])
    except Exception: continue
    if b'TJ' not in d and b'Tj' not in d: continue
    txt = []
    for t in re.finditer(rb'\[((?:[^\]\\]|\\.)*)\]\s*TJ|\(((?:[^)\\]|\\.)*)\)\s*Tj|(T\*|Td|TD|ET)', d):
        if t.group(1) is not None:
            parts = re.findall(rb'\(((?:[^)\\]|\\.)*)\)|(-?\d+\.?\d*)', t.group(1))
            for p, num in parts:
                if p: txt.append(p.decode('latin-1'))
                elif num and float(num) < -200: txt.append(' ')
        elif t.group(2) is not None: txt.append(t.group(2).decode('latin-1'))
        else: txt.append('\n' if t.group(3) in (b'T*', b'ET') else ' ')
    out.append(''.join(txt))
open(sys.argv[2], 'w').write('\n'.join(out))
