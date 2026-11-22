#!/bin/bash
while kill -0 600377 2>/dev/null; do sleep 5; done
DIAG_SETS=none DIAG_MODES=built DIAG_PSIS=5,6 ./run_study.sh laion-clip-512-normalized $PWD/costs-laion-6f86a1c.kv $PWD/laion-c6f-cal.txt > laion-c6f-cal.log 2>&1
