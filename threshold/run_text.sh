#!/bin/bash
# Text-query rerun of one query set: thresholds (text_split.py), the unchanged adapt / adapt_gm / lb_run / flat / tables
# steps of the item run, on the item run's data-only fit. Two processes at a time on cores 8-9, one thread each.
# usage: run_text.sh ITEMWORK SETNPY OUTWORK LOGDIR [STAGES]   (stages: any of prep lb gm flat tables; default all)
set -e; H=$(cd "$(dirname "$0")" && pwd); PY=$H/../probe/.venv/bin/python; IW=$1; SET=$2; W=$3; L=$4; ST=" ${5:-prep lb gm flat tables} "
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 WORK=$W
mkdir -p $W $L; ln -sfn $IW/corpus.npy $W/corpus.npy; ln -sfn $IW/fit $W/fit
NQ=$(taskset -c 8,9 $PY -I -c "import numpy; print(len(numpy.load('$SET')))")
if [[ $ST == *" prep "* ]]; then
taskset -c 8,9 $PY -I $H/text_split.py $W $SET > $L/split.log
taskset -c 8,9 $PY -I $H/adapt.py $W > $L/adapt.log
taskset -c 8,9 $PY -I $H/adapt_gm.py $W > $L/adapt_gm.log; fi
[[ $ST == *" lb "* ]] && for w in 0 1 2 3 4 5 6 7 8; do echo "$w"; done | xargs -P 2 -I{} sh -c "taskset -c 8,9 $PY -I $H/lb_run.py bio {} 9 > $L/lb_w{}.log 2>&1"
[[ $ST == *" gm "* ]] && for w in 0 1 2 3 4 5 6 7 8; do echo "$w"; done | xargs -P 2 -I{} sh -c "TAG=gm IDX=index_gm.npz taskset -c 8,9 $PY -I $H/lb_run.py bio {} 9 > $L/gm_w{}.log 2>&1"
[[ $ST == *" flat "* ]] && OMP_NUM_THREADS=2 taskset -c 8,9 $PY -I $H/flat.py s256,s512,1,2,4,6 $NQ > $L/flat.log 2>&1
[[ $ST == *" tables "* ]] && taskset -c 8,9 $PY -I $H/tables.py $W $NQ > $L/tables.md
echo "done$ST$SET NQ=$NQ"
