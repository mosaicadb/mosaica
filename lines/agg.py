import pandas as pd, sys
N=999000
d=pd.concat([pd.read_csv(f'results/count_{c}.csv') for c in ['item','text']])
g=d.groupby(['class','vis','frac','layout'])[['lines','runs','pairs','bytes','full','touched']].mean().reset_index()
# per item of the index (all rows): bytes of line traffic per item
g['L']=g.lines*64/N; g['R']=g.runs/N*1000; g['P']=g.pairs*128/N
pd.set_option('display.width',250); pd.set_option('display.max_rows',2000)
order=['rowtight','row64','row128','col64','col32','hyb:0:64:22','hyb:0:64:43','hyb:0:128:43','hyb:1:64:285','hyb:2:64:285','hyb:4:64:285','hyb:8:64:285','hyb:16:64:285','hyb:32:64:285','hyb:4:128:285','hyb:8:128:285','hyb:16:128:285','hyb:8:64:84','hyb:16:64:84','hyb:8:64:43','hyb:16:64:43']
for what,lab in [('L','line bytes per item (lines x 64 / N)'),('P','pair bytes per item (128-B pairs x 128 / N)'),('R','runs (non-sequential starts) per 1000 items')]:
    print('==',lab)
    for (c,v),x in g.groupby(['class','vis']):
        t=x.pivot(index='layout',columns='frac',values=what).loc[order]
        print(f'-- {c}, visible {v}'); print(t.round(2).to_string())
# demand bytes (bytes of stream needed, rowtight) and full reads
print('== demand bytes per item (stream bytes needed, before line rounding), touched share, full-read share')
x=g[g.layout=='rowtight'].copy(); x['need']=x.bytes/N; x['touched']/=N; x['full']/=N
print(x.pivot_table(index=['class','vis'],columns='frac',values=['need','touched','full']).round(4).to_string())
