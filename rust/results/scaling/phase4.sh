# After R3M 1,000 / 4,000 / 8,000 once each: the best coarse size is the one of 4,000 and 8,000 with the lower geometric
# mean time over all twelve threshold cells (item and text). Second repeats: the best size (verify first), then
# 1,000, on the kept indices; then delete the R3M indices; then the probe counts of the R1M and R3M fits, alone.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
until grep -q PHASE23_DONE $D/sc/phase23.log; do sleep 5; done
best=$(python3 -I -c "
import csv, math
def g(s): v=[float(r['median_of_runs_ms']) for r in csv.DictReader(open('$D/res/t_r3m_%d_1/e2e2.csv' % s)) if r['what']=='index2_thr' and r['threads']=='1']; return math.exp(sum(map(math.log, v))/len(v))
print(min((g(s), s) for s in (4000, 8000))[1])")
echo "best coarse size at R3M: $best"; rm -rf $D/sc/r3m_$( [ $best = 4000 ] && echo 8000 || echo 4000 )
export ITEMQ=$D/r_item_q.npy
EXTRA="verify --vitem 20 --vtext 20" KEEP=1 $D/scripts/stage_c.sh r3m 2 $best
KEEP=1 $D/scripts/stage_c.sh r3m 2 1000
rm -rf $D/sc/r3m_1000 $D/sc/r3m_$best
echo TIMING_DONE
p() { PROBE_ROWS=$1 PROBE_ITEMQ=$D/r_item_q.npy bash -c "[ -f $3 ] || python3 -I $D/scripts/probe.py --truth $D/c5m_x.npy $3; python3 -I $D/scripts/probe.py $D/c5m_x.npy $2 $3 $4 $D/sc/probe 2>&1 | tail -1"; }
for s in 1000 4000 8000 2000; do p $D/r1m_rows.npy $D/sc/keep_r1m_$s $D/sc/truth_r1m.npy r1m_$s; done
for s in 1000 $best; do p $D/r3m_rows.npy $D/sc/keep_r3m_$s $D/sc/truth_r3m.npy r3m_$s; done
echo PHASE4_DONE
