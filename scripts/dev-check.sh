#!/usr/bin/env bash
set -euo pipefail

# Simple developer sanity check:
# - format
# - basic build
# - tests

echo "==> cargo fmt"
cargo fmt

echo "==> cargo build"
cargo build

echo "==> cargo test"
cargo test

echo
echo "✅ dev-check: ok"
