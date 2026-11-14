# Markdown tables for REPORT_LINES.md from the count and time results.
import pandas as pd
N=999000
cnt=pd.concat([pd.read_csv(f'results/count_{c}.csv') for c in ['item','text']])
g=cnt.groupby(['class','vis','frac','layout'])[['lines','pairs','bytes','full']].mean().reset_index()
lay=['rowtight','row64','row128','col32','col64','hyb:0:64:43','hyb:1:64:285','hyb:4:64:285','hyb:8:64:285','hyb:16:64:285','hyb:8:128:285','hyb:8:64:84']
names={'rowtight':'(a) rows, tight, growing levels','row64':'(b) rows, 64 B steps','row128':'rows, 128 B steps','col32':'col32 (as built)','col64':'(c) col64, all columnar',
 'hyb:0:64:43':'(d) fixed split: col to 43 B, then rows/64','hyb:1:64:285':'(d) switch at <=1 of 64','hyb:4:64:285':'(d) switch at <=4 of 64','hyb:8:64:285':'(d) switch at <=8 of 64',
 'hyb:16:64:285':'(d) switch at <=16 of 64','hyb:8:128:285':'(d) switch at <=8, rows/128','hyb:8:64:84':'(d) switch at <=8, col only to 84 B'}
fr=[0.0001,0.001,0.01,0.1,0.5]
hdr='| Layout | 0.01% | 0.1% | 1% | 10% | 50% |\n|---|---|---|---|---|---|'
def tab(c,v,val,fmt):
    out=[hdr]
    x=g[(g['class']==c)&(g.vis==v)]
    d=x[x.layout=='rowtight'].set_index('frac')
    out.append('| *stream bytes needed* | '+' | '.join(f'{d.loc[f,"bytes"]/N:.1f}' for f in fr)+' |')
    for l in lay:
        y=x[x.layout==l].set_index('frac')
        out.append(f'| {names[l]} | '+' | '.join(fmt(y.loc[f]) for f in fr)+' |')
    out.append('| *full vectors (all layouts)* | '+' | '.join(f'{d.loc[f,"full"]*1536/N:.1f}' for f in fr)+' |')
    return '\n'.join(out)
for c in ['text','item']:
    for v in ['all','10%']:
        print(f'\n**{c} queries, visible {v}** — line bytes per item of N: distinct 64 B lines x 64 / N (in brackets: distinct 128 B pairs x 128 / N)\n')
        print(tab(c,v,None,lambda r:f'{r.lines*64/N:.1f} ({r.pairs*128/N:.1f})'))
def tt(f):
    d=pd.read_csv(f).drop(columns=['lines','runs','pairs'])
    d['ms']=d[['ms','ms_pf']].min(axis=1)
    return d.groupby(['class','vis','frac','layout']).ms.median()
t1=pd.concat([tt('results/time1_item.csv'),tt('results/time1_text.csv')])
t50=pd.concat([tt('results/time50_item.csv'),tt('results/time50_text.csv')])
lay2=['rowtight','row64','row128','col32','col64','hyb:0:64:43','hyb:4:64:285','hyb:8:64:285','hyb:16:64:285','hyb:8:128:285','hyb:8:64:84']
for c in ['text','item']:
    for v in ['all','10%']:
        print(f'\n**{c} queries, visible {v}** — ms per query at 1M (at 50M-scale placement in brackets); median of 10 queries\n')
        print(hdr)
        for l in lay2:
            print(f'| {names[l]} | '+' | '.join(f'{t1[(c,v,f,l)]:.2f} ({t50[(c,v,f,l)]:.2f})' for f in fr)+' |')
