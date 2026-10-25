#!/usr/bin/env bash
# The read side at several shards (sharding.md §8.2): for each shard count, a build of the corpus
# at a `[shards] size` of its items over N, rounded up, then the interactive bench (cold, and
# reopened over its kept cache) and the serve battery (cold, pages cold over a warm engine, hot),
# each server under the 24 GiB cap. One shard is built without a `[shards]` table, at the default.
#
#   cargo build --release -p mosaica
#   npm --prefix clients/ts run build -w @mosaicajs/client
#   BINARY and CORE name the binary and the TS core's built index.js, so a run keeps its own
#   while the checkout moves on.
#   LADDER=<ladder> OUT=<dir> CORPUS=<rung> ITEMS=<items> SHARDS="1 2 8" UNIQUE=<field> \
#     TEXT=<text attribute> RANKS=<ranks file in the rung> bash run.sh
#
# Each count writes under $OUT/<corpus>-<N>/: build.log and build-time.txt, bench.json and
# battery.json. KEEP=0 deletes the bundle after its runs, for corpora too large to hold several.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$HERE/../..}
LADDER=${LADDER:?the ladder directory holding the corpus}
OUT=${OUT:?a directory for the results}
CORPUS=${CORPUS:?the rung}
ITEMS=${ITEMS:?the corpus items, which each shard count divides}
SHARDS=${SHARDS:?the shard counts}
UNIQUE=${UNIQUE:?the unique attribute the lookups use}
TEXT=${TEXT:?a text attribute for the battery's text searches}
RANKS=${RANKS:?the rung's term ranks file}
KEEP=${KEEP:-1}
BINARY=${BINARY:-$REPO/target/release/mosaica}
CORE=${CORE:-$REPO/clients/ts/core/dist/index.js}
CAP=$((24 << 30))
RUNG=$LADDER/$CORPUS
TARGETS=0.01,0.07,0.85,1.0

# Timed runs wait for a one-minute load average under 3, since other work shares this host.
quiet() {
  while [ "$(cut -d' ' -f1 /proc/loadavg | cut -d. -f1)" -ge 3 ]; do sleep 60; done
  cut -d' ' -f1-3 /proc/loadavg >>"$dir/load.txt"
}

deployment() {
  local n=$1 dir=$2
  mkdir -p "$dir"
  for f in "$RUNG"/*; do
    case $(basename "$f") in
      corpus.toml | mosaica.toml | tessera.toml | bundle* | workload-results) ;;
      *) ln -sfn "$f" "$dir/" ;;
    esac
  done
  cp "$RUNG/.env" "$RUNG/mosaica.toml" "$RUNG/corpus.toml" "$dir/"
  local cred
  cred=$(sed -n 's/^operator_credential_env *= *"\(.*\)"/\1/p' "$dir/mosaica.toml")
  grep -q "^$cred=" "$dir/.env" || echo "$cred=$(openssl rand -hex 24)" >>"$dir/.env"
  if [ "$n" -gt 1 ]; then
    (cd "$REPO" && python3 -c "from pathlib import Path; from test_corpora.common.ingest_cycle.split \
import declare_shard_size; declare_shard_size(Path('$dir/corpus.toml'), $(((ITEMS + n - 1) / n)))")
  fi
}

run() {
  local n=$1 dir=$OUT/$CORPUS-$1
  echo "== $CORPUS at $n shards ($(cut -d' ' -f1-3 /proc/loadavg), $(df -h --output=avail / | tail -1) free)" >&2
  deployment "$n" "$dir"
  if [ ! -f "$dir/bundle/.built" ]; then
    rm -rf "$dir/bundle"
    (cd "$dir" && /usr/bin/time -v -o build-time.txt "$BINARY" build >build.log 2>&1)
    du -sb "$dir/bundle" >"$dir/bundle-bytes.txt"
    touch "$dir/bundle/.built"
  fi
  if [ "${BUILD_ONLY:-0}" = 1 ]; then return; fi
  cd "$REPO"
  quiet
  # The camera script runs once, on the first count's bundle, and every count replays its requests,
  # so each ratio compares the same work. Delete the reference when that bundle is rebuilt.
  local ref=$OUT/$CORPUS-reference.json
  if [ ! -f "$ref" ]; then
    python3 -m test_corpora.common.interactive_bench --deployment "$dir" --start --reopen \
      --binary "$BINARY" --core "$CORE" --cap 24G --scratch "$dir/bench-scratch" --unique-field "$UNIQUE" \
      --ranks "$dir/$RANKS" --out "$ref" >"$OUT/$CORPUS-reference.log" 2>&1
    quiet
  fi
  python3 -m test_corpora.common.interactive_bench --deployment "$dir" --start --reopen \
    --binary "$BINARY" --core "$CORE" --cap 24G --scratch "$dir/bench-scratch" --unique-field "$UNIQUE" \
    --ranks "$dir/$RANKS" --replay "$ref" --out "$dir/bench.json" >"$dir/bench.log" 2>&1
  quiet
  python3 -m test_corpora.common.serve_battery --targets "$TARGETS" --samples 15 --boot-rung "$dir" --boot-bundle "$dir/bundle" \
    --boot-scratch "$dir/battery-scratch" --boot-binary "$BINARY" --cap-bytes "$CAP" \
    --ranks "$dir/$RANKS" --text-column "$TEXT" --out "$dir/battery.json" >"$dir/battery.log" 2>&1
  rm -rf "$dir/bench-scratch" "$dir/battery-scratch"
  if [ "$KEEP" = 0 ]; then rm -rf "$dir/bundle"; fi
}

for n in $SHARDS; do run "$n"; done
echo "== done" >&2
