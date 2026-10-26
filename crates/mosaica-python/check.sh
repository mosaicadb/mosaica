#!/usr/bin/env bash
# The extension module's own gate: build it, import it, and run `tests/binding.py` against it.
#
# Separate from `cargo test` because a Python extension module has no Rust test target — the
# object is linked with libpython's symbols left undefined, which is what an interpreter resolves
# at import and what an executable cannot be linked with. So the module's behaviour is tested
# where it is used, from Python.
#
# No wheel and no packaging: the object is copied to `_mosaica.so` in a scratch directory and put
# on `PYTHONPATH`. The build is the one `clients/py-native`'s hook runs, the release `mosaica`
# binary and module with symbols stripped, so after `clients/py/check.sh` it recompiles only pyo3
# and what links it, for the interpreter, which differs from the hook's build environment. The
# tests compare the module's page with that binary's, unless MOSAICA_BIN names another. The
# interpreter is `clients/py/.venv`'s, made by `clients/py/venv.sh` where it is absent. Needs a
# cargo toolchain, and Python 3.12 or later through uv or `python3`.
set -euo pipefail

cd "$(dirname "$0")/../.."

source clients/py/venv.sh

echo "check-python-binding: building"
CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --release -p mosaica -p mosaica-python

out="${CARGO_TARGET_DIR:-target}/release"
lib=$(find "$out" -maxdepth 1 -name 'lib_mosaica.so' -o -maxdepth 1 -name 'lib_mosaica.dylib' \
      | head -n 1)
if [ -z "$lib" ]; then
  echo "check-python-binding: no extension module was built" >&2
  exit 1
fi
export MOSAICA_BIN="${MOSAICA_BIN:-$out/mosaica}"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$lib" "$stage/_mosaica.so"

echo "check-python-binding: $("$py" -V), module $(du -h "$stage/_mosaica.so" | cut -f1)"
PYTHONPATH="$stage" "$py" -m unittest discover -s crates/mosaica-python/tests -p 'binding.py' -v
