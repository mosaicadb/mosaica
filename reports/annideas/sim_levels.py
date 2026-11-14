import sys, os
sys.path.append('/home/joe/.local/lib/python3.10/site-packages')
import numpy as np
A='/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/annideas/'
W=A+'sim/'; DD=A+'dump/'
P='/home/joe/code/vexact-data/bioclip2-1m/prep/'; I='/home/joe/code/vexact-benchup-data/index_r4/'
S=np.load(W+'S.npy',mmap_mode='r'); N=S.shape[1]
perm=np.fromfile(DD+'row_ent.bin',dtype='<u4').astype(np.int64); cstart=np.load(I+'cstart.npy')
truth=np.load(P+'order.npy',mmap_mode='r')
Q=np.load(P+'queries.npy').astype(np.float32)
sub=np.load(W+'sub16.npy'); Csub=np.load(W+'Csub16.npy')
rowof=np.empty(len(perm),np.int64); rowof[perm]=np.arange(len(perm))
LV=[0,1,2,3,5,6,7]; BY=[6.5,11.0,15.5,20.0,28.5,37.0,66.5,106.9,176.8,275.9]
KS=[1,10,100,1000]; TG=[0.8,0.9,0.99]
qis=sorted(int(f.split('_')[1].split('.')[0]) for f in os.listdir(DD) if f.startswith('order_'))
dt=np.dtype([('r','<u4'),('lb','<f4'),('ub','<f4')])
NPS=[8,16,32,64,128,256]; SPS=[32,64,128,256,512,1024,2048]
# ranks[(level, sel)][k] -> list of arrays of ranks of the true top-k by midpoint inside the selection
ranks={}; items={}; errs={l:[] for l in LV}; hws={l:[] for l in LV}; viol={l:0 for l in LV}; tot=0
dialstat={}
two={}
for qi in qis:
    order=np.fromfile(DD+f'order_{qi}.bin',dtype='<u4'); crank=np.empty(len(order),np.int64); crank[order]=np.arange(len(order))
    s=np.asarray(S[qi]); tr=np.asarray(truth[qi,:1000]).astype(np.int64)
    subrank=np.empty(len(Csub),np.int64); subrank[np.argsort(-(Csub@Q[qi]))]=np.arange(len(Csub))
    mids={}
    for l in LV:
        a=np.fromfile(DD+f'iv_{qi}_{l}.bin',dtype=dt); ent=perm[a['r']]; mid=(a['lb'].astype(np.float64)+a['ub'])/2; hw=(a['ub'].astype(np.float64)-a['lb'])/2
        true=s[ent]; pr=crank[np.searchsorted(cstart,a['r'],side='right')-1]; sr=subrank[sub[ent]]
        pos=np.full(N,-1,np.int64); pos[ent]=np.arange(len(ent)); tp=pos[tr]
        errs[l].append(true-mid); hws[l].append(hw); viol[l]+=int(((true>a['ub']+2e-3)|(true<a['lb']-2e-3)).sum())
        mids[l]=(ent,mid,pos)
        sels=[('np',p,pr<p) for p in NPS]+[('sp',p,sr<p) for p in SPS]
        for kind,p,m in sels:
            c=np.cumsum(m); rk=np.where((tp>=0)&m[np.maximum(tp,0)],c[np.maximum(tp,0)]-1,10**9)
            ranks.setdefault((l,kind,p),[]).append(rk); items.setdefault((l,kind,p),[]).append(int(m.sum()))
        # dial-style pruning at the true k-th score: keep if mid + f*hw >= tau
        for k in KS:
            tau=s[tr[k-1]]
            for kind,p,m in (('np',64,pr<64),('np',128,pr<128),('sp',256,sr<256),('sp',1024,sr<1024)):
                for f in (0.0,0.05,0.1,0.15,0.2,0.3,0.5,1.0):
                    keep=m&(mid+f*hw>=tau)
                    hit=int(((tp[:k]>=0)&keep[np.maximum(tp[:k],0)]).sum())
                    dialstat.setdefault((l,kind,p,k,f),[]).append((hit/k,int(keep.sum())))
    # two-stage: level a selects R1 by midpoint (inside selection), level b ranks those; rank of true items by level-b mid among the R1
    for (kind,p) in (('np',64),('np',128),('sp',256),('sp',1024)):
        for la,lb_ in ((0,6),(1,6),(3,6),(1,7)):
            enta,mida,posa=mids[la]; entb,midb,posb=mids[lb_]
            ra=rowof
            pra=crank[np.searchsorted(cstart,ra[enta],side='right')-1]; sra=subrank[sub[enta]]
            m=(pra<p) if kind=='np' else (sra<p)
            sel_ent=enta[m]  # in descending level-a midpoint
            for R1 in (100,300,1000,3000,10000):
                e1=sel_ent[:R1]; mb=midb[posb[e1]]; ob=e1[np.argsort(-mb)]
                rr=np.full(N,10**9,np.int64); rr[ob]=np.arange(len(ob))
                two.setdefault((kind,p,la,lb_,R1),[]).append(rr[tr])
    tot+=1
