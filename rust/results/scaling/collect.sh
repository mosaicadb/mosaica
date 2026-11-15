# Collect the scaling run into rust/results/scaling/ and write the full working-tree diff.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
W=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/results/scaling
mkdir -p $W/runs $W/fits $W/probe
cp $D/scripts/{samples.py,rsamples.py,fetch5m.py,subsets.py,thresholds.py,fitmeta.py,peakrss.py,srun.sh,srun_c.sh,srun_knn.sh,stage_ab.sh,stage_c.sh,post_ab.sh,phase1.sh,phase23.sh,phase4.sh,probe.py,probe_tables.py,cmp_sc.py,watchdog.sh,memwatch.sh,fits_ab.sh,collect.sh,phase6.sh,cmp_r3m_session.py} $W/ 2>/dev/null
for d in $D/res/t_k500_* $D/res/t_m1_* $D/res/t_r1m_* $D/res/t_r3m_* $D/res/knn_*; do
  [ -d $d ] || continue; t=$(basename $d)
  cp $d/e2e2.csv $W/runs/e2e_$t.csv 2>/dev/null; cp $d/breakdown2.csv $W/runs/breakdown_$t.csv 2>/dev/null
  grep -E "^breakdown2 (counts|tables)|^VERIFY2|peak RSS" $d.out > $W/runs/$t.summary.txt
done
cp $D/sc/meta_*.json $D/sc/fit_*.log $W/fits/ 2>/dev/null; cp $D/fit1m.log $W/fits/fit_m1_1000.log
cp $D/sc/probe/*.json $W/probe/; cp $D/sc/thresholds*.log $D/sc/thresholds*.json $D/sc/fetch5m.log $D/sc/model.json $D/sc/watchdog.log $W/ 2>/dev/null
for f in stage_ab post_ab phase1 phase23 phase4 phase6 fits_ab; do cp $D/sc/$f.log $W/log_$f.txt 2>/dev/null; done
awk '$2>100000{a[$1]=($2>a[$1]?$2:a[$1]); f[$1]=($3>f[$1]?$3:f[$1])} END{for(k in a) printf "%s anon %.2f file %.2f GiB\n", k, a[k]/1048576, f[k]/1048576}' $D/sc/memwatch.raw $D/sc/memwatch2.raw | sort > $W/memory_peaks.txt
python3 -I $D/scripts/cmp_sc.py $D > $W/tables.md 2>&1
python3 -I $D/scripts/cmp_r3m_session.py $D >> $W/tables.md; echo >> $W/tables.md; echo "# Probe counts" >> $W/tables.md; python3 -I $D/scripts/probe_tables.py $D >> $W/tables.md
cd /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv && git add -N rust/results/scaling && git diff > /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/scaling-full.patch
du -sh $W; ls $W | wc -l
