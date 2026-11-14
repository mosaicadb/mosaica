# Do the two gains stack: the NDL 2 binary with --dlv 16,32 on the baseline fit and on P4 (base, p4, p4, base).
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data; B=$D/bin/vexact_ndl2
until grep -q SWEEP_DONE $D/cl/sweep.log; do sleep 5; done
for x in "base $D/c1m 1" "p4 $D/cl/p4 1" "p4 $D/cl/p4 2" "base $D/c1m 2"; do set -- $x
  echo "$(date +%T) start k_$1_$3"; IDX=$2 $D/scripts/run.sh $B k_$1_$3 --dlv 16,32 e2e --nq 20 --runs 3 --knn 0 --sizes 0.0001,0.001,0.01,0.1,0.5,0.75 | head -1
done
echo STACK_DONE
