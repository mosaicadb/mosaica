#!/bin/bash
# R3M at 2,000 items per cluster, with 4,000 refitted and timed again in the same session as a control (the container
# restarted since the earlier R3M runs). Order: 2,000 (verify first), 4,000, 4,000, 2,000. Then check the 4,000 refit
# is identical to the earlier one, delete the indices, and count the approximate-mode probes for R3M 2,000, alone.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
export ITEMQ=$D/r_item_q.npy
EXTRA="verify --vitem 20 --vtext 20" KEEP=1 $D/scripts/stage_c.sh r3m 1 2000
KEEP=1 $D/scripts/stage_c.sh r3m 3 4000
KEEP=1 $D/scripts/stage_c.sh r3m 4 4000
KEEP=1 $D/scripts/stage_c.sh r3m 2 2000
for f in perm.npy cstart.npy grp.npy; do cmp $D/sc/r3m_4000/$f $D/sc/keep_r3m_4000/$f && echo "refit 4000 $f identical"; done
rm -rf $D/sc/r3m_2000 $D/sc/r3m_4000
echo TIMING_DONE
PROBE_ROWS=$D/r3m_rows.npy PROBE_ITEMQ=$D/r_item_q.npy python3 -I $D/scripts/probe.py $D/c5m_x.npy $D/sc/keep_r3m_2000 $D/sc/truth_r3m.npy r3m_2000 $D/sc/probe 2>&1 | tail -1
echo PHASE6_DONE
