#!/bin/bash
# Times a group's setup per group, as queries pay it, on one pinned core: `vexact --v2 ... gsetup` (rust/src/run2.rs,
# gsetup_cost), warm and --cold, on the suite's indexes and optionally on synthetic indexes at larger n. See
# ../groups-setup.md for what each column is and what it decides. Run it only when the machine is quiet.
#
# The suite's saved indexes were built by another library version (their key's library fingerprint differs), and
# opening one with the CLI can write cache files into its directory, so each is re-encoded from its fit into a scratch
# copy first: the fit's files are copied there, its Xs.npy (read only) linked. Nothing under $SUITE is written.
#
# usage: S=/scratch/dir [CORE=3] [NQ=100] [DATASETS="..."] [SYNTH="768"] [KEEP=1] [DRY=1] groups_setup_time.sh
#   S         scratch directory (indexes and results go to $S/gsetup); about 1.4 GB per suite index while it runs
#   CORE      the CPU the timed process is pinned to (taskset and --pin); the builds use every CPU
#   NQ        queries per class (item queries from the fit's qi.npy, text queries from the suite's queries.npy)
#   DATASETS  suite datasets (default: the three of the quick suite, n = 512, 384 and 128)
#   SYNTH     dimensions of synthetic indexes to time as well (64 groups of one cluster, 2n items each, clustered unit
#             vectors); n = 2048 is about 2.2 GB of vectors: give SYN_G=16 for 16 groups
#   KEEP      keep each scratch index after its run (default: removed, results kept)
#   DRY       print the commands only
set -e
S=${S:?set S to a scratch directory}
CORE=${CORE:-3}
NQ=${NQ:-100}
SUITE=${SUITE:-/home/user/suite-data}
DATASETS=${DATASETS:-laion-clip-512-normalized yahoo-minilm-384-normalized glove-100-angular}
SYNTH=${SYNTH:-}
SYN_G=${SYN_G:-64}
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(cd "$HERE/../../../../../rust" && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$RUST/target}
BIN=$CARGO_TARGET_DIR/release/vexact
OUT=$S/gsetup
run() { echo "+ $*"; [ -n "$DRY" ] || "$@"; }

run cargo build --release --manifest-path "$RUST/Cargo.toml"
mkdir -p "$OUT"

# the timed runs: warm (bases as the previous part left them) and cold (64 MB read before each part)
time_index() {
  local idx=$1 res=$2 queries=$3
  local q=(); [ -n "$queries" ] && q=(--queries "$queries")
  run taskset -c "$CORE" "$BIN" --v2 --index "$idx" --results "$res" "${q[@]}" --nq "$NQ" --pin "$CORE" gsetup
  run taskset -c "$CORE" "$BIN" --v2 --index "$idx" --results "$res" "${q[@]}" --nq "$NQ" --pin "$CORE" --cold gsetup
}

for ds in $DATASETS; do
  fit=$SUITE/work/$ds/vexact/index
  idx=$OUT/$ds
  [ -e "$fit/meta.json" ] || { echo "no fit at $fit; give DATASETS from $SUITE/work" >&2; exit 2; }
  run mkdir -p "$idx"
  for f in "$fit"/*; do
    b=$(basename "$f")
    case $b in encoded*|results) continue ;; Xs.npy) run ln -sf "$f" "$idx/$b" ;; *) [ -f "$f" ] && run cp "$f" "$idx/$b" ;; esac
  done
  run "$BIN" --v2 --index "$idx" --rebuild --results "$OUT/results/$ds"
  time_index "$idx" "$OUT/results/$ds" "$SUITE/prep/$ds/queries.npy"
  [ -n "$KEEP" ] || run rm -rf "$idx/encoded"
done

for n in $SYNTH; do
  idx=$OUT/syn$n
  vec=$OUT/syn$n.npy
  qs=$OUT/syn$n-queries.npy
  run python3 -I -c "
import numpy as np
n, g = $n, $SYN_G
rng = np.random.default_rng(n)
m = 2 * n
c = rng.standard_normal((g, n)); c /= np.linalg.norm(c, axis=1, keepdims=True)
x = c[np.repeat(np.arange(g), m)] + 0.5 * rng.standard_normal((g * m, n)) / np.sqrt(n)
x /= np.linalg.norm(x, axis=1, keepdims=True)
np.save('$vec', x.astype(np.float32))
q = c[rng.integers(0, g, 200)] + 0.7 * rng.standard_normal((200, n)) / np.sqrt(n)
np.save('$qs', (q / np.linalg.norm(q, axis=1, keepdims=True)).astype(np.float32))
"
  run "$BIN" --vectors "$vec" --index "$idx" --groups "$SYN_G" --per-cluster $((2 * n)) create
  time_index "$idx" "$OUT/results/syn$n" "$qs"
  [ -n "$KEEP" ] || run rm -rf "$idx" "$vec"
done

echo "results: $OUT/results/*/gsetup.csv and gsetup_cold.csv (microseconds per group; columns in ../groups-setup.md)"
