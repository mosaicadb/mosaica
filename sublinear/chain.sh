#!/bin/bash
# runs after the ladder finishes: tree, tail, lsf (one thread each, in sequence)
V=/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc
cd $V/sublinear
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
while pgrep -f "ladder.py" > /dev/null; do sleep 3; done
$V/venv/bin/python lfit.py > lfit.txt 2>&1
$V/venv/bin/python tree.py > tree.log 2>&1
$V/venv/bin/python tail.py > tail.txt 2>&1
$V/venv/bin/python lsf.py > lsf.log 2>&1
