#!/bin/bash
# Runs every query of both datasets in pieces; each piece's workers finish well inside the 590 s timeout.
cd /home/joe/code/vexact-probe-clusters/cascade
for lo in 0 2500 5000 7500; do ./piece.sh nytimes-256-angular $lo $((lo + 2500)) || echo "FAILED nytimes $lo"; echo "piece nytimes $lo"; done
for lo in $(seq 0 1000 9000); do ./piece.sh glove-100-angular $lo $((lo + 1000)) || echo "FAILED glove $lo"; echo "piece glove $lo"; done
echo ALLDONE
