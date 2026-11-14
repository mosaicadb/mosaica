import sys, time
sys.path.append('/home/joe/.local/lib/python3.10/site-packages')
import numpy as np
W='/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/annideas/sim/'
P='/home/joe/code/vexact-data/bioclip2-1m/prep/'; I='/home/joe/code/vexact-benchup-data/index_r4/'
t0=time.time()
X=np.load(P+'corpus16.npy').astype(np.float32); N=len(X)
Q=np.load(P+'queries.npy').astype(np.float32)
S=(X@Q.T).T.copy(); np.save(W+'S.npy',S); print('scores',S.shape,time.time()-t0,flush=True)
perm=np.load(I+'perm.npy'); cstart=np.load(I+'cstart.npy'); K=len(cstart)-1
cl=np.empty(N,np.int32)
for c in range(K): cl[perm[cstart[c]:cstart[c+1]]]=c
np.save(W+'cl1000.npy',cl)
def centres(a,K):
    C=np.zeros((K,X.shape[1]),np.float64); 
    o=np.argsort(a,kind='stable'); b=np.r_[0,np.cumsum(np.bincount(a,minlength=K))]
    for c in range(K):
        if b[c+1]>b[c]: C[c]=X[o[b[c]:b[c+1]]].mean(0,dtype=np.float64)
    return C.astype(np.float32)
C=centres(cl,K); np.save(W+'C1000.npy',C)
rad=np.sqrt(np.bincount(cl,weights=((X-C[cl])**2).sum(1),minlength=K)/np.maximum(np.bincount(cl,minlength=K),1)); np.save(W+'rad1000.npy',rad)
print('base',time.time()-t0,flush=True)
rng=np.random.default_rng(1)
def km(Xc,k,it=8):
    n=len(Xc)
    if n<=k: return np.arange(n)%k
    c=Xc[rng.choice(n,k,replace=False)].copy()
    for _ in range(it):
        d=-2*Xc@c.T+(c*c).sum(1)[None]
        a=d.argmin(1)
        for j in range(k):
            m=a==j
            if m.any(): c[j]=Xc[m].mean(0)
            else: c[j]=Xc[rng.integers(n)]
    d=-2*Xc@c.T+(c*c).sum(1)[None]
    return d.argmin(1)
for s in (4,8,16):
    sub=np.empty(N,np.int32)
    for c in range(K):
        ids=perm[cstart[c]:cstart[c+1]]
        sub[ids]=c*s+km(X[ids],s)
    np.save(W+f'sub{s}.npy',sub); np.save(W+f'Csub{s}.npy',centres(sub,K*s)); print('sub',s,time.time()-t0,flush=True)
# flat 4096
Kf=4096; tr=X[rng.choice(N,200000,replace=False)]; c=tr[rng.choice(len(tr),Kf,replace=False)].copy()
for it in range(6):
    a=np.concatenate([(tr[i:i+50000]@c.T-0.5*(c*c).sum(1)[None]).argmax(1) for i in range(0,len(tr),50000)])
    for j in range(Kf):
        m=a==j
        if m.any(): c[j]=tr[m].mean(0)
    print('flat it',it,time.time()-t0,flush=True)
a=np.concatenate([(X[i:i+50000]@c.T-0.5*(c*c).sum(1)[None]).argmax(1) for i in range(0,N,50000)]).astype(np.int32)
np.save(W+'flat4096.npy',a); np.save(W+'Cflat4096.npy',centres(a,Kf)); print('flat done',time.time()-t0,np.bincount(a,minlength=Kf).max(),flush=True)
