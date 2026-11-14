D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data; B=$D/bin; P=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/results/bounds/plan_fixed.txt
until grep -q TIMING_DONE $D/timing.log; do sleep 5; done
echo "$(date +%T) bd base"; $D/scripts/run.sh $B/vexact_b2 bd_base breakdown --nq 20 --bfracs 0.01,0.5 | head -2
echo "$(date +%T) bd c2a"; $D/scripts/run.sh $B/vexact_ndl2 bd_c2a --dlv 8,32 breakdown --nq 20 --bfracs 0.01,0.5 | head -2
v1m() { b=$1; t=$2; shift 2; echo "$(date +%T) verify 1M $t"; taskset -c 0-3 $B/$b --v2 --int --no-approx --plan-file $P --index $D/c1m --queries $D/text/text_q.npy --item-queries $D/c1m_item_q.npy --results $D/res/$t --gtab "$@" verify --vitem 20 --vtext 20 > $D/res/$t.out 2>&1; grep -E "^VERIFY2|panicked" $D/res/$t.out | cut -c1-260; }
v1m vexact_ndl2 v1m_c2a --dlv 8,32 --flags repack
v1m vexact_b2 v1m_ddec5 --flags repack,ddec=5
for x in "ndl3 c3_d02 --dlv 4,16,32 repack,ddec=0.2" "ndl3 c3_d2 --dlv 4,16,32 repack,ddec=2" "ndl2 c2a_d1 --dlv 8,32 repack,ddec=1" "ndl2 c2b_none --dlv 16,32 repack,ddec=none"; do
  set -- $x; echo "== 50k $2"; FLAGS=$5 $D/scripts/check.sh $B/vexact_$1 chk_$2 $3 $4
done
echo POST_DONE
