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
schemes={}
cl=np.load(W+'cl1000.npy'); C=np.load(W+'C1000.npy'); rad=np.load(W+'rad1000.npy').astype(np.float32)
schemes['K1000 centre']=(cl,C,None); schemes['K1000 +0.2rad']=(cl,C,0.2*rad)
for s in (4,8,16): schemes[f'sub x{s}']=(np.load(W+f'sub{s}.npy'),np.load(W+f'Csub{s}.npy'),None)
schemes['flat 4096']=(np.load(W+'flat4096.npy'),np.load(W+'Cflat4096.npy'),None)
D={}
print('== A. fixed nprobe: probes / mean items scanned to reach mean recall (all 150 queries; img / txt items in brackets)')
for name,(a,Cc,b) in schemes.items():
    d=prep(a,Cc,b); d['a']=a; D[name]=d
    for k in KS:
        row=[]
        for tg in TG:
            p,it,_=fixed(d,kinds['all'],k,tg); pi,iti,_=fixed(d,kinds['img'],k,tg); pt,itt,_=fixed(d,kinds['txt'],k,tg)
            row.append(f'r{tg}: p={p} items={it:.0f} (img {iti:.0f} / txt {itt:.0f})')
        print(f'{name:14s} k={k:5d} | '+' | '.join(row),flush=True)
print('== B. two-level probing for sub-clusters: parents ranked by parent centre(+0.2rad), top-P parents opened, their sub-centres ranked')
base=D['K1000 +0.2rad']
for s in (8,16):
    d=D[f'sub x{s}']; a=d['a']
    for Pp in (16,32,64,128,256):
        # sub-cluster key = -inf unless parent rank < Pp
        par=np.arange(d['K'])//s; ok=base['rank'][:,par]<Pp
        key=np.where(ok,d['cs'],-np.inf); order=np.argsort(-key,1); rank=np.empty_like(order); rank[np.arange(NQ)[:,None],order]=np.arange(d['K'])[None]
        d2=dict(K=d['K'],cum=np.cumsum(d['sz'][order],1),trk=rank[np.arange(NQ)[:,None],a[truth]])
        # ranks beyond Pp*s are unreachable
        d2['trk']=np.where(np.take_along_axis(ok,a[truth],1),d2['trk'],10**9)
        row=[]
        for k in KS:
            for tg in (0.9,0.99):
                trk=d2['trk'][:,:k]; h=np.array([(trk<p).mean() for p in range(1,Pp*s+1)])
                if h[-1]<tg: row.append(f'k{k} r{tg}: unreachable({h[-1]:.3f})')
                else:
                    p=int(np.argmax(h>=tg)+1); row.append(f'k{k} r{tg}: p={p} items={d2["cum"][:,p-1].mean():.0f}')
        print(f'sub x{s} P={Pp:3d} centres scored={1000+Pp*s:5d} | '+' | '.join(row),flush=True)
print('== C. adaptive stopping vs fixed nprobe at matched mean recall (items scanned; in-sample delta, then cross-fitted halves)')
ev=np.arange(0,NQ,2); od=np.arange(1,NQ,2)
for name in ('K1000 centre','sub x16','flat 4096'):
    d=D[name]
    radv=rad if name.startswith('K1000') else None
    for k in KS:
        tau=taus(d,k)
        for tg in TG:
            p,it,_=fixed(d,kinds['all'],k,tg); om,oit=oracle(d,kinds['all'],k,tg)
            grid=np.linspace(-0.05,0.6,131)
            b=best_adaptive(d,tau,k,tg,kinds['all'],grid)
            # cross-fit
            cf=[]
            for fit,test in ((ev,od),(od,ev)):
                bb=best_adaptive(d,tau,k,tg,fit,grid)
                if bb: 
                    pp=adaptive(d,tau,k,bb[0]); cf.append(evalp(d,pp,test,k))
            cfs=' xfit rec/items '+' ; '.join(f'{r:.3f}/{i:.0f}' for r,i,_ in cf)
            fz=best_adaptive(d,tau,k,tg,kinds['all'],grid,freeze=max(4,d['K']//250))
            line=f'{name:12s} k={k:5d} r{tg}: fixed p={p} items={it:.0f} | live-tau delta={b[0]:.3f} probes={b[2]:.1f} items={b[1]:.0f} |{cfs} | frozen-tau items={fz[1]:.0f} probes={fz[2]:.1f} | per-query oracle probes={om:.1f} items={oit:.0f}'
            if radv is not None:
                br=best_adaptive(d,tau,k,tg,kinds['all'],np.linspace(-0.2,3,161),scale=radv)
                line+=f' | rad-scaled items={br[1]:.0f}'
            print(line,flush=True)
