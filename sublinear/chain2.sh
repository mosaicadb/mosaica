#!/bin/bash
# after the fixed-G ladder: LSF with the wider grid, the truncated-dimension scores, and their tail statistics
V=/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc
cd $V/sublinear
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
while pgrep -f "ladder.py" > /dev/null || pgrep -f "prep_g.py" > /dev/null; do sleep 3; done
$V/venv/bin/python lsf.py > lsf.log 2>&1
$V/venv/bin/python dims.py > dims.log 2>&1
for d in 32 96 256; do SFILE=S_$d.f32 $V/venv/bin/python tail.py > tail_$d.txt 2>&1; done
