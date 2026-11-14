import sys
sys.path.append('/home/joe/.local/lib/python3.10/site-packages')
import numpy as np
W='/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/annideas/sim/'
P='/home/joe/code/vexact-data/bioclip2-1m/prep/'; I='/home/joe/code/vexact-benchup-data/index_r4/'
Q=np.load(P+'queries.npy').astype(np.float32); NQ=len(Q)
truth=np.load(P+'order.npy',mmap_mode='r')[:,:1000].astype(np.int64)
grp=np.load(I+'grp.npy'); cl=np.load(W+'cl1000.npy'); C=np.load(W+'C1000.npy'); sub=np.load(W+'sub16.npy'); Cs=np.load(W+'Csub16.npy')
rad=lambda C: np.sqrt(np.maximum(1-(C*C).sum(1),0))
kp=(C@Q.T).T+0.2*rad(C)[None]; ks=(Cs@Q.T).T+0.2*rad(Cs)[None]
prank=np.argsort(np.argsort(-kp,1),1); par=np.arange(len(Cs))//16; szs=np.bincount(sub,minlength=len(Cs)); szp=np.bincount(cl,minlength=1000)
print('K1000 +0.2rad: groups opened by nprobe:',{p:round(float(np.mean([len(set(grp[np.argsort(-kp[q])[:p]])) for q in range(NQ)])),1) for p in (5,13,35,94,121,191)})
for Pp in (32,64,128,256,1000):
    key=np.where(prank[:,par]<Pp,ks,-np.inf); order=np.argsort(-key,1); rank=np.argsort(order,1)
    trk=np.where(prank[np.arange(NQ)[:,None],cl[truth]]<Pp,rank[np.arange(NQ)[:,None],sub[truth]],10**9)
    cum=np.cumsum(szs[order],1)
    for k in (10,100,1000):
        out=[]
        for tg in (0.8,0.9,0.99):
            r=np.sort(trk[:,:k].ravel()); v=r[int(np.ceil(tg*r.size-1e-9))-1]
            if v>=10**9: out.append(f'r{tg}: unreachable'); continue
            p=int(v)+1; npar=np.mean([len(set(par[order[q,:p]])) for q in range(NQ)]); ng=np.mean([len(set(grp[par[order[q,:p]]])) for q in range(NQ)])
            out.append(f'r{tg}: subs {p} items {cum[:,p-1].mean():.0f} parents {npar:.0f} groups {ng:.1f}')
        print(f'two-level P={Pp:4d} (centres scored {1000+16*min(Pp,1000)}) k={k}: '+' | '.join(out),flush=True)
