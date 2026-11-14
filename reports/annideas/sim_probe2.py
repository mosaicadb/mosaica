import sys, time, heapq
sys.path.append('/home/joe/.local/lib/python3.10/site-packages')
import numpy as np
W='/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/annideas/sim/'
P='/home/joe/code/vexact-data/bioclip2-1m/prep/'
S=np.load(W+'S.npy'); NQ,N=S.shape
Q=np.load(P+'queries.npy').astype(np.float32)
truth=np.load(P+'order.npy',mmap_mode='r')[:,:1000].astype(np.int64)
KS=[1,10,100,1000]; TG=[0.8,0.9,0.99]
kinds={'all':np.arange(NQ),'img':np.arange(50),'txt':np.arange(50,150)}
def prep(a,C,bonus=None):
    K=len(C); cs=(C@Q.T).T  # NQ x K
    key=cs if bonus is None else cs+bonus[None]
    order=np.argsort(-key,1); rank=np.empty_like(order); rank[np.arange(NQ)[:,None],order]=np.arange(K)[None]
    sz=np.bincount(a,minlength=K)
    cum=np.cumsum(sz[order],1)          # items scanned after p probes
    trk=rank[np.arange(NQ)[:,None],a[truth]]  # cluster rank of each true item
    return dict(K=K,cs=cs,key=key,order=order,rank=rank,cum=cum,trk=trk,sz=sz)
def fixed(d,qs,k,tg):
    trk=d['trk'][qs,:k]; K=d['K']
    # recall at p = mean(trk<p)
    h=np.bincount(trk.ravel(),minlength=K).cumsum()/trk.size
    p=int(np.searchsorted(h,tg-1e-12)+1)
    return p, d['cum'][qs,min(p,K)-1].mean(), h[min(p,K)-1]
def oracle(d,qs,k,tg):
    # per-query least probes so that the query's own recall >= tg (ceil)
    trk=np.sort(d['trk'][qs,:k],1); need=int(np.ceil(tg*k-1e-9)); p=trk[:,need-1]+1
    return p.mean(), d['cum'][qs,p-1].mean()
def taus(d,k,M=30000):
    # tau[q][p] = k-th best score among items in the first p clusters (for p=0..Pmax), -inf if unknown
    out=[]
    for q in range(NQ):
        top=np.argpartition(-S[q],M)[:M]; r=d['rank'][q,d['a'][top]]; s=S[q,top]
        o=np.argsort(r,kind='stable'); r=r[o]; s=s[o]
        K=d['K']; t=np.full(K+1,-np.inf); h=[]; j=0
        for p in range(1,K+1):
            while j<len(r) and r[j]<p:
                if len(h)<k: heapq.heappush(h,s[j])
                elif s[j]>h[0]: heapq.heapreplace(h,s[j])
                j+=1
            if len(h)==k: t[p]=h[0]
        out.append(t)
    return np.array(out)
def adaptive(d,tau,k,delta,scale=None,freeze=None,minp=1):
    # stop before the first cluster p (0-based rank) with key_p < tau(after p probes) - delta*scale_p ; returns probes per query
    K=d['K']; ks=np.take_along_axis(d['key'],d['order'],1)  # keys in probe order
    t=tau[:,:K] if freeze is None else np.repeat(tau[:,freeze:freeze+1],K,1)
    sc=1.0 if scale is None else scale[d['order']]
    stop=ks<t-delta*sc
    stop[:,:max(minp, freeze or 0)]=False
    return np.where(stop.any(1),stop.argmax(1),K)
def evalp(d,p,qs,k):
    rec=(d['trk'][qs,:k]<p[qs,None]).mean(); it=d['cum'][qs,np.maximum(p[qs],1)-1].mean()
    return rec,it,p[qs].mean()
def best_adaptive(d,tau,k,tg,qs,grid,**kw):
    best=None
    for dl in grid:
        p=adaptive(d,tau,k,dl,**kw); rec,it,pm=evalp(d,p,qs,k)
        if rec>=tg and (best is None or it<best[1]): best=(dl,it,pm,rec)
    return best

