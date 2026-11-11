#!/usr/bin/env bash
# Runs the Rust tests a change can reach, with `cargo nextest run`: the packages it touches and
# every package that depends on them, by any kind of edge. The changes are those since this branch
# left the base (origin/main unless BASE names another ref), plus uncommitted and untracked files.
# nextest's default profile leaves out the slow tier that .config/nextest.toml names, and
# `--profile ci` runs it too.
#
#   bash scripts/test-affected.sh                   # run them
#   bash scripts/test-affected.sh -n                # print the choice and run nothing
#   bash scripts/test-affected.sh -- --profile ci   # arguments after -- go to cargo nextest run
#
# A change to the workspace's own configuration runs `--workspace`. A changed file outside
# `crates/` that a Rust test reads (RUST_READS, the same trees as the `rust` filter in
# .github/workflows/ci.yml) selects each package whose sources name that file or its directory.
#
# `-p` resolves features over the selected packages only, so a test gated on a feature that another
# member turns on can be absent here and present under `--workspace` (see the self dev-dependencies
# in crates/mosaica-engine/Cargo.toml). CI runs the whole workspace on every pull request.
set -euo pipefail

cd "$(dirname "$0")/.."

dry=0
if [ "${1:-}" = "-n" ]; then dry=1; shift; fi
if [ "${1:-}" = "--" ]; then shift; fi

WORKSPACE_CONFIG='^(Cargo\.toml|Cargo\.lock|rust-toolchain\.toml|\.cargo/.*|\.config/nextest\.toml)$'
RUST_READS='^(docs/reference|docs/openapi|corpora|examples|reference/vectors)/'

base_ref=${BASE:-origin/main}
if ! base=$(git merge-base "$base_ref" HEAD 2>/dev/null); then
  echo "test-affected: no merge base with $base_ref; fetch it, or name another ref in BASE" >&2
  exit 1
fi

changed=$( { git diff --name-only --no-renames "$base"; git ls-files --others --exclude-standard; } \
  | sort -u)
echo "test-affected: $(printf '%s' "$changed" | grep -c . || true) files changed since $base_ref ($(git rev-parse --short "$base"))"

config=$(printf '%s\n' "$changed" | grep -E "$WORKSPACE_CONFIG" || true)
if [ -n "$config" ]; then
  echo "test-affected: the workspace's configuration changed, so every package is tested:"
  printf '  %s\n' $config
  command=(cargo nextest run --workspace --no-fail-fast "$@")
  echo "test-affected: ${command[*]}"
  [ "$dry" = 1 ] || exec "${command[@]}"
  exit 0
fi

# Each file under RUST_READS is followed to the crate sources that name it (a Python module also by
# its dotted name), or, where none does, to those that name its directory.
reads=""
for file in $(printf '%s\n' "$changed" | grep -E "$RUST_READS" || true); do
  names=(-e "$file")
  case "$file" in *.py) names+=(-e "$(echo "${file%.py}" | tr / .)") ;; esac
  sources=$(git grep -l -F "${names[@]}" -- crates || git grep -l -F "$(dirname "$file")/" -- crates || true)
  for source in $sources; do reads+="$source $file"$'\n'; done
done

packages=$(cargo metadata --format-version 1 --no-deps | CHANGED="$changed" READS="$reads" python3 -c '
import json, os, sys

meta = json.load(sys.stdin)
root = meta["workspace_root"] + "/"
dirs = {p["name"]: os.path.dirname(p["manifest_path"])[len(root):] + "/" for p in meta["packages"]}

def owner(path):
    found = [name for name, d in dirs.items() if path.startswith(d)]
    return max(found, key=lambda name: len(dirs[name])) if found else None

why = {}
for path in os.environ["CHANGED"].split():
    name = owner(path)
    if name:
        why.setdefault(name, f"changed: {path}")
for line in os.environ["READS"].splitlines():
    source, read = line.split(" ", 1)
    name = owner(source)
    if name:
        why.setdefault(name, f"{source} names {read}")

# A library is affected through normal and build edges, and the tests of a package also through
# its dev-dependencies, which are not compiled into anything that depends on it.
normal, dev = {}, {}
for p in meta["packages"]:
    for d in p["dependencies"]:
        if d.get("path") and d["name"] != p["name"]:
            (dev if d["kind"] == "dev" else normal).setdefault(d["name"], set()).add(p["name"])
libs = list(why)
for name in libs:
    for user in sorted(normal.get(name, ())):
        if user not in why:
            why[user] = f"depends on {name}"
            libs.append(user)
for name in list(libs):
    for user in sorted(dev.get(name, ())):
        why.setdefault(user, f"dev-depends on {name}")
for name in sorted(why):
    print(name, why[name])
')

if [ -z "$packages" ]; then
  echo "test-affected: no change reaches a Rust package; nothing to test"
  exit 0
fi

echo "test-affected: testing $(printf '%s\n' "$packages" | wc -l | tr -d ' ') packages:"
printf '%s\n' "$packages" | sed 's/^\([^ ]*\) /  \1: /'
selection=()
for name in $(printf '%s\n' "$packages" | cut -d' ' -f1); do selection+=(-p "$name"); done
command=(cargo nextest run --no-fail-fast "${selection[@]}" "$@")
echo "test-affected: ${command[*]}"
[ "$dry" = 1 ] || exec "${command[@]}"
