#!/usr/bin/env bash
# Fails if any tracked Rust or TypeScript source file exceeds MAX_LINES,
# except files in EXEMPT (generated code or pure data-struct files, where
# "split it" buys nothing — see the discussion that introduced this script: a
# 1000+-line file full of Diesel Queryable/Insertable structs isn't a
# testability problem the way a 1000+-line file full of logic is).
#
# Clippy has no "file too long" lint (only `too-many-lines-threshold`, which
# caps individual *function* length, configured in clippy.toml) — this
# script is the file-level counterpart. TypeScript joined it on 2026-10-12.
set -euo pipefail

MAX_LINES=1000

EXEMPT=(
  "crates/thunderforge-server/src/schema.rs"   # diesel print_schema output — regenerated, never hand-edited
  "crates/thunderforge-server/src/models.rs"   # Queryable/Insertable structs only, no logic to split out
)

# Files that were already over the limit when TypeScript joined the gate. Each
# is a split owed, not an exemption: the list only shrinks. A file that drops
# under the limit fails the check until its entry is removed, so nothing
# stays here after it stops needing to.
PENDING=(
  "apps/web/src/pages/world/WorldPage.tsx"
  "apps/web/src/engine/bevy/index.ts"
  "packs/systems/dnd5e/web/src/ActorSheet.tsx"
  "apps/web/e2e/token-authoring.spec.ts"
  "apps/web/e2e/canvas-authoring.spec.ts"
  "apps/web/e2e/world-cache-isolated.spec.ts"
  "apps/web/e2e/library-deltas.spec.ts"
  "apps/web/src/pages/admin/PlayPausesPage.tsx"
  "packs/systems/roll_for_shoes/web/src/game.test.ts"
  "apps/web/playtest/combat.ts"
  "apps/demo/e2e/demo.spec.ts"
)

cd "$(git rev-parse --show-toplevel)"

listed() {
  local f="$1"
  shift
  for e in "$@"; do
    [[ "$f" == "$e" ]] && return 0
  done
  return 1
}

failed=0
while IFS= read -r -d '' file; do
  listed "$file" "${EXEMPT[@]}" && continue
  [[ -f "$file" ]] || continue  # tracked but deleted-in-worktree (not yet staged)
  lines=$(wc -l < "$file")
  if listed "$file" "${PENDING[@]}"; then
    if [[ "$lines" -le "$MAX_LINES" ]]; then
      echo "FAIL: $file is down to $lines lines — remove it from PENDING in scripts/check-file-length.sh"
      failed=1
    fi
    continue
  fi
  if [[ "$lines" -gt "$MAX_LINES" ]]; then
    echo "FAIL: $file has $lines lines (max $MAX_LINES) — split it by responsibility (Rust: a module directory, see auth/ and map_import/; TypeScript: sibling modules or hooks, see pages/world/)"
    failed=1
  fi
done < <(git ls-files -z -- '*.rs' '*.ts' '*.tsx' '*.mts' '*.cts')

for file in "${PENDING[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "FAIL: $file is in PENDING but no longer exists — remove its entry"
    failed=1
  fi
done

if [[ "$failed" -eq 0 ]]; then
  echo "OK: no tracked .rs/.ts/.tsx file exceeds $MAX_LINES lines (outside the exempt list; ${#PENDING[@]} splits owed)."
fi

exit "$failed"
