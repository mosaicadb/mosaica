#!/usr/bin/env bash
#
# Bring up the development viewer: `mosaica serve` over one bundle per dataset, plus the viewer.
#
# This is the client developer's loop. `mosaica demo NAME` (docs/start/datasets.md) is the way to
# look at a dataset; this script serves several at once, each with principals measured over its own
# access terms, behind the viewer's dataset picker. Two things about it are development-only: the
# API key that mints each principal's session reaches the browser, and `serve.dev_cors_origins` is
# typed in to let it talk. Neither is anything to copy into an integration.
#
#   ./run_demo.sh                       # arxiv-sample and places, then open the viewer
#   ./run_demo.sh --dataset geonames    # a dataset of `mosaica demo --list`. Repeatable; the
#                                       # order is the picker's order
#   ./run_demo.sh --dataset gbif:64     # NAME:N builds from a sample, as `mosaica demo NAME:N`
#   ./run_demo.sh --bundle PATH --terms GB,FR,DE [--ranks R.json] [--label 'Name'] [--prose name]
#                                       # a bundle you already have, with its own access terms
#   ./run_demo.sh --deployment D/mosaica.toml --terms-file D/country-terms.txt --ranks D/country-ranks.json
#                                       # a rung of the dataset ladder (corpora/), served on its own
#                                       # deployment: its ports, its credentials, its disclosure floor
#   ./run_demo.sh --no-viewer           # servers only (for curl and the smoke scripts)
#   ./run_demo.sh --build-only          # build the bundles and stop
#   ./run_demo.sh --rebuild             # discard and rebuild the bundles
#
# A dataset is fetched and prepared by `mosaica.datasets.prepared`, through `uv run` in this
# checkout, so the first run of one that is not in git downloads it. The bundle is built here, from
# that declaration, so that its deployment carries this script's ports and credentials.
#
# ## Where it writes: `./mosaica-demo/`, and nothing in the source tree
#
# Bundles, cache, WALs, the generated `mosaica-<dataset>.toml` deployments, the measured presets, the
# build timings and the dataset document all live under `./mosaica-demo/` at the checkout root,
# which is gitignored. `MOSAICA_DEMO_DIR` names somewhere else; a second session on one checkout
# wants its own, because the layout holds one slot per dataset and two runs sharing it overwrite each
# other's picker.
#
# Nothing is written under `clients/`. The viewer is told where the dataset document is through the
# URL this script prints (`?datasets=/@fs/<absolute path>`), and that document carries each
# dataset's API key. `VITE_PORT` moves the viewer off 5173, and whatever port it lands on is what
# the generated deployments enumerate in `serve.dev_cors_origins`.
#
# ## Filtering
#
# The controls come from `/v1/meta`'s `filter_operands`, so each dataset shows the columns its
# declaration renders or indexes.
#
# **Watch `visible` against `matched` in the Counts panel while you filter.** `visible` is the mask,
# what this principal may see in the drawn region, and it must not move. `matched` is what the
# filter admits. A filter that moved `visible` would be reporting the size of a set the principal was
# never granted, which is why the two are separate numbers.
#
# ## Watching the replica work
#
# The viewer holds what it has fetched, and buys a margin beyond the screen while the view is
# still. The **Last request** panel reports `tiles from cache` (of the region the view wanted, how
# much needed no request) and `replica held`; the drawer beneath it opens on `prefetched ahead`,
# `— of which provisional` (marks borrowed from another zoom level while this one loads — drawn like
# any other mark, and deliberately carrying no counts: the count channel is what stops a superset
# being read as density) and the server's stage breakdown.
#
# Adding `&prefetch=0` to the URL this script prints turns look-ahead off and leaves the cache on.
# That is the A/B the measurements use: the cache decides what a request is *answered from*,
# look-ahead decides what is *asked for*, and they are worth judging separately.
#
# `&dataset=places` opens straight onto a dataset, skipping the switch.
#
# ## Recording a session — `?trace=1`
#
# Adding `&trace=1` to the printed URL records what you did, what the client did about it, and how
# long each frame took, into a file you can hand to someone else. It exists because the browser-free
# harness cannot see the three costs a user actually feels:
# GPU upload, frame scheduling, and the delay between an input and the paint answering it.
#
# A bar appears bottom-left. **Press `m`, or click `mark`, the moment something feels wrong** — the
# trace then carries your judgement rather than leaving it to be inferred from the numbers, and that
# marker is the first thing a reader looks for. Then `download trace`.
#
# What it records: frame gaps past 20 ms (smooth runs cost one counter, not one record per frame),
# long tasks, pointer and wheel input, every request and its arrival, splitting a response into
# bands, deriving a frame, the slab sync, and the gap from handing deck.gl its layers to the next
# frame — which is the upload. Plus a header naming the GPU: a trace whose renderer says SwiftShader
# was software-rasterised and its timings mean nothing.
#
# Off unless the parameter is present, down to not installing the observers — an instrument that
# runs inside the frame loop it is timing has to cost nothing when it is not wanted.
#
# Two debug knobs answer the one question the platform will not. `painted` — handing deck.gl its
# layers to the next frame — is the largest remaining cost, and deck's own `gpuTime` reads zero
# here because the GPU timer query extension is absent under ANGLE, so the GPU half cannot be
# measured directly. Measure it by difference, same gesture each time:
#
#   ?trace=1&radius=0.6     marks a third the area. If `painted` falls, the cost is fill rate.
#   ?trace=1&pickable=0     no per-instance picking colours. If `painted` falls, it is that buffer.
#
# Both change what is drawn or what can be clicked. They are instruments, not settings.
#
# Three things that will otherwise waste your time:
#
#   - **Zoom in before panning.** At zoom 0 the whole world is on screen and the view box clamps,
#     so panning changes nothing and every gesture is answered without a request. Nothing is wrong;
#     there is just nothing to fetch.
#   - **The defaults are now the hard case** — every term, the largest mark budget, coloured by
#     `archive` — because the narrow principal saturates at almost any depth and exercises none of
#     the machinery worth watching. Drop to a single term to see the *opposite* extreme; do not
#     expect the cache or the budget to do anything there.
#   - **Let the depth budget settle.** Marks-per-tile calibrates over the first few interactions
#     and only ever goes deeper; while it is moving, each view lands at a depth nothing is held at,
#     so the cache reads cold for reasons that have nothing to do with the cache.
#
# `clients/ts/viewer/smoke-cache.mjs` and `smoke-lookahead.mjs` do all of the above headlessly and
# print the numbers, against a server this script has already started.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
REPO="$PWD"

