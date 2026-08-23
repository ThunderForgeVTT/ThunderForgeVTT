#!/usr/bin/env bash
# Fails if any tracked Rust source file exceeds MAX_LINES, except files in
# EXEMPT (generated code or pure data-struct files, where "split it" buys
# nothing — see the discussion that introduced this script: a 1000+-line
# file full of Diesel Queryable/Insertable structs isn't a testability
# problem the way a 1000+-line file full of logic is).
#
# Clippy has no "file too long" lint (only `too-many-lines-threshold`, which
# caps individual *function* length, configured in clippy.toml) — this
# script is the file-level counterpart.
set -euo pipefail

MAX_LINES=1000

EXEMPT=(
  "src/server/src/schema.rs"   # diesel print_schema output — regenerated, never hand-edited
  "src/server/src/models.rs"   # Queryable/Insertable structs only, no logic to split out
)

cd "$(git rev-parse --show-toplevel)"

is_exempt() {
  local f="$1"
  for e in "${EXEMPT[@]}"; do
    [[ "$f" == "$e" ]] && return 0
  done
  return 1
}

failed=0
while IFS= read -r -d '' file; do
  is_exempt "$file" && continue
  [[ -f "$file" ]] || continue  # tracked but deleted-in-worktree (not yet staged)
  lines=$(wc -l < "$file")
  if [[ "$lines" -gt "$MAX_LINES" ]]; then
    echo "FAIL: $file has $lines lines (max $MAX_LINES) — split it into a module directory (see auth/, map_import/ for the established pattern)"
    failed=1
  fi
done < <(git ls-files -z -- '*.rs')

if [[ "$failed" -eq 0 ]]; then
  echo "OK: no tracked .rs file exceeds $MAX_LINES lines (outside the exempt list)."
fi

exit "$failed"
