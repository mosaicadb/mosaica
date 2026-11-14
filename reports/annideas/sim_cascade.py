import sys, os
sys.path.append('/home/joe/.local/lib/python3.10/site-packages')
import numpy as np
A='/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/annideas/'
W=A+'sim/'; DD=A+'dump/'
P='/home/joe/code/vexact-data/bioclip2-1m/prep/'; I='/home/joe/code/vexact-benchup-data/index_r4/'
S=np.load(W+'S.npy',mmap_mode='r'); N=S.shape[1]
rowent=np.fromfile(DD+'row_ent.bin',dtype='<u4').astype(np.int64); cstart=np.load(I+'cstart.npy')
truth=np.load(P+'order.npy',mmap_mode='r'); Q=np.load(P+'queries.npy').astype(np.float32)
sub=np.load(W+'sub16.npy'); Csub=np.load(W+'Csub16.npy'); rsub=np.sqrt(np.maximum(1-(Csub*Csub).sum(1),0))
LV=[0,1,2,3,5,6,7]; BY={0:6.5,1:11.0,2:15.5,3:20.0,5:37.0,6:66.5,7:106.9}
KS=[1,10,100,1000]
qis=sorted(int(f.split('_')[1].split('.')[0]) for f in os.listdir(DD) if f.startswith('order_'))
dt=np.dtype([('r','<u4'),('lb','<f4'),('ub','<f4')])
SEL=[('np',16),('np',64),('np',128),('np',256),('sp',256),('sp',1024),('sp',2048),('sp',4096)]
data=[]
for qi in qis:
    order=np.fromfile(DD+f'order_{qi}.bin',dtype='<u4'); crank=np.empty(len(order),np.int64); crank[order]=np.arange(len(order))
    s=np.asarray(S[qi]); tr=np.asarray(truth[qi,:1000]).astype(np.int64)
    subrank=np.empty(len(Csub),np.int64); subrank[np.argsort(-(Csub@Q[qi]+0.2*rsub))]=np.arange(len(Csub))
    base=None; mid={}; hw={}
    for l in LV:
        a=np.fromfile(DD+f'iv_{qi}_{l}.bin',dtype=dt); o=np.argsort(a['r']); a=a[o]
        if base is None:
            base=a['r'].astype(np.int64); ent=rowent[base]; true=s[ent].astype(np.float32)
            pr=crank[np.searchsorted(cstart,base,side='right')-1]; sr=subrank[sub[ent]]
            pos=np.full(N,-1,np.int64); pos[ent]=np.arange(len(ent)); tp=pos[tr]
        assert np.array_equal(a['r'],base)
        mid[l]=((a['lb'].astype(np.float64)+a['ub'])/2).astype(np.float32); hw[l]=((a['ub'].astype(np.float64)-a['lb'])/2).astype(np.float32)
    data.append(dict(qi=qi,true=true,pr=pr,sr=sr,tp=tp,mid=mid,hw=hw,tau={k:s[tr[k-1]] for k in KS}))
print('queries',len(data))
def selmask(d,kind,p): return (d['pr']<p) if kind=='np' else (d['sr']<p)
def fit(ds,kind,p,k,eps):
    f={}
    for l in LV:
        r=[]
        for d in ds:
            m=selmask(d,kind,p); t=d['tp'][:k]; t=t[(t>=0)]; t=t[m[t]]
            r.append((d['true'][t]-d['mid'][l][t])/np.maximum(d['hw'][l][t],1e-9))
        r=np.concatenate(r); f[l]=max(float(np.quantile(r,1-eps)),0.0) if len(r) else 1.0
    return f
def run(ds,kind,p,k,f,seedtau=False):
    ent=np.zeros(len(LV)+1); rec=0; ceil=0
    for d in ds:
        m=selmask(d,kind,p); tau=d['tau'][k]
        if seedtau:
            first=d['true'][(d['pr']<4)&m] if kind=='np' else d['true'][(d['sr']<64)&m]
            tau=np.sort(first)[-k] if len(first)>=k else -9.0
        alive=m.copy()
        for i,l in enumerate(LV):
            ent[i]+=alive.sum(); alive&=(d['mid'][l]+f[l]*d['hw'][l]>=tau)
        ent[-1]+=alive.sum(); t=d['tp'][:k]; ok=t>=0
        rec+=(ok&alive[np.maximum(t,0)]).sum()/k; ceil+=(ok&m[np.maximum(t,0)]).sum()/k
    n=len(ds); return ent/n,rec/n,ceil/n
def cost(ent):
    det=ent[:5].sum()+ent[4]  # levels 0,1,2,3 then 4 and 5 both charged at level 5's entry count
    us=det*0.0078+(ent[5]+ent[6])*0.0313+ent[7]*0.109
    by=ent[0]*6.5+ent[1]*4.5+ent[2]*4.5+ent[3]*4.5+ent[4]*17+ent[5]*29.5+ent[6]*40.4+ent[7]*1536
    return us,by
ev=data[0::2]; od=data[1::2]
print('== cascade with a per-level factor on the certified half-width, fitted to lose eps of the true top-k per level (cross-fitted: fit on one half of the queries, run on the other; both halves averaged)')
print('   enter = items entering levels 0,1,2,3,5,6,7 and full vectors; us = per-item work only from the planner constants (7.8 ns det level, 31 ns rand level, 109 ns full vector); B = bytes read')
for kind,p in SEL:
    for k in (10,100,1000):
        for eps in (0.0,0.002,0.01,0.03):
            res=[]
            for a,b in ((ev,od),(od,ev)):
                f=fit(a,kind,p,k,eps) if eps>0 else {l:1.0 for l in LV}
                res.append(run(b,kind,p,k,f)); 
                if eps>0: res_seed=run(b,kind,p,k,f,True)
            ent=(res[0][0]+res[1][0])/2; rec=(res[0][1]+res[1][1])/2; ce=(res[0][2]+res[1][2])/2; us,by=cost(ent)
            fs=' '.join(f'{f[l]:.2f}' for l in LV) if eps>0 else 'certified'
            extra=f' | seed-tau (one half): rec {res_seed[1]:.3f} us {cost(res_seed[0])[0]:.0f}' if eps>0 else ''
            print(f'{kind}={p:4d} k={k:4d} eps={eps:5.3f}: recall {rec:.3f} (probe ceiling {ce:.3f}) enter {[int(x) for x in ent]} us {us:.0f} kB {by/1e3:.0f} | f [{fs}]{extra}',flush=True)