# Everything this run produces: the bundles, their caches and WALs, the generated deployments, the
# measured presets and the timings. Directory-local rather than under `$HOME`, and gitignored, so
# that two checkouts, or two sessions with `MOSAICA_DEMO_DIR` set, run it without overwriting each
# other.
DEMO="${MOSAICA_DEMO_DIR:-$REPO/mosaica-demo}"
# What is running, which is a fact about this invocation rather than about the repository. The
# viewer fetches it at startup, so restarting against a different set of datasets needs no rebuild,
# and is told where it is by the `?datasets=` parameter in the URL printed at the end, because the
# document lives outside the viewer's package and Vite serves it over `/@fs/`.
DATASETS="$DEMO/datasets.json"

# Vite is `strictPort`, and this origin is what gets written into `dev_cors_origins`. A fallback to
# another port would surface as a CORS failure that reads like a broken server. Set `VITE_PORT` to
# run a second viewer beside one already holding 5173.
VITE_PORT="${VITE_PORT:-5173}"

export MOSAICA_OPERATOR_CRED="${MOSAICA_OPERATOR_CRED:-dev-operator-credential}"

# The checkout's Python environment (the root `pyproject.toml`), which carries pyarrow and the
# `mosaica` package that fetches and prepares the datasets.
PY=(uv run --quiet --project "$REPO" python)

