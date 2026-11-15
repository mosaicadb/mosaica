# Stages A (1M) and B (500k): each fit in its own process, forward then reverse; verify in the first pass.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
until grep -q FITS_AB_DONE $D/sc/fits_ab.log && grep -q "^done" $D/sc/fetch5m.log; do sleep 5; done
idx() { case $1 in m1_1000) echo $D/c1m ;; *) echo $D/sc/$1 ;; esac; }
for stage in "m1_1000 m1_2000 m1_4000 m1_8000" "k500_1000 k500_2000 k500_4000 k500_8000"; do
  for rep in 1 2; do
    order=$stage; [ $rep = 2 ] && order=$(echo $stage | tr ' ' '\n' | tac | tr '\n' ' ')
    for f in $order; do
      v=""; [ $rep = 1 ] && v="verify --vitem 20 --vtext 20"
      echo "$(date +%T) start t_${f}_$rep"; $D/scripts/srun.sh $(idx $f) t_${f}_$rep $v; echo "$(date +%T) end t_${f}_$rep"
    done
  done
done
echo STAGE_AB_DONE
