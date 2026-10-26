#!/usr/bin/env bash
# The extension module's own gate: build it, import it, and run `tests/binding.py` against it.
#
# Separate from `cargo test` because a Python extension module has no Rust test target — the
# object is linked with libpython's symbols left undefined, which is what an interpreter resolves
# at import and what an executable cannot be linked with. So the module's behaviour is tested
# where it is used, from Python.
#
# No wheel and no packaging: the object is copied to `_mosaica.so` in a scratch directory and put
# on `PYTHONPATH`. Needs Python >= 3.12 and a cargo toolchain, nothing else.
set -euo pipefail

cd "$(dirname "$0")/../.."

if ! python3 -c 'import sys; sys.exit(sys.version_info < (3, 12))'; then
  echo "check-python-binding: python3 is $(python3 -V), and the module needs Python 3.12 or later." >&2
  echo "  Put a Python 3.12 or later first on PATH as python3 and re-run." >&2
  exit 1
fi

echo "check-python-binding: building"
cargo build -p mosaica-python

lib=$(find "${CARGO_TARGET_DIR:-target}/debug" -maxdepth 1 -name 'lib_mosaica.so' -o \
      -maxdepth 1 -name 'lib_mosaica.dylib' | head -n 1)
if [ -z "$lib" ]; then
  echo "check-python-binding: no extension module was built" >&2
  exit 1
fi

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$lib" "$stage/_mosaica.so"

echo "check-python-binding: $(python3 -V), module $(du -h "$stage/_mosaica.so" | cut -f1)"
PYTHONPATH="$stage" python3 -m unittest discover -s crates/mosaica-python/tests -p 'binding.py' -v
