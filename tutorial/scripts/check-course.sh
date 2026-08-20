#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
chapters="$root/tutorial/chapters"

for number in $(seq -w 1 14); do
  file="$(find "$chapters" -maxdepth 1 -type f -name "${number}-*.md" -print -quit)"
  if [[ -z "$file" ]]; then
    echo "missing chapter $number" >&2
    exit 1
  fi
  for heading in "Product goal" "Rust concepts" "AI collaboration script" "Build" "Compiler conversation" "Tests" "Review checklist" "Checkpoint" "Stretch exercise" "Reflection"; do
    if ! grep -q "^## $heading" "$file"; then
      echo "$(basename "$file"): missing '$heading'" >&2
      exit 1
    fi
  done
done

for file in "$root/tutorial/README.md" "$root/tutorial/checkpoints.md" "$root/tutorial/prompts/README.md" "$root/tutorial/exercises/README.md" "$root/tutorial/troubleshooting.md"; do
  test -s "$file" || { echo "missing or empty: $file" >&2; exit 1; }
done

echo "Rust in the Loop course structure is complete."
