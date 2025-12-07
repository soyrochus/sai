#!/usr/bin/env bash
set -euo pipefail

# Pre-publish checklist for sai-cli.
# This script DOES NOT call `cargo publish` – that remains a manual step.

CRATE_NAME="sai-cli"

echo "==> Pre-publish check for crate: ${CRATE_NAME}"
echo

# 1. Basic git hygiene (optional but very useful)
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "==> Checking git status"
  if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "⚠️  Working tree is not clean."
    echo "    Commit or stash changes before publishing."
    echo
  fi

  CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
  echo "Current branch: ${CURRENT_BRANCH}"
  echo
fi

# 2. Show crate version from Cargo.toml
echo "==> Crate version"
grep -E '^\s*version\s*=' Cargo.toml || echo "Could not detect version line"
echo

# 3. Formatting
echo "==> cargo fmt"
cargo fmt

# 4. Build & tests
echo "==> cargo build --release"
cargo build --release

echo "==> cargo test"
cargo test

# 5. Clippy (treat warnings as errors here)
if command -v cargo-clippy >/dev/null 2>&1 || cargo clippy -V >/dev/null 2>&1; then
  echo "==> cargo clippy -- -D warnings"
  cargo clippy -- -D warnings
else
  echo "⚠️  cargo clippy not installed; skipping clippy step."
fi

# 6. Check what will be packaged
echo "==> cargo package"
cargo package

# 7. Dry-run publish (no upload)
echo "==> cargo publish --dry-run"
cargo publish --dry-run

echo
echo "✅ pre-publish checks passed."
echo
echo "Next steps:"
echo "  1) Review git status & tags if needed."
echo "  2) When ready, publish manually:"
echo
echo "     cargo publish"
echo