bundle_override=""
# A `mosaica.toml` an operator already has, such as the one a ladder rung's `prepare.py` writes
# beside its parquets. Unlike `--bundle`, nothing here is generated: the bundle path, the three
# ports, the disclosure floor and the credential variable names are that file's, and it is read
# rather than rewritten.
deployment_override=""
run_viewer=1
rebuild=0
build_only=0
datasets=()
# A bundle served with `--bundle` or `--deployment` has its own dictionary, so its candidate terms
# have to be named; a term names a different set in every dictionary.
#
#   --terms       a comma-separated candidate list
#   --terms-file  the same list, one term per line, for terms that hold a comma, such as
#                 TreeOfLife's publisher names ("Royal Botanic Gardens, Kew")
#   --ranks       `[{term, pairs}]`, most-covering first, which composes the sparse, medium and
#                 heavy coverage principals
terms_override=''
terms_file_override=''
ranks_override=''
# What the picker calls a `--bundle` or `--deployment` entry, and the prose columns its source
# panel names. The panel is a caption; the filter surface itself is `/v1/meta`'s.
label_override=''
prose_override=''
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dataset)    datasets+=("$2"); shift 2 ;;
    --bundle)     bundle_override="$2"; shift 2 ;;
    --deployment) deployment_override="$2"; shift 2 ;;
    --terms)      terms_override="$2"; shift 2 ;;
    --terms-file) terms_file_override="$2"; shift 2 ;;
    --ranks)      ranks_override="$2"; shift 2 ;;
    --label)      label_override="$2"; shift 2 ;;
    --prose)      prose_override="$2"; shift 2 ;;
    --no-viewer)  run_viewer=0; shift ;;
    --build-only) build_only=1; shift ;;
    --rebuild)    rebuild=1; shift ;;
    -h|--help)    sed -n '3,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)            echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
# `--ranks` and `--terms-file` are resolved against the working directory they were typed in,
# because the measurement loop below runs from `clients/ts`.
if [[ -n "$ranks_override" ]]; then
  [[ -f "$ranks_override" ]] || { echo "no such ranks file: $ranks_override" >&2; exit 1; }
  ranks_override="$(cd "$(dirname "$ranks_override")" && pwd)/$(basename "$ranks_override")"
fi
if [[ -n "$terms_file_override" ]]; then
  if [[ -n "$terms_override" ]]; then
    echo "--terms and --terms-file name the candidate list twice" >&2; exit 2
  fi
  [[ -f "$terms_file_override" ]] || { echo "no such terms file: $terms_file_override" >&2; exit 1; }
  terms_file_override="$(cd "$(dirname "$terms_file_override")" && pwd)/$(basename "$terms_file_override")"
fi
if [[ -n "$bundle_override$deployment_override" && -z "$terms_override$terms_file_override" ]]; then
  echo "--bundle and --deployment need the bundle's access terms: --terms A,B,C or --terms-file F" >&2
  exit 2
fi

# The default is the two example datasets, which a fresh clone holds.
[[ ${#datasets[@]} -eq 0 ]] && datasets=(arxiv-sample places)

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

# One trap for every child. Without it a Ctrl-C in the foreground viewer leaves `mosaica serve`
# holding the WALs and the listeners, and the next run fails on a bound port rather than on the
# thing that went wrong.
SERVE_PIDS=()
cleanup() {
  for pid in "${SERVE_PIDS[@]:-}"; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done
  for pid in "${SERVE_PIDS[@]:-}"; do
    [[ -n "$pid" ]] && wait "$pid" 2>/dev/null || true
  done
}
trap cleanup EXIT INT TERM

# `MOSAICA_BIN` serves a binary built elsewhere, such as a second worktree sharing one machine's
# release build, and is otherwise this checkout's, built here.
if [[ -n "${MOSAICA_BIN:-}" ]]; then
  BIN="$MOSAICA_BIN"
  [[ -x "$BIN" ]] || { echo "MOSAICA_BIN=$BIN is not an executable" >&2; exit 1; }
  say "using $BIN"
else
  say "building the release binary"
  cargo build --release -p mosaica
  BIN="$REPO/target/release/mosaica"
fi
export MOSAICA_BIN="$BIN"

# Ports are fixed per dataset rather than allocated, so a `curl` in a second terminal keeps working
# across restarts and the smoke scripts need no discovery step.
ports_of() {
  case "${1%%:*}" in
    arxiv-sample) echo "37591 49309 45727" ;;
    places)       echo "37592 49310 45728" ;;
    geonames)     echo "37593 49311 45729" ;;
    gbif)         echo "37594 49312 45730" ;;
    custom)       echo "37599 49399 45799" ;;
    *) echo "run_demo.sh has no ports for ${1%%:*}; add it to ports_of" >&2; exit 2 ;;
  esac
}
viewer_of()  { local p; p=($(ports_of "$1")); echo "${p[0]}"; }
session_of() { local p; p=($(ports_of "$1")); echo "${p[1]}"; }
control_of() { local p; p=($(ports_of "$1")); echo "${p[2]}"; }

