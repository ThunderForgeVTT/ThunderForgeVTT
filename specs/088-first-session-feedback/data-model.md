# Data Model: First Session Feedback

Three additive migrations, one new world event, one wall mark, and two
client-side models. Nothing is dropped or rewritten.

## Migrations

### M1 `world_invites_default_one` (US1, FR-012)

```sql
-- up
ALTER TABLE world_invites ALTER COLUMN max_uses SET DEFAULT 1;
-- down
ALTER TABLE world_invites ALTER COLUMN max_uses SET DEFAULT 0;
```

No row changes. Links made before this keep their uses and expiry
(Open item 7). `code` keeps its type (`TEXT`): the new codes are 26
characters, the old ones 20.

### M2 `base_map_assets` (US2, FR-029)

```sql
-- up
ALTER TABLE canvas_image_assets ADD COLUMN base_map_id TEXT NULL;
-- down
ALTER TABLE canvas_image_assets DROP COLUMN base_map_id;
```

- Set only when a background is made from a base map. The value is the
  map's `id` in `maps.json` (for example `grassy-path-ambush`).
- No foreign key: the base maps are files, not rows. An id the directory
  no longer holds still resolves its credit, because the credit is one
  record for every map (`examples/maps/credit.json`).

### M3 `rolls_cleared_at` (US5, FR-040)

```sql
-- up
ALTER TABLE worlds ADD COLUMN rolls_cleared_at TIMESTAMPTZ NULL;
-- down
ALTER TABLE worlds DROP COLUMN rolls_cleared_at;
```

`NULL` means never cleared. A clear sets it to the transaction's `now()`.
A later clear moves it forward. Nothing moves it back.

`schema.rs` is regenerated after each migration.

## World event 39: `ROLLS_CLEARED` (US5)

| Field | Value |
| --- | --- |
| `event_code` | `39` (`EVENT_CODE_ROLLS_CLEARED` in `world_events.rs`) |
| `world_id` | the world |
| `created_by` | the GM who cleared |
| payload | `{ "clearedAt": "<RFC 3339>" }` |

It reaches every member, with no visibility filter. A client drops every
roll entry whose `createdAt <= clearedAt`.

### The clear rule (`rolls/visibility.rs`)

```text
cleared(roll.created_at, world.rolls_cleared_at) =
    rolls_cleared_at is set and roll.created_at <= rolls_cleared_at
```

Applied first, before `view_of` and `event_reaches`, on every path:

| Path | A cleared roll |
| --- | --- |
| `worldRolls`, `worldRollRecords` | left out of the list |
| `worldRoll(id)` | `null` |
| live subscription | its `ROLL_MADE` / `ROLL_REVEALED` event is not delivered |
| catch-up since an event id | the same |
| `revealRoll`, re-roll | refused: "That roll was cleared." |

The GM and site admins are not exempt.

## Perimeter wall (US6)

An ordinary `walls` row:

| Column | Value |
| --- | --- |
| `level_id` | the imported level |
| `x1, y1, x2, y2` | along one edge of the map's rectangle, scene px, centred, y up |
| `blocks_vision`, `blocks_movement` | `true` |
| `door_state` | `none` |
| `metadata` | `{ "perimeter": true }` |

The mark has one use: a later import or background change on the same
level finds the old perimeter by it. A marked wall is replaced only if it
still lies on the old bounds (both ends within 0.5 px of the old
rectangle's edge). A wall the GM moved is left alone. Editing a wall in
the wall tool keeps `metadata` as it is.

### `perimeter_walls` (pure)

```text
perimeter_walls(placement: ScenePlacement, existing: &[WallInsert]) -> Vec<WallInsert>

edges = top, right, bottom, left of [-w/2, w/2] × [-h/2, h/2]
for each edge:
    covered = intervals along the edge of each existing wall
              whose two ends are within ε = 0.5 px of the edge's line
    merge covered; clamp to the edge
    emit one wall per gap longer than ε
```

Its test fixtures (`perimeter_tests.rs`, mirrored in the demo):

| Case | Expected |
| --- | --- |
| 48 × 27 cells, no walls (the ambush map) | 4 walls, the four full edges |
| a file wall along the whole top edge | 3 walls |
| a file wall along the middle third of the left edge | 5 walls: left split in two |
| a wall crossing the edge at an angle | 4 walls, as if absent |
| two overlapping file walls along one edge | merged; no overlap |
| a wall 0.3 px inside the edge, parallel | counts as covering |
| a wall 2 px inside the edge, parallel | does not cover |

## Base map (US2), served, not stored

From the base-maps directory's `maps.json` (contracts/base-maps.md):

```ts
type BaseMap = {
  id: string;            // file stem, e.g. "grassy-path-ambush"
  name: string;          // "Grassy Path Ambush"
  width: number;         // px
  height: number;        // px
  gridSize: number;      // px per cell
  image: string;         // "<id>.webp"
  thumbnail: string;     // "<id>.thumb.webp"
  walls: WallJson[];     // perimeter included, marked
  lights: LightJson[];
};

type MapCredit = {
  author: "MBRound18";
  licence: "CC BY-SA 4.0";
  licenceUrl: "https://creativecommons.org/licenses/by-sa/4.0/";
  source: "https://github.com/mbround18/vtt-maps";
  catalog: "https://vtt-maps.dnd-apps.dev/catalog";
  shareAlike: "The copies here are offered under the same licence.";
};
```

## Settings form model (US7, client only)

`apps/web/src/pages/admin/settingsForm.ts`, pure:

```ts
type Baseline = Record<Key, { value: string | null; secret: boolean; fixed: boolean }>;
type Change = { kind: "set"; value: string } | { kind: "clear" };

type FormState = {
  baseline: Baseline;
  changes: Partial<Record<Key, Change>>;
  errors: Partial<Record<Key, string>>;
};

edit(state, key, draft): FormState    // dirty iff trim(draft) !== baseline; secret: dirty iff draft !== ""
clear(state, key): FormState
discard(state): FormState
dirtyKeys(state, order: Key[]): Key[] // form order, "mail.enabled" moved last
canSave(state): boolean               // ≥1 dirty, no client error
applyResults(state, results): FormState   // saved → new baseline; refused → stays dirty with error
rebase(state, freshBaseline): FormState   // keep only the keys still dirty
```

`useUnsavedChanges(dirty: boolean, message?: string)` registers
`beforeunload` while `dirty`, and intercepts in-app link clicks and the
settings page's section switches with a confirm dialog.
