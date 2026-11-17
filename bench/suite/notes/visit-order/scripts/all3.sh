#!/bin/bash
# orders3.py on yahoo, VIBE laion and LAION 1M (text, images, learn text), one dataset at a time, deleted after.
set -e
S=${S:?set S to the scratch directory}; R=$S/replay; PY=$S/pyenv/bin/python
cd /home/user/mosaica
ds=yahoo-minilm-384-normalized; D=$R/data/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data --drop-hdf5 > $R/logs/prep4-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/queries.npy $S/suite-data/prep/$ds/queries.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit-$ds --metric angular --basis dual > $R/logs/fit3-$ds.log 2>&1
$PY -I $R/scripts/orders3.py $R/fit-$ds $D/corpus32.npy $R/res3 $ds yahoo=$D/queries.npy:$D/order.npy "yahoo-900*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders3-$ds.log 2>&1
rm -rf $R/data $R/fit-$ds; df -h /tmp | tail -1
ds=laion-clip-512-normalized; D=$R/data/prep/$ds
$PY -I bench/suite/run.py quick --datasets $ds --stages prep --data $R/data --no-f16 --drop-hdf5 > $R/logs/prep4-$ds.log 2>&1
cmp $D/order.npy $S/suite-data/prep/$ds/order.npy && cmp $D/learn_nb.npy $S/suite-data/prep/$ds/learn_nb.npy && echo "$ds truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus32.npy $R/fit-$ds --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit3-$ds.log 2>&1
$PY -I $R/scripts/orders3.py $R/fit-$ds $D/corpus32.npy $R/res3 $ds vibe-laion=$D/queries.npy:$D/order.npy "vibe-laion-learn*=$R/qsets/$ds-extra/queries.npy:$R/qsets/$ds-extra/gt.npy" > $R/logs/orders3-$ds.log 2>&1
rm -rf $R/data $R/fit-$ds; df -h /tmp | tail -1
$PY -I bench/suite/run.py scale-test --datasets laion-s1m --stages prep --data $R/data --download $S/suite-data/download/laion > $R/logs/prep4-laion.log 2>&1
rm -rf $R/data/prep/laion-s200k $R/data/prep/laion-s500k
D=$R/data/prep/laion-s1m
cmp $D/order.npy $S/suite-data/prep/laion-s1m/order.npy && cmp $D/queries.npy $S/suite-data/prep/laion-s1m/queries.npy && echo "laion truth identical"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py rust/tools/fit.py $D/corpus16.npy $R/fit-laion --metric angular --basis dual --query-sample $D/learn.npy > $R/logs/fit3-laion.log 2>&1
$PY -I $R/scripts/orders3.py $R/fit-laion $D/corpus16.npy $R/res3 laion-s1m laion-text=$D/queries.npy:$D/order.npy laion-image=$R/qsets/laion-image/queries.npy:$R/qsets/laion-image/gt.npy "laion-text-learn*=$R/qsets/laion-learn/queries.npy:$R/qsets/laion-learn/gt.npy" > $R/logs/orders3-laion.log 2>&1
rm -rf $R/data $R/fit-laion; df -h /tmp | tail -1
echo ALL DONE
