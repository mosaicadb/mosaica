# Contributing to Mosaica

Thank you for considering a contribution. This page says how to build Mosaica, how to run the
checks a change must pass, and how to propose the change.

## What you need

- Rust stable (`rust-toolchain.toml` names the channel).
- Node 22.15 or later, for the TypeScript packages and the Python wheel's build hook.
- Python 3.12 or later, with `uv` or `venv`.
- Tens of gigabytes of free disc for a full test build. A build without debug info is much
  smaller: set `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`.

## Building

```bash
cargo build --release -p mosaica    # the binary, at target/release/mosaica
npm --prefix clients/ts ci          # the TypeScript workspace
```

`clients/py/check.sh` makes its own virtualenv and builds the Python packages, including the
`mosaica-native` wheel, which runs cargo.

## The checks

While you work, and before you open a pull request, run the checks your change reaches:

```bash
bash scripts/test-affected.sh                      # the Rust tests the change reaches
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/check-layers.sh
bash scripts/check-test-reachability.sh --quick    # a test's attributes, a feature or a dev-dependency changed
bash scripts/check-clients.sh                      # clients/ts or the HTTP API changed
bash clients/py/check.sh                           # clients/py, clients/py-native or the HTTP API changed
bash scripts/check-docs.sh                         # docs/ or mkdocs.yml changed
```

`scripts/test-affected.sh` compares your branch and its uncommitted files with `origin/main` (set
`BASE` to compare with another ref) and runs `cargo test --no-fail-fast` on each package you
changed and each package that depends on one of them, printing which it chose and why. It tests the
whole workspace when `Cargo.toml`, `Cargo.lock`, `.cargo/` or `rust-toolchain.toml` changed, and
nothing when no Rust did. With `-n` it prints the choice and runs nothing.

A selection with `-p` resolves features over the selected packages only, so it can compile fewer
tests than `cargo test --workspace`. The continuous integration workflow
(`.github/workflows/ci.yml`) therefore runs the whole workspace's tests on every pull request,
with clippy, every script above, the conformance suite and `cargo deny`, and skips only the jobs
the change cannot reach. A change must pass it before it is merged. When you run the workspace's
tests yourself, use `cargo test --workspace --no-fail-fast` and read the totals, because with
`--no-fail-fast` every test binary runs and a failure in an early binary does not hide the count of
the rest.

`check-clients.sh` needs Playwright's Chromium (`cd clients/ts && npx playwright install
chromium`), and `check-docs.sh` needs the documentation's Python packages, which its header says
how to install.

The components' browser suite, which `check-clients.sh` runs, checks the explorer in each of its
states for accessibility: every axe-core rule for WCAG 2.2 A and AA, the contrast of text in both
colour schemes, text cut off at 320 px wide or under wider text spacing, pointer targets under
24 px that crowd another, and that every Tab stop is visible and shows the focus. A new control
needs a name a screen reader can read, a 24 px target and a test beside its element. If it appears
in a state the suite does not build, add that state to
`clients/ts/components/test/browser/accessibility-page.ts`.

`bash scripts/install-hooks.sh` installs a pre-commit hook that refuses a commit in which `rustfmt`
reformatted files the change did not touch.

## How the code is organised

[CLAUDE.md](CLAUDE.md) describes the layout, the security boundary and the conventions for code,
comments and tests. [docs/system/overview.md](docs/system/overview.md) describes the system and
[docs/writing.md](docs/writing.md) is the style for prose, in British spelling. A change that
alters what a viewer can see must keep every claim in
[docs/dev/threat-model.md](docs/dev/threat-model.md), the threat model.

## Proposing a change

1. Open an issue first for anything larger than a small fix, so the approach can be agreed before
   the work.
2. Make the change on a branch, with tests through the public surface: what is stored, what is
   served, and what survives a restart.
3. A capability added to one surface (the HTTP API, the TypeScript client, the Python client or the
   command line) is added to all four, starting with the HTTP API.
4. Open a pull request that says what changed and why, and which checks you ran.

Report a suspected vulnerability privately, as [SECURITY.md](SECURITY.md) describes, and not in an
issue.

Contributions are licensed under the Apache License 2.0, the licence of the project.
