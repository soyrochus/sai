#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
chapters="$root/tutorial/src/chapters"

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

for file in "$root/tutorial/book.toml" "$root/tutorial/src/SUMMARY.md" "$root/tutorial/src/README.md" "$root/tutorial/src/assistant-setup.md" "$root/tutorial/src/checkpoints.md" "$root/tutorial/src/prompts/README.md" "$root/tutorial/src/exercises/README.md" "$root/tutorial/src/troubleshooting.md" "$root/tutorial/src/divergences.md" "$root/tutorial/src/instructor-guide.md" "$root/tutorial/src/proposal.md"; do
  test -s "$file" || { echo "missing or empty: $file" >&2; exit 1; }
done

if command -v mdbook >/dev/null 2>&1; then
  (cd "$root/tutorial" && mdbook build) || { echo "mdbook build failed" >&2; exit 1; }
else
  echo "mdbook not installed; skipping build check" >&2
fi

echo "Rust in the Loop course structure is complete."
