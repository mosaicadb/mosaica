import numpy as np, glob
R="/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
I=np.load(R+"idx_m768/index.npz"); qcls=I["qcls"]; K=int(I["K"])
D=[dict(np.load(f)) for f in sorted(glob.glob(R+"out_m768_w*.npz"))]
for nm in ("V3sub+C1+C2","V3sub+C1"):
    cent=sum(d[nm+"|cent"] for d in D); srch=sum(d[nm+"|srch"] for d in D)
    for cl in ("item","text"):
        m=qcls==cl
        print(nm,cl,"centres opened %",(100*cent[:,m].mean(1)/K).round(1),"clusters needing a range search %",(100*srch[:,m].mean(1)/K).round(1))
