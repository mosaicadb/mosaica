#!/bin/bash
# One quick-suite dataset end to end: truth check against the kept prep, fit (merged defaults, as the suite), the real
# index's row order, the extra hdf5 queries, orders2 on the suite's queries and on the extra ones.
# usage: run_ds.sh NAME HDF5NAME
set -e
S=${S:?set S to the scratch directory}; R=$S/replay; PY=$S/pyenv/bin/python
ds=$1; D=$R/data/prep/$ds; O=$S/suite-data/prep/$ds
for f in queries.npy query_ids.npy order.npy knn_thr.npy thresholds.npy order_random-1.npy order_random-25.npy vis_random-1.npy vis_random-25.npy learn_nb.npy; do
  [ -e $O/$f ] || [ -e $D/$f ] || continue
  cmp -s $D/$f $O/$f && echo "truth $f identical" || echo "truth $f DIFFERS"
done
mkdir -p $R/qsets/$ds $R/qsets/$ds-extra
cp $D/queries.npy $R/qsets/$ds/queries.npy; cp $D/order.npy $R/qsets/$ds/gt.npy
$PY -I $R/scripts/hdf5_extra.py $R/data/hdf5/$ds.hdf5 $D $R/qsets/$ds-extra
rm -f $R/data/hdf5/$ds.hdf5
extra=""; [ -e $D/learn.npy ] && extra="--query-sample $D/learn.npy"
env OPENBLAS_NUM_THREADS=4 $PY -I $R/scripts/fitwrap.py /home/user/mosaica/rust/tools/fit.py $D/corpus32.npy $R/fit-$ds --metric angular --basis dual $extra > $R/logs/fit-$ds.log 2>&1
tail -1 $R/logs/fit-$ds.log
$PY -I $R/scripts/build_rowent.py $R/fit-$ds $R/qsets/$ds/queries.npy $R/qsets/$ds/gt.npy $R/check_$ds.json 2>&1 | grep -v "^\["
$PY -I $R/scripts/orders2.py $R/fit-$ds $D/corpus32.npy $R/qsets/$ds/queries.npy $R/qsets/$ds/gt.npy $R/res2/$ds.json $ds > $R/logs/orders2-$ds.log 2>&1
$PY -I $R/scripts/orders2.py $R/fit-$ds $D/corpus32.npy $R/qsets/$ds-extra/queries.npy $R/qsets/$ds-extra/gt.npy $R/res2/$ds-extra.json "$ds-1000*" > $R/logs/orders2-$ds-extra.log 2>&1
grep -h "O0 \|faiss\|neighbour sim\|O0 f" $R/logs/orders2-$ds.log $R/logs/orders2-$ds-extra.log
