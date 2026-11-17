#!/bin/bash
# Re-runs every dataset (one at a time, deleted after) with the current orders2.py: LAION 1M (text, image, learn text),
# glove, yahoo, VIBE laion.
set -e
S=${S:?set S to the scratch directory}; R=$S/replay; PY=$S/pyenv/bin/python
cd /home/user/mosaica
# LAION 1M
$PY -I bench/suite/run.py scale-test --datasets laion-s1m --stages prep --data $R/data --download $S/suite-data/download/laion > $R/logs/prep3-laion.log 2>&1
rm -rf $R/data/prep/laion-s200k $R/data/prep/laion-s500k
D=$R/data/prep/laion-s1m
cmp $D/order.npy $S/suite-data/prep/laion-s1m/order.npy && cmp $D/queries.npy $S/suite-data/prep/laion-s1m/queries.npy && echo "laion truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus16.npy $R/fit-laion --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit-laion.log 2>&1
$PY -I $R/scripts/build_rowent.py $R/fit-laion $R/qsets/laion-text/queries.npy $R/qsets/laion-text/gt.npy $R/check_laion-text.json 2>&1 | grep -v "^\["
for q in laion-text laion-image laion-learn; do nm=$q; [ $q = laion-learn ] && nm="laion-text-learn*"
  $PY -I $R/scripts/orders2.py $R/fit-laion $D/corpus16.npy $R/qsets/$q/queries.npy $R/qsets/$q/gt.npy $R/res2/$q.json "$nm" > $R/logs/orders2-$q.log 2>&1; done
rm -rf $R/data/prep/laion-s1m $R/fit-laion; df -h /tmp | tail -1
# glove, yahoo
for ds in glove-100-angular yahoo-minilm-384-normalized; do
  $PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data > $R/logs/prep3-$ds.log 2>&1
  $R/scripts/run_ds.sh $ds | grep -c identical
  rm -rf $R/data/prep/$ds $R/fit-$ds; df -h /tmp | tail -1
done
# VIBE laion
ds=laion-clip-512-normalized; D=$R/data/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data --no-f16 --drop-hdf5 > $R/logs/prep3-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/learn_nb.npy $S/suite-data/prep/$ds/learn_nb.npy && echo "vibe laion truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit-$ds --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit-$ds.log 2>&1
$PY -I $R/scripts/build_rowent.py $R/fit-$ds $R/qsets/$ds/queries.npy $R/qsets/$ds/gt.npy $R/check_$ds.json 2>&1 | grep -v "^\["
$PY -I $R/scripts/orders2.py $R/fit-$ds $D/corpus32.npy $R/qsets/$ds/queries.npy $R/qsets/$ds/gt.npy $R/res2/$ds.json vibe-laion-text > $R/logs/orders2-$ds.log 2>&1
$PY -I $R/scripts/orders2.py $R/fit-$ds $D/corpus32.npy $R/qsets/$ds-extra/queries.npy $R/qsets/$ds-extra/gt.npy $R/res2/$ds-extra.json "vibe-laion-learn*" > $R/logs/orders2-$ds-extra.log 2>&1
rm -rf $R/data $R/fit-$ds; df -h /tmp | tail -1
echo ALL DONE