# What the viewer needs to know of a dataset, read from its declaration once it is prepared: the
# items, the prose columns the source panel names, the column a record is titled by, the column
# holding each item's access terms, and the label the picker shows.
declare -A SCHEMA ITEMS PROSE TITLE ACCESS LABEL
describe_dataset() {
  local name="$1" line
  line="$("${PY[@]}" - "$name" <<'DESCRIBE'
import json, sys, tomllib
from pathlib import Path
import pyarrow.parquet as pq
from mosaica import datasets
import contextlib
name, _, sample = sys.argv[1].partition(":")
# What the steps print goes to the terminal; stdout carries the one line this script reads.
with contextlib.redirect_stdout(sys.stderr):
    schema = datasets.prepared(name, sample=int(sample) if sample else None)
declared = tomllib.loads(schema.read_text())
view = declared["view"][0]
source = view.get("source") or declared.get("defaults", {}).get("source")
items = pq.ParquetFile(schema.parent / declared["sources"][source]).metadata.num_rows
texts = [a["name"] for a in declared.get("attribute", []) if a["type"] == "text"]
prose = [a for a in texts if any(b["name"] == a and b.get("index") for b in declared["attribute"])]
print("\x1f".join([str(schema), str(items), json.dumps(prose)[1:-1], texts[0] if texts else "",
                 view["point_visibility"]["field"], datasets.describe(name).title]))
DESCRIBE
)" || exit 1
  IFS=$'\x1f' read -r "SCHEMA[$name]" "ITEMS[$name]" "PROSE[$name]" "TITLE[$name]" "ACCESS[$name]" "LABEL[$name]" <<<"$line"
}

# A built bundle's items: the high water of every shard in its newest publication.
high_water_of() {
  "${PY[@]}" -c "
import sys
sys.path.insert(0, sys.argv[2])
from oracle.layout import newest_side_manifest
side = newest_side_manifest(sys.argv[1])
print(sum(record['high_water'] for record in side['shards']['records']))
" "$1" "$REPO/reference"
}

# `--bundle` serves a bundle as it is: one server, one entry in the picker, no build.
if [[ -n "$bundle_override" && -z "$deployment_override" ]]; then
  [[ -d "$bundle_override" ]] || { echo "no such bundle: $bundle_override" >&2; exit 1; }
  datasets=(custom)
fi

