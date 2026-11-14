# Summarise breakdown2 CSVs: per class, answer, threads: wall and the grouped phases (ms of CPU).
import csv, sys
for f in sys.argv[1:]:
    print(f)
    for r in csv.DictReader(open(f)):
        g = lambda *k: sum(float(r[x]) for x in k)
        print(f"  {r['class']} {r['answer_frac']} t{r['threads']}: wall {float(r['wall_ms']):.2f}  cpu {float(r['cpu_ms_total']):.2f} | tables {g('tables'):.2f} setup {g('anchor','cut','basis','rotations','cluster setup','candidate init'):.2f} (cut {g('anchor','cut'):.2f}) decode+decisions {g('level decode','bounds+decisions+compaction'):.2f} re-pack {g('re-pack'):.2f} full {g('full vectors'):.2f} other {float(r['wall_ms']) - float(r['cpu_ms_total']) + g('result bitmap'):.2f} | B/item {float(r['bytes_read']):.2f} wrong {r['wrong']}")
