#!/usr/bin/env bash
# Runs this track's small jobs one after another on core 6, after the two-round job has finished.
cd /home/joe/code/vexact-theory/notes/program/rotations
while kill -0 103611 2>/dev/null; do sleep 5; done
for job in "d_butterfly.py:out_d.txt" "e_rounding.py:out_e.txt" "f_subblocks.py:out_f.txt" "b3_structured.py:out_b3.txt"; do
  s="${job%%:*}"; o="${job##*:}"
  ../run-limited.sh 6 1 2 python3 "$s" > "$o" 2>&1
done
