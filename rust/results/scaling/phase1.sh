# R1M fits two at a time; alongside them the remaining probe jobs of the row-group fits (1M and 500k) and then the
# population threshold check. Nothing is timed meanwhile.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data; F=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/tools/fit.py
f() { python3 -I $D/scripts/peakrss.py python3 -I $F $D/c5m_x.npy $D/sc/r1m_$1 --rows $D/r1m_rows.npy --basis dual --per-cluster $1 --groups 64 > $D/sc/fit_r1m_$1.log 2>&1; python3 -I $D/scripts/fitmeta.py $D/sc/r1m_$1 r1m_$1 $D/sc; }
( f 1000 & f 2000; wait; f 4000 & f 8000; wait; echo R1M_FITS_DONE ) &
( p() { [ -f $3 ] || python3 -I $D/scripts/probe.py --truth $1 $3; python3 -I $D/scripts/probe.py $1 $2 $3 $4 $D/sc/probe 2>&1 | tail -1; }
  p $D/c1m_x.npy $D/sc/keep_m1_8000 $D/sc/truth_m1.npy m1_8000
  for s in 1000 2000 4000 8000; do p $D/c500k_x.npy $D/sc/keep_k500_$s $D/sc/truth_k500.npy k500_$s; done
  python3 -I $D/scripts/thresholds.py $D r > $D/sc/thresholds_r.log 2>&1
  echo PROBES1_DONE ) &
wait
echo PHASE1_DONE
