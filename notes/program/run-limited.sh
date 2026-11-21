#!/usr/bin/env bash
# Run one job inside the limits the program agreed with other sessions on this machine.
# usage: run-limited.sh <cores, e.g. 0-3> <threads> <address-space limit in GB> <command...>
# Never give cores 10 or 11 while another session is timing; check `uptime` and `ps` first.
set -euo pipefail
cores="$1"; threads="$2"; gb="$3"; shift 3
exec nice -n 10 taskset -c "$cores" prlimit --as=$((gb * 1024 * 1024 * 1024)) \
  env OMP_NUM_THREADS="$threads" OPENBLAS_NUM_THREADS="$threads" MKL_NUM_THREADS="$threads" "$@"