print('queries dumped',tot,'(every 3rd of 150: images',sum(q<50 for q in qis),'text',sum(q>=50 for q in qis),')')
print('== 1. midpoint as an estimator: sd of (true - midpoint), mean certified half-width, their ratio; violations beyond 2e-3')
for l in LV:
    e=np.concatenate(errs[l]); h=np.concatenate(hws[l])
    print(f'level {l} ({BY[l]:5.1f} B): err sd {e.std():.4f}  mean hw {h.mean():.4f}  hw/sd {h.mean()/e.std():.1f}  viol {viol[l]}')
def needR(rks,k,tg):
    r=np.sort(np.concatenate([x[:k] for x in rks])); n=len(r); v=r[int(np.ceil(tg*n-1e-9))-1]
    return None if v>=10**9 else int(v)+1
def ceil_(rks,k): return np.mean(np.concatenate([x[:k] for x in rks])<10**9)
print('== 2. IVF + re-rank: R = full vectors read (best R by level midpoint) for mean recall >= target. "-" = the probe set itself is below target')
for kind,ps in (('np',NPS),('sp',SPS)):
    for p in ps:
        print(f'-- {"parent clusters probed" if kind=="np" else "x16 sub-clusters probed"} = {p}; mean items scanned {np.mean(items[(0,kind,p)]):.0f}; ceiling recall by k: '+' '.join(f'k{k}:{ceil_(ranks[(0,kind,p)],k):.3f}' for k in KS))
        for l in LV:
            row=[]
            for k in KS:
                row.append(f'k{k}: '+'/'.join(str(needR(ranks[(l,kind,p)],k,tg) or '-') for tg in TG))
            print(f'   level {l} ({BY[l]:5.1f} B)  R for 0.8/0.9/0.99  '+' | '.join(row))
print('== 3. two-stage: R1 by level-a midpoint, then level b on those, R2 full vectors for recall 0.8/0.9/0.99')
for key in sorted(two):
    kind,p,la,lb_,R1=key
    print(f'{kind}={p:5d} L{la}->L{lb_} R1={R1:6d}: '+' | '.join(f'k{k}: '+'/'.join(str(needR(two[key],k,tg) or '-') for tg in TG) for k in KS))
print('== 4. threshold form: keep items with midpoint + f*halfwidth >= true k-th score; recall of top-k / mean survivors')
for key in sorted(dialstat):
    l,kind,p,k,f=key
    if l in (0,1,3,6) :
        v=np.array(dialstat[key]); print(f'L{l} {kind}={p:5d} k={k:5d} f={f:.2f}: recall {v[:,0].mean():.3f} survivors {v[:,1].mean():.0f}')
