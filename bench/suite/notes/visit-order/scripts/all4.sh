#!/bin/bash
# orders4.py on every dataset, one at a time (glove's prep and fit from the first run are reused, then deleted).
set -e
S=${S:?set S to the scratch directory}; R=$S/replay; PY=$S/pyenv/bin/python
cd $R
ds=glove-100-angular; D=$R/data4/prep/$ds
$PY -I $R/scripts/orders4.py $R/fit4-$ds $D/corpus32.npy $R/res4 $ds glove=$D/queries.npy:$D/order.npy "glove-1000*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders4-$ds.log 2>&1
rm -rf $R/data4 $R/fit4-$ds; df -h /tmp | tail -1
cd /home/user/mosaica
ds=yahoo-minilm-384-normalized; D=$R/data4/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data4 --drop-hdf5 > $R/logs/prep5-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/queries.npy $S/suite-data/prep/$ds/queries.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit4-$ds --metric angular --basis dual > $R/logs/fit4-$ds.log 2>&1
$PY -I $R/scripts/orders4.py $R/fit4-$ds $D/corpus32.npy $R/res4 $ds yahoo=$D/queries.npy:$D/order.npy "yahoo-900*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders4-$ds.log 2>&1
rm -rf $R/data4 $R/fit4-$ds; df -h /tmp | tail -1
ds=laion-clip-512-normalized; D=$R/data4/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data4 --no-f16 --drop-hdf5 > $R/logs/prep5-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/learn_nb.npy $S/suite-data/prep/$ds/learn_nb.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit4-$ds --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit4-$ds.log 2>&1
$PY -I $R/scripts/orders4.py $R/fit4-$ds $D/corpus32.npy $R/res4 $ds vibe-laion=$D/queries.npy:$D/order.npy "vibe-laion-learn*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders4-$ds.log 2>&1
rm -rf $R/data4 $R/fit4-$ds; df -h /tmp | tail -1
$PY -I bench/suite/run.py scale-test --datasets laion-s1m --stages prep --data $R/data4 --download $S/suite-data/download/laion > $R/logs/prep5-laion.log 2>&1
rm -rf $R/data4/prep/laion-s200k $R/data4/prep/laion-s500k
D=$R/data4/prep/laion-s1m
cmp $D/order.npy $S/suite-data/prep/laion-s1m/order.npy && cmp $D/queries.npy $S/suite-data/prep/laion-s1m/queries.npy && echo "laion truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus16.npy $R/fit4-laion --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit4-laion.log 2>&1
$PY -I $R/scripts/orders4.py $R/fit4-laion $D/corpus16.npy $R/res4 laion-s1m laion-text=$D/queries.npy:$D/order.npy laion-image=$R/qsets/laion-image/queries.npy:$R/qsets/laion-image/gt.npy "laion-text-learn*=$R/qsets/laion-learn/queries.npy:$R/qsets/laion-learn/gt.npy" > $R/logs/orders4-laion.log 2>&1
rm -rf $R/data4 $R/fit4-laion; df -h /tmp | tail -1
echo ALL DONE