cl=np.load(W+'cl1000.npy'); C=np.load(W+'C1000.npy')
def radof(C): return np.sqrt(np.maximum(1-(C*C).sum(1),0)).astype(np.float32)   # RMS radius of a cluster of unit vectors about its mean
D={}
print('== A2. radius bonus on every scheme: order by centre score + f * RMS radius; items scanned at mean recall 0.8/0.9/0.99')
allc={'K1000':(cl,C)}
for s_ in (4,16): allc[f'sub x{s_}']=(np.load(W+f'sub{s_}.npy'),np.load(W+f'Csub{s_}.npy'))
allc['flat 4096']=(np.load(W+'flat4096.npy'),np.load(W+'Cflat4096.npy'))
for name,(a,Cc) in allc.items():
    r=radof(Cc)
    for f in (0.0,0.1,0.2,0.3,0.4):
        d=prep(a,Cc,f*r); d['a']=a; D[(name,f)]=d
        print(f'{name:10s} f={f:.1f} | '+' | '.join(f'k{k}: '+'/'.join(f'{fixed(d,kinds["all"],k,tg)[1]:.0f}' for tg in TG) for k in KS)+' || img k10: '+'/'.join(f'{fixed(d,kinds["img"],10,tg)[1]:.0f}' for tg in TG)+' txt k10: '+'/'.join(f'{fixed(d,kinds["txt"],10,tg)[1]:.0f}' for tg in TG),flush=True)
print('== C2. adaptive stop on the radius-bonus order (key = cs + f rad; stop when key < tau - delta); cross-fitted halves')
ev=np.arange(0,NQ,2); od=np.arange(1,NQ,2)
for name,f in (('K1000',0.2),('sub x16',0.2)):
    d=D[(name,f)]
    for k in KS:
        tau=taus(d,k)
        for tg in TG:
            p,it,_=fixed(d,kinds['all'],k,tg); om,oit=oracle(d,kinds['all'],k,tg)
            grid=np.linspace(-0.1,0.6,141); b=best_adaptive(d,tau,k,tg,kinds['all'],grid); cf=[]
            for fit,test in ((ev,od),(od,ev)):
                bb=best_adaptive(d,tau,k,tg,fit,grid)
                if bb: cf.append(evalp(d,adaptive(d,tau,k,bb[0]),test,k))
            print(f'{name:8s} f={f} k={k:5d} r{tg}: fixed p={p} items={it:.0f} | live-tau items={b[1]:.0f} probes={b[2]:.1f} | xfit rec/items '+' ; '.join(f'{r:.3f}/{i:.0f}' for r,i,_ in cf)+f' | oracle items={oit:.0f}',flush=True)
print('== D. within-cluster order by key x.mhat (K1000, +0.2rad order): prune item if beta*key + c*sqrt((1-beta^2)(1-key^2)) < true k-th score')
X=np.load(P+'corpus16.npy',mmap_mode='r'); Ch=C/np.linalg.norm(C,axis=1,keepdims=True)
key=np.concatenate([(X[i:i+50000].astype(np.float32)*Ch[cl[i:i+50000]]).sum(1) for i in range(0,N,50000)])
d=D[('K1000',0.2)]; beta_all=(Ch@Q.T).T
print('key spread inside a cluster: mean sd',np.mean([key[cl==c].std() for c in range(0,1000,25)]),' key mean',key.mean())
for k,npb in ((10,94),(100,121),(1000,191),(10,13),(100,17)):
    for c in (0.0,0.05,0.1,0.15,0.2,0.3,1.0):
        fr=[];lost=[]
        for q in range(NQ):
            m=d['rank'][q,cl]<npb; ids=np.nonzero(m)[0]; b=beta_all[q,cl[ids]]; kk=key[ids]
            tau=S[q,truth[q,k-1]]
            keep=b*kk+c*np.sqrt(np.maximum((1-b*b)*(1-kk*kk),0))>=tau
            fr.append(1-keep.mean()); t=truth[q,:k]; inprobe=m[t]; kept=np.zeros(N,bool); kept[ids[keep]]=True
            lost.append((inprobe&~kept[t]).sum()/max(inprobe.sum(),1))
        print(f'k={k} np={npb} c={c:.2f}: pruned {np.mean(fr):.3f} of scanned items, lost {np.mean(lost):.4f} of the true top-k inside the probe set',flush=True)
