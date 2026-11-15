# R1M timing (forward then reverse; verify in the first pass; k-NN in every process), then keep only perm/cstart/grp/
# meta of the R1M fits, then R3M 1,000 / 4,000 / 8,000 once each, indices kept for the second repeats.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
until grep -q PHASE1_DONE $D/sc/phase1.log; do sleep 5; done
export ITEMQ=$D/r_item_q.npy
for rep in 1 2; do
  order="1000 2000 4000 8000"; [ $rep = 2 ] && order="8000 4000 2000 1000"
  for s in $order; do
    v=""; [ $rep = 1 ] && v="verify --vitem 20 --vtext 20"
    echo "$(date +%T) start t_r1m_${s}_$rep"; BFRACS=0.0001,0.001,0.01,0.1,0.5,0.75 $D/scripts/srun_c.sh $D/sc/r1m_$s t_r1m_${s}_$rep $v; echo "$(date +%T) end t_r1m_${s}_$rep"
  done
done
for s in 1000 2000 4000 8000; do mkdir -p $D/sc/keep_r1m_$s; cp $D/sc/r1m_$s/{perm,cstart,grp,meta}.* $D/sc/keep_r1m_$s/ && rm -rf $D/sc/r1m_$s; done
echo R1M_DONE
KEEP=1 $D/scripts/stage_c.sh r3m 1 1000 4000 8000
echo PHASE23_DONE
