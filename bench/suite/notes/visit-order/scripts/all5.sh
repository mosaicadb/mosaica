#!/bin/bash
# orders5.py on every dataset, one at a time (glove's prep and fit from the first run are reused, then deleted).
set -e
S=${S:?set S to the scratch directory}; R=$S/replay; PY=$S/pyenv/bin/python
cd $R
ds=glove-100-angular; D=$R/data5/prep/$ds
$PY -I $R/scripts/orders5.py $R/fit5-$ds $D/corpus32.npy $R/res5 $ds glove=$D/queries.npy:$D/order.npy "glove-1000*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders5-$ds.log 2>&1
rm -rf $R/data5 $R/fit5-$ds; df -h /tmp | tail -1
cd /home/user/mosaica
ds=yahoo-minilm-384-normalized; D=$R/data5/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data5 --drop-hdf5 > $R/logs/prep6-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/queries.npy $S/suite-data/prep/$ds/queries.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit5-$ds --metric angular --basis dual > $R/logs/fit5-$ds.log 2>&1
$PY -I $R/scripts/orders5.py $R/fit5-$ds $D/corpus32.npy $R/res5 $ds yahoo=$D/queries.npy:$D/order.npy "yahoo-900*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders5-$ds.log 2>&1
rm -rf $R/data5 $R/fit5-$ds; df -h /tmp | tail -1
ds=laion-clip-512-normalized; D=$R/data5/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data5 --no-f16 --drop-hdf5 > $R/logs/prep6-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/learn_nb.npy $S/suite-data/prep/$ds/learn_nb.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit5-$ds --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit5-$ds.log 2>&1
$PY -I $R/scripts/orders5.py $R/fit5-$ds $D/corpus32.npy $R/res5 $ds vibe-laion=$D/queries.npy:$D/order.npy "vibe-laion-learn*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders5-$ds.log 2>&1
rm -rf $R/data5 $R/fit5-$ds; df -h /tmp | tail -1
$PY -I bench/suite/run.py scale-test --datasets laion-s1m --stages prep --data $R/data5 --download $S/suite-data/download/laion > $R/logs/prep6-laion.log 2>&1
rm -rf $R/data5/prep/laion-s200k $R/data5/prep/laion-s500k
D=$R/data5/prep/laion-s1m
cmp $D/order.npy $S/suite-data/prep/laion-s1m/order.npy && cmp $D/queries.npy $S/suite-data/prep/laion-s1m/queries.npy && echo "laion truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus16.npy $R/fit5-laion --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit5-laion.log 2>&1
$PY -I $R/scripts/orders5.py $R/fit5-laion $D/corpus16.npy $R/res5 laion-s1m laion-text=$D/queries.npy:$D/order.npy laion-image=$R/qsets/laion-image/queries.npy:$R/qsets/laion-image/gt.npy "laion-text-learn*=$R/qsets/laion-learn/queries.npy:$R/qsets/laion-learn/gt.npy" > $R/logs/orders5-laion.log 2>&1
rm -rf $R/data5 $R/fit5-laion; df -h /tmp | tail -1
echo ALL DONE
