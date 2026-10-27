# Sourced by clients/py/check.sh and crates/mosaica-python/check.sh, which share one venv: makes
# clients/py/.venv on the first run, with a Python 3.12 or later from uv where uv is installed and
# from `python3` otherwise, and sets `py` to its interpreter. Set MOSAICA_CHECK_FRESH=1 to rebuild
# it.

venv="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/.venv"
if [ -n "${MOSAICA_CHECK_FRESH:-}" ]; then rm -rf "$venv"; fi

py="$venv/bin/python"
if [ ! -x "$py" ]; then
  echo "making clients/py/.venv"
  if command -v uv >/dev/null 2>&1; then
    uv venv -q --python '>=3.12' "$venv"
  else
    python3 -m venv "$venv"
  fi
fi
if ! "$py" -c 'import sys; sys.exit(sys.version_info < (3, 12))'; then
  echo "clients/py/.venv is $("$py" -V), and mosaica needs Python 3.12 or later. Remove" >&2
  echo "  clients/py/.venv and re-run with a Python 3.12 or later as python3, or with uv." >&2
  exit 1
fi
