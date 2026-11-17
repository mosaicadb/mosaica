#!/bin/bash
# targets5.py on every dataset, one at a time (prep, fit, run, delete); the shared variant is sh64+own4 (alpha 0.5, w 0.3).
set -e
S=${S:?set S to the scratch directory}; R=$S/replay; PY=$S/pyenv/bin/python
SHV="sh64+own4 0.5 0.3"
cd /home/user/mosaica
for ds in glove-100-angular yahoo-minilm-384-normalized; do
  D=$R/data5b/prep/$ds
  $PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data5b --drop-hdf5 > $R/logs/prep7-$ds.log 2>&1
  cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/queries.npy $S/suite-data/prep/$ds/queries.npy && echo "$ds truth identical"
  env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit5b-$ds --metric angular --basis dual > $R/logs/fit5b-$ds.log 2>&1
  short=${ds%%-*}; ex=$([ $short = glove ] && echo "glove-1000*" || echo "yahoo-900*")
  $PY -I $R/scripts/targets5.py $R/fit5b-$ds $R/res5 $ds $SHV $short=$D/queries.npy:$D/order.npy "$ex=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/targets5-$ds.log 2>&1
  rm -rf $R/data5b $R/fit5b-$ds; df -h /tmp | tail -1
done
ds=laion-clip-512-normalized; D=$R/data5b/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data5b --no-f16 --drop-hdf5 > $R/logs/prep7-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit5b-$ds --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit5b-$ds.log 2>&1
$PY -I $R/scripts/targets5.py $R/fit5b-$ds $R/res5 $ds $SHV vibe-laion=$D/queries.npy:$D/order.npy "vibe-laion-learn*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/targets5-$ds.log 2>&1
rm -rf $R/data5b $R/fit5b-$ds; df -h /tmp | tail -1
$PY -I bench/suite/run.py scale-test --datasets laion-s1m --stages prep --data $R/data5b --download $S/suite-data/download/laion > $R/logs/prep7-laion.log 2>&1
rm -rf $R/data5b/prep/laion-s200k $R/data5b/prep/laion-s500k
D=$R/data5b/prep/laion-s1m
cmp $D/order.npy $S/suite-data/prep/laion-s1m/order.npy && echo "laion truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus16.npy $R/fit5b-laion --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit5b-laion.log 2>&1
$PY -I $R/scripts/targets5.py $R/fit5b-laion $R/res5 laion-s1m $SHV laion-text=$D/queries.npy:$D/order.npy laion-image=$R/qsets/laion-image/queries.npy:$R/qsets/laion-image/gt.npy "laion-text-learn*=$R/qsets/laion-learn/queries.npy:$R/qsets/laion-learn/gt.npy" > $R/logs/targets5-laion.log 2>&1
rm -rf $R/data5b $R/fit5b-laion; df -h /tmp | tail -1
echo ALL DONE