# `--deployment` differs from `--bundle` in what it does not do: it writes no deployment file. A
# ladder rung's `mosaica.toml` carries the bundle, the three ports, the disclosure floor and the
# names of the variables holding its credentials, and none of that is the demo's to overwrite.
#
# The secrets are read from a `.env` beside that file when the environment does not already carry
# them, since `mosaica serve` reads credentials from the process environment alone.
if [[ -n "$deployment_override" ]]; then
  [[ -n "$bundle_override" ]] && { echo "--deployment and --bundle name the same thing twice" >&2; exit 2; }
  [[ -f "$deployment_override" ]] || { echo "no such deployment: $deployment_override" >&2; exit 1; }
  deployment_override="$(cd "$(dirname "$deployment_override")" && pwd)/$(basename "$deployment_override")"
  DEPLOY_DIR="$(dirname "$deployment_override")"

  # A scalar out of one `[section]`, quotes stripped. The keys read here are addresses, paths and
  # variable names, one line each, which is the whole of the TOML this script needs.
  toml_scalar() {
    awk -v want_section="$1" -v want_key="$2" '
      /^[[:space:]]*\[/ { section = $0; gsub(/[][[:space:]]/, "", section); next }
      { line = $0; sub(/#.*/, "", line)
        if (section == want_section && match(line, /^[[:space:]]*[A-Za-z_]+[[:space:]]*=/)) {
          key = substr(line, 1, index(line, "=") - 1); gsub(/[[:space:]]/, "", key)
          if (key == want_key) {
            value = substr(line, index(line, "=") + 1)
            gsub(/^[[:space:]]*["'"'"']?|["'"'"']?[[:space:]]*$/, "", value)
            print value; exit
          }
        }
      }' "$deployment_override"
  }
  port_of_addr() { echo "${1##*:}"; }

  bundle_override="$(toml_scalar bundle path)"
  [[ -n "$bundle_override" ]] || { echo "$deployment_override: no [bundle].path" >&2; exit 1; }
  # `[bundle].path` is resolved against the deployment file, as the binary resolves it.
  [[ "$bundle_override" = /* ]] || bundle_override="$DEPLOY_DIR/$bundle_override"
  [[ -d "$bundle_override" ]] || { echo "$deployment_override names a bundle that is not there: $bundle_override" >&2; exit 1; }

  # The operator credential this deployment declares, taken from the environment first and from
  # the `.env` beside the file second: an exported variable is more specific than a file.
  var="$(toml_scalar serve operator_credential_env)"
  if [[ -n "$var" ]]; then
    if [[ -z "${!var:-}" && -f "$DEPLOY_DIR/.env" ]]; then
      value="$(sed -n "s/^[[:space:]]*\(export[[:space:]]\+\)\?$var=//p" "$DEPLOY_DIR/.env" | head -1 | sed "s/^[\"']//;s/[\"']$//")"
      [[ -n "$value" ]] && export "$var=$value"
    fi
    [[ -n "${!var:-}" ]] || { echo "$deployment_override declares $var for the operator and nothing sets it (not the environment, not $DEPLOY_DIR/.env)" >&2; exit 1; }
    # The presets script reads the demo's variable name, not this deployment's.
    export MOSAICA_OPERATOR_CRED="${!var}"
  fi

  # The browser talks to this server directly, so an origin the deployment does not enumerate is a
  # CORS failure that reads like a broken server. Reported, not refused: `--no-viewer` wants no
  # origin at all.
  if [[ $run_viewer -eq 1 ]] && ! grep -q "localhost:$VITE_PORT" "$deployment_override"; then
    echo "warning: $deployment_override does not list http://localhost:$VITE_PORT in [serve].dev_cors_origins;" >&2
    echo "         the viewer will load and every request from it will fail CORS." >&2
  fi

  # The server does not create the parents of the WAL and the cache the deployment names.
  for relative in "$(toml_scalar bundle wal)" "$(toml_scalar bundle cache)"; do
    [[ -n "$relative" ]] || continue
    absolute="$relative"
    [[ "$absolute" = /* ]] || absolute="$DEPLOY_DIR/$absolute"
    mkdir -p "$(dirname "$absolute")"
  done

  datasets=(custom)
  ports_of() {
    echo "$(port_of_addr "$(toml_scalar serve viewer)") $(port_of_addr "$(toml_scalar serve session)") $(port_of_addr "$(toml_scalar serve control)")"
  }
  label_override="${label_override:-$(basename "$DEPLOY_DIR")}"
fi

if [[ -n "$bundle_override" ]]; then
  ITEMS[custom]="$(high_water_of "$bundle_override")"
  PROSE[custom]="$(if [[ -n "$prose_override" ]]; then printf '"%s"' "${prose_override//,/\",\"}"; fi)"
  TITLE[custom]=''
  LABEL[custom]="${label_override:-$bundle_override}"
else
  for name in "${datasets[@]}"; do
    ports_of "$name" >/dev/null
    say "preparing $name"
    describe_dataset "$name"
  done
fi

bundle_of() {
  if [[ -n "$bundle_override" ]]; then echo "$bundle_override"; else echo "$DEMO/bundle-$1"; fi
}

# Which `mosaica.toml` a dataset is built and served against: the operator's under
# `--deployment`, and otherwise the one `write_deployment` generates below.
deployment_of() {
  if [[ -n "$deployment_override" ]]; then echo "$deployment_override"; else echo "$DEMO/mosaica-$1.toml"; fi
}

# ------------------------------------------------------------------------------------ the builds

# One `mosaica.toml` per dataset, written to the gitignored dev directory and read by both verbs:
# `mosaica build` reads `[bundle].path` as its output and `[build].schema` as its declaration, and
# `mosaica serve` opens what the build wrote. The paths are absolute because this file is
# generated per machine and never committed.
write_deployment() {
  # `--deployment` names a file this script did not write and must not rewrite.
  [[ -n "$deployment_override" ]] && return 0
  local name="$1" bundle
  bundle="$(bundle_of "$name")"
  mkdir -p "$DEMO/$name"
  cat > "$DEMO/mosaica-$name.toml" <<EOF
[bundle]
path  = "$bundle"
cache = "$DEMO/$name/cache"
wal   = "$DEMO/$name/wal.log"

[build]
schema = "${SCHEMA[$name]:-}"

[disclosure]
token_max_lifetime = 3600

[serve]
viewer = "127.0.0.1:$(viewer_of "$name")"
session = "127.0.0.1:$(session_of "$name")"
control = "127.0.0.1:$(control_of "$name")"
max_k = 5000
operator_credential_env = "MOSAICA_OPERATOR_CRED"
dev_cors_origins = ["http://localhost:$VITE_PORT"]

[catalogue]
dir = "$DEMO/$name/catalogue"
EOF
}

build_dataset() {
  local name="$1" bundle
  bundle="$(bundle_of "$name")"

  [[ $rebuild -eq 1 ]] && rm -rf "$bundle"
  # `CURRENT` is written last, so its presence, not the directory's, says the build finished. A
  # build killed partway leaves `v00000` holding a truncated segment, and serving that directory
  # would serve a map missing whatever the writer had not reached.
  if [[ -d "$bundle" && -e "$bundle/CURRENT" ]]; then
    say "reusing $name at $bundle (--rebuild to discard it)"
    return
  fi
  if [[ -d "$bundle" ]]; then
    say "discarding an incomplete $name bundle at $bundle (no CURRENT: a build died partway)"
    rm -rf "$bundle"
  fi

  say "building the $name bundle (${ITEMS[$name]} items)"
  mkdir -p "$DEMO"
  write_deployment "$name"

  # Timed, with the peak RSS kept alongside the wall clock, because peak RSS decides whether the
  # next dataset fits on the machine and the bundle on disk does not record it. `/usr/bin/time -v`
  # rather than the shell builtin, which reports no memory.
  local t0=$SECONDS
  # `MOSAICA_BUILD_MEMORY_BUDGET` (e.g. `8g`) caps the build's own structures. Unset, the binary
  # sizes its batches from `MemAvailable` when it starts, which is wrong on a machine where the page
  # cache for a large points file and bundle competes for the same pages.
  #
  # `MOSAICA_BUILD_MEMORY_MAX` (e.g. `9G`) runs the build inside a transient cgroup, which also
  # charges the page cache against the ceiling, so the kernel reclaims this build's cache instead of
  # driving the whole machine into reclaim. `MemorySwapMax=0` keeps the build out of swap. The
  # binary reads the cgroup's `memory.max` too, so the plan it prints accounts for it.
  #
  # `MemoryHigh` defaults to `infinity`, because `memory.high` throttles rather than caps: measured
  # at 250,000,000 items with `MemoryHigh=7G`, the build spent 449 of 466 seconds in reclaim and
  # wrote nothing. `MemoryMax` is the number to set.
  local -a scope=()
  if [[ -n "${MOSAICA_BUILD_MEMORY_MAX:-}" ]]; then
    scope=(systemd-run --user --scope -q --collect
           -p "MemoryMax=$MOSAICA_BUILD_MEMORY_MAX"
           -p "MemoryHigh=${MOSAICA_BUILD_MEMORY_HIGH:-infinity}"
           -p "MemorySwapMax=0")
  fi
  "${scope[@]}" \
  /usr/bin/time -v -o "$DEMO/build-$name.time" \
  "$BIN" build \
    --deployment "$(deployment_of "$name")" \
    ${MOSAICA_BUILD_MEMORY_BUDGET:+--memory-budget "$MOSAICA_BUILD_MEMORY_BUDGET"} \
    --no-oracle-pairs
  local peak
  peak=$(awk '/Maximum resident set size/ {printf "%.1f GiB", $NF / 1048576}' "$DEMO/build-$name.time")
  printf 'built %s in %dm%02ds, peak %s, %s on disk\n' \
    "$name" "$(( (SECONDS - t0) / 60 ))" "$(( (SECONDS - t0) % 60 ))" \
    "$peak" "$(du -sh "$bundle" | cut -f1)" | tee -a "$DEMO/build-times.txt"
}

if [[ -z "$bundle_override" ]]; then
  for name in "${datasets[@]}"; do build_dataset "$name"; done
fi

if [[ $build_only -eq 1 ]]; then
  say "built; not serving (--build-only)"
  exit 0
fi

# ----------------------------------------------------------------------------------- the servers

start_dataset() {
  local name="$1" viewer_port
  viewer_port="$(viewer_of "$name")"

  # The same file the build was invoked against. `--bundle` skips the build, so it may not exist
  # yet, and rewriting it is idempotent either way.
  write_deployment "$name"

  # A port already in use is a refusal. The readiness poll below asks the port whether it is
  # ready, not the process this function started, so a server left over from another run would
  # answer and everything downstream would talk to a different bundle.
  for port in $(ports_of "$name"); do
    if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
      exec 3<&- 3>&-
      echo "port $port is already in use, so $name cannot be served on it." >&2
      echo "Something is already listening, most likely a mosaica serve from an earlier run." >&2
      echo "Stop it, or serve this bundle on other ports." >&2
      exit 1
    fi
  done

  say "starting mosaica serve for $name on :$viewer_port"
  "$BIN" serve --deployment "$(deployment_of "$name")" &
  SERVE_PIDS+=($!)
  local pid=${SERVE_PIDS[-1]}

  # Poll `/readyz` rather than sleeping: a 200 means the bundle opened and the workers are ready.
  printf 'waiting for %s readyz' "$name"
  local ready=0
  for _ in $(seq 1 300); do
    if curl -sf "http://127.0.0.1:$viewer_port/readyz" >/dev/null 2>&1; then ready=1; break; fi
    kill -0 "$pid" 2>/dev/null || { echo; echo "mosaica serve ($name) exited during startup" >&2; exit 1; }
    printf '.'; sleep 1
  done
  echo
  [[ $ready == 1 ]] || { echo "$name did not become ready" >&2; exit 1; }
  echo "ready: $name viewer plane on http://127.0.0.1:$viewer_port"
}

for name in "${datasets[@]}"; do start_dataset "$name"; done

# The viewer's configuration, in the environment rather than in a `.env.local` under `clients/`, so
# nothing is left behind for the next run to read. These name the fallback server, which the
# viewer uses when no dataset document reaches it; the picker's entries come from `$DATASETS`.
export VITE_MOSAICA_VIEWER_URL="http://127.0.0.1:$(viewer_of "${datasets[0]}")"
export VITE_MOSAICA_SESSION_URL="http://127.0.0.1:$(session_of "${datasets[0]}")"
export VITE_MOSAICA_DATASETS="/@fs$DATASETS"
export VITE_PORT

# ------------------------------------------------------------------------- the dataset document

# Presets are measured per bundle, since a term names a different set in each dictionary. A
# dataset's candidate terms are read off the column its view's `point_visibility` names, and
# ranked by how many items carry each, so the presets script composes the same five bands over
# every dataset: narrow, sparse, medium, heavy and full.
[[ -d "$REPO/clients/ts/node_modules" ]] || { say "npm ci"; (cd "$REPO/clients/ts" && npm ci); }

say "measuring principals per dataset"
mkdir -p "$(dirname "$DATASETS")" "$DEMO/presets"
for name in "${datasets[@]}"; do
  ranks="$ranks_override"
  if [[ -n "$terms_file_override" ]]; then
    terms_args=(--terms-file "$terms_file_override")
  elif [[ -n "$terms_override" ]]; then
    terms_args=(--terms "$terms_override")
  else
    ranks="$DEMO/presets/$name.term-ranks.json"
    "${PY[@]}" - "${SCHEMA[$name]}" "$ranks" "${ACCESS[$name]}" <<'CANDIDATES'
import collections, json, sys, tomllib
from pathlib import Path
import pyarrow.parquet as pq
schema = Path(sys.argv[1])
declared = tomllib.loads(schema.read_text())
points = schema.parent / declared["sources"][declared["defaults"]["source"]]
# A list of terms per item, or one term (a place's country).
counts = collections.Counter(
    t for held in pq.read_table(points, columns=[sys.argv[3]]).column(0).to_pylist()
    for t in ([held] if isinstance(held, str) else held or []))
ranked = [{"term": t, "pairs": n} for t, n in counts.most_common()]
Path(sys.argv[2]).write_text(json.dumps(ranked) + "\n")
Path(sys.argv[2]).with_suffix(".txt").write_text("\n".join(r["term"] for r in ranked) + "\n")
CANDIDATES
    terms_args=(--terms-file "${ranks%.json}.txt")
  fi
  # Each preset is a principal of the dataset's catalogue, created here with the operator
  # credential, and the viewer mints its sessions with the API key written beside the presets.
  (cd "$REPO/clients/ts" && node scripts/measure-principals.mjs \
    --viewer "http://127.0.0.1:$(viewer_of "$name")" \
    --session "http://127.0.0.1:$(session_of "$name")" \
    --control "http://127.0.0.1:$(control_of "$name")" \
    "${terms_args[@]}" \
    --out "$DEMO/presets/$name.json" \
    --key-out "$DEMO/presets/$name.key" \
    $([[ -n "$ranks" && -f "$ranks" ]] && echo --ranks "$ranks"))
done

# This run's entries, one per dataset, in the order given: the viewer opens on the first.
fresh="$DEMO/presets/datasets-this-run.json"
{
  echo '['
  first=1
  for name in "${datasets[@]}"; do
    [[ $first -eq 1 ]] || echo ','
    first=0
    printf '{"id":"%s","label":"%s","items":%s,"prose":[%s],' \
      "$name" "${LABEL[$name]}" "${ITEMS[$name]}" "${PROSE[$name]}"
    if [[ -n "${TITLE[$name]}" ]]; then printf '"titleField":"%s",' "${TITLE[$name]}"; fi
    # The key reaches the viewer's page through this document, which is for development only.
    printf '"viewerUrl":"http://127.0.0.1:%s","sessionUrl":"http://127.0.0.1:%s","apiKey":"%s","presets":' \
      "$(viewer_of "$name")" "$(session_of "$name")" "$(cat "$DEMO/presets/$name.key")"
    cat "$DEMO/presets/$name.json"
    printf '}'
  done
  echo ']'
} > "$fresh"

# Merged with what is already running rather than overwritten, so two invocations serving two sets
# of datasets on their fixed ports both appear in the picker. An entry this run did not write is
# kept if its viewer plane still answers `/readyz` and dropped if it does not.
"${PY[@]}" - "$DATASETS" "$fresh" <<'MERGE'
import json, pathlib, sys, urllib.request
dest, fresh = pathlib.Path(sys.argv[1]), json.loads(pathlib.Path(sys.argv[2]).read_text())
mine = {d["id"] for d in fresh}
# An entry naming an address this run claimed is stale whatever answers there, since two ladder
# rungs share one port triple.
ours = {d["viewerUrl"] for d in fresh}
kept = []
if dest.exists():
    for d in json.loads(dest.read_text()).get("datasets", []):
        if d["id"] in mine:
            continue
        if d["viewerUrl"] in ours:
            print(f"  dropping {d['id']}: {d['viewerUrl']} is served by this run's "
                  + ", ".join(e["id"] for e in fresh if e["viewerUrl"] == d["viewerUrl"]))
            continue
        try:
            urllib.request.urlopen(d["viewerUrl"] + "/readyz", timeout=2)
            kept.append(d)
        except Exception:
            print(f"  dropping {d['id']}: nothing answers at {d['viewerUrl']}")
dest.write_text(json.dumps({"datasets": fresh + kept}, indent=2) + "\n")
print(f"  {', '.join(d['id'] for d in fresh)} written" + (f"; {', '.join(d['id'] for d in kept)} kept" if kept else ""))
MERGE
echo "wrote $DATASETS"

if [[ $run_viewer -eq 0 ]]; then
  say "servers only; Ctrl-C to stop"
  wait
  exit 0
fi

cd "$REPO/clients/ts"

# The dataset document is outside the viewer's package, so the viewer is handed its location:
# Vite serves any file under an allowed root at `/@fs/<absolute path>` (`vite.config.ts` allows
# `$DEMO`), and `?datasets=` names it. The bare port falls back to the single server the
# environment above names.
VIEWER_URL="http://localhost:$VITE_PORT/?datasets=/@fs$DATASETS"

say "viewer on $VIEWER_URL (Ctrl-C to stop everything)"
echo "Opens on the broadest principal and the largest mark budget."
echo
echo "Filters are in the left column, drawn from what each bundle publishes as filterable."
echo "  Watch 'visible' hold and 'matched' fall in Counts: the filter never moves the mask."
echo
echo "To watch the replica: zoom in a few notches, then pan away and back."
echo "To record a session for someone else: add &trace=1, press m when it feels wrong, download."
echo "Add &prefetch=0 to turn look-ahead off, cache still on, for comparison."
echo "The smoke scripts take the whole URL: --url '$VIEWER_URL'"
npm run dev -w @mosaicajs/viewer
