# The cluster-size sweep: each fit in its own process (verify in the first pass only, before any timing), end-to-end
# timing at six answer sizes and the profiled breakdown; all fits in order, then in reverse order.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data; B=$D/bin/vexact_cl
until grep -q FITS_DONE $D/cl/fits.log; do sleep 5; done
idx() { case $1 in base) echo $D/c1m ;; *) echo $D/cl/$1 ;; esac; }
for rep in 1 2; do
  order="base p4 p16 d4 d16"; [ $rep = 2 ] && order="d16 d4 p16 p4 base"
  for f in $order; do
    [ -d $(idx $f) ] || { echo "skip $f"; continue; }
    v=""; [ $rep = 1 ] && v="verify --vitem 20 --vtext 20"
    echo "$(date +%T) start s_${f}_$rep"
    IDX=$(idx $f) $D/scripts/run.sh $B s_${f}_$rep $v e2e --nq 20 --runs 3 --knn 0 --sizes 0.0001,0.001,0.01,0.1,0.5,0.75 breakdown --bfracs 0.0001,0.01,0.5 | head -1
    grep -E "^VERIFY2 40|^VERIFY2 checker" $D/res/s_${f}_$rep.out | cut -c1-200
    echo "$(date +%T) end s_${f}_$rep"
  done
done
echo SWEEP_DONE
