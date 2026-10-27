#!/usr/bin/env bash
# scripts/install-hooks.sh — install the pre-commit check. Run once per clone:
#
#     bash scripts/install-hooks.sh
#
# The check refuses stray reformats (`check-stray-reformat.sh`): `rustfmt <file>` follows the
# file's `mod` declarations and rewrites everything they reach, silently.
#
# Re-run the installer after the check changes: the hook is a copy, not a link. Hooks are shared
# across worktrees, so one install serves every branch.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
common_dir="$(git rev-parse --git-common-dir)"
case "$common_dir" in
  /*) ;;
  *) common_dir="$repo_root/$common_dir" ;;
esac

hooks_path="$(git config --get core.hooksPath || true)"
if [ -n "$hooks_path" ]; then
  case "$hooks_path" in
    /*) hooks_dir="$hooks_path" ;;
    *) hooks_dir="$repo_root/$hooks_path" ;;
  esac
  echo "note: core.hooksPath is set; installing into $hooks_dir"
else
  hooks_dir="$common_dir/hooks"
fi

mkdir -p "$hooks_dir"
hook="$hooks_dir/pre-commit"

if [ -e "$hook" ] && ! grep -q 'Installed by scripts/install-hooks.sh' "$hook" 2>/dev/null; then
  echo "FAIL: $hook already exists and is not this hook — move it aside and re-run" >&2
  exit 1
fi

cat > "$hook" <<'HOOK'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh. Re-run that after changing the check.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
reformat="$repo_root/scripts/check-stray-reformat.sh"
if [ -f "$reformat" ]; then
  bash "$reformat" --staged
fi
HOOK
chmod +x "$hook"

echo "ok: installed $hook"
echo "     - scripts/check-stray-reformat.sh"
