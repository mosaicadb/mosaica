import pandas as pd, glob, os
N=999000
pd.set_option('display.width',250)
cnt=pd.concat([pd.read_csv(f'results/count_{c}.csv') for c in ['item','text']])
for f in sorted(glob.glob('results/time*_*.csv')):
    d=pd.read_csv(f).drop(columns=['lines','runs','pairs'])
    if d.empty: continue
    d=d.merge(cnt[['class','layout','frac','vis','query','lines','pairs']],on=['class','layout','frac','vis','query'])
    d['ms_best']=d[['ms','ms_pf']].min(axis=1)
    g=d.groupby(['class','vis','frac','layout']).agg(ms=('ms_best','median'),lines=('lines','median'),touched=('touched','median')).reset_index()
    g['GBs']=g.lines*64/(g.ms*1e-3)/1e9
    g['ns_touched']=g.ms*1e6/g.touched
    print('=====',os.path.basename(f),': median over queries of ms per query (best of plain / software-prefetched replay; each the median of 5 runs, L3 flushed)')
    for (c,v),x in g.groupby(['class','vis']):
        print(f'-- {c}, visible {v}: ms per query  |  effective GB/s (distinct lines x 64 / time)  |  ns per touched item')
        a=x.pivot(index='layout',columns='frac',values='ms'); b=x.pivot(index='layout',columns='frac',values='GBs'); e=x.pivot(index='layout',columns='frac',values='ns_touched')
        print(pd.concat([a.round(2),b.round(1),e.round(1)],axis=1,keys=['ms','GB/s','ns/touched']).to_string())
