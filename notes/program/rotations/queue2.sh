#!/usr/bin/env bash
# Second batch of this track's small jobs, after queue1.sh has finished. Core 6, one thread, 2 GB.
cd /home/joe/code/vexact-theory/notes/program/rotations
while pgrep -f "rotations/queue1.sh|\./queue1.sh" > /dev/null 2>&1; do sleep 5; done
for job in "h_unitary.py:out_h.txt" "g_dither.py:out_g.txt"; do
  s="${job%%:*}"; o="${job##*:}"
  ../run-limited.sh 6 1 2 python3 "$s" > "$o" 2>&1
done
