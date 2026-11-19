#!/bin/bash
# Scratch index directories for one dataset: DEST/def/<ds> (the default layout) and DEST/c/<ds> (layout (c)), each the
# suite's fit (its .npy files and meta.json symlinked, read-only) with the encoded index built there on first open.
# usage: bash scratch.sh DATASET [DEST]   (rm -rf DEST/def/DATASET DEST/c/DATASET when done)
set -euo pipefail
DS=$1
DEST=${2:-/home/user/scratch-sub}
FIT=/home/user/suite-data/work/$DS/vexact/index
for flavour in def c; do
  d=$DEST/$flavour/$DS
  mkdir -p "$d"
  for f in "$FIT"/*.npy "$FIT"/meta.json; do
    ln -sf "$f" "$d/$(basename "$f")"
  done
done
echo "$DEST/def/$DS $DEST/c/$DS"
