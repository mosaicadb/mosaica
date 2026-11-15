# Stage C, fit by fit at the large N: fit, record its statistics, time once (with EXTRA commands first, e.g. verify),
# then delete the index unless KEEP is set. usage: stage_c.sh NN REP SIZE...  (NN: m5 for every row of c5m_x.npy;
# otherwise the row subset NN_rows.npy, e.g. r3m for r3m_rows.npy)
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
F=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/tools/fit.py
nn=$1; rep=$2; shift 2
rows=""; [ $nn != m5 ] && rows="--rows $D/${nn}_rows.npy"
for s in "$@"; do
  df -h /tmp | tail -1; free -g | sed -n 2p
  avail=$(df --output=avail -k /tmp | tail -1); [ $avail -lt 5500000 ] && { echo "not enough disk for the index ($avail kB)"; exit 1; }
  if [ ! -f $D/sc/${nn}_$s/meta.json ]; then
      echo "$(date +%T) fit ${nn}_$s"
    python3 -I $D/scripts/peakrss.py python3 -I $F $D/c5m_x.npy $D/sc/${nn}_$s $rows --basis dual --per-cluster $s --groups 64 > $D/sc/fit_${nn}_${s}_$rep.log 2>&1 || { echo "fit failed"; exit 1; }
    cp $D/sc/fit_${nn}_${s}_$rep.log $D/sc/fit_${nn}_$s.log
    python3 -I $D/scripts/fitmeta.py $D/sc/${nn}_$s ${nn}_$s $D/sc
  fi
  mkdir -p $D/sc/keep_${nn}_$s; cp $D/sc/${nn}_$s/{perm,cstart,grp,meta}.* $D/sc/keep_${nn}_$s/ 2>/dev/null
  echo "$(date +%T) start t_${nn}_${s}_$rep"; $D/scripts/srun_c.sh $D/sc/${nn}_$s t_${nn}_${s}_$rep $EXTRA; echo "$(date +%T) end t_${nn}_${s}_$rep"
  [ -n "$KEEP" ] || rm -rf $D/sc/${nn}_$s
done
echo STAGE_C_DONE $nn $rep "$@"
