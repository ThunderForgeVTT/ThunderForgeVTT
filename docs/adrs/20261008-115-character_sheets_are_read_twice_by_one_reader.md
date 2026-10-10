# ADR-115: Character Sheets Are Read Twice, by One Reader

**Date:** 2026-10-08
**Status:** Accepted (2026-10-10)
**Participants:** ThunderForgeVTT Team
**Related:** spec 048 (FR-001, FR-001a, FR-003, FR-005, FR-011, FR-030 to FR-037), spec 049 (reading a source book in the browser), [ADR-113](./20261006-113-the_rules_of_a_fight_are_one_crate.md) (one crate, two targets), [ADR-062](./20260902-062-packs_extend_the_engine_with_data_not_code.md) (packs extend the engine with data, not code)

---

## Problem Statement

A player brings a character into a world from an exported character sheet,
D&D Beyond's PDF first. Three questions decide the shape of the work:

1. **Where is the sheet read?** The player needs to review what was read
   before anything is written, and the review has to be quick. The server
   has to trust what it writes onto an actor the GM runs, and the file the
   instance keeps has to be the file that was read.
2. **Where does "a character" live?** The PDF crate must stay ignorant of
   character sheets (FR-001a), and the importer must stay ignorant of 5e
   (FR-011). A second system should add a reader and a declaration and
   touch nothing else.
3. **Where does content the world lacks go?** A spell or feature the world
   does not have arrives with the character. Play must never use it before
   the GM has said yes, and no query should have to remember that.

## Decision

1. **One reader runs in two places.** A pack's `sheet/` crate is compiled to
   wasm for the browser and linked natively by the server.
   - *Review:* the browser reads the PDF and sends the server the reading
     (the neutral character as JSON) and the player's corrections.
     `sheetImportPreview` plans the reading onto the actor and returns the
     plan with its hash. It writes nothing, and the file is not uploaded.
   - *Accept:* `applySheetImport` uploads the file. The server reads it
     again with the same crate, applies the same corrections, and plans
     again. It writes only when the plan's hash equals the hash the player
     reviewed; otherwise it refuses with `PLAN_CHANGED` and writes nothing.
   - *Decline:* nothing is uploaded and nothing is written.

2. **The neutral character and the mapping engine live in a new crate,
   `crates/thunderforge-sheet-import`.** It holds `ImportedCharacter`, in
   which every leaf is a `Field{value, certainty, source}`; the
   `SheetReader` trait; and `plan`, which maps a reading onto an actor from
   the pack's `sheetImport` declaration, with the pack's refine hook last.
   It depends on `thunderforge-pdf` and names no system. Canvas core gains a
   `SheetImport` slot that speaks bytes in and JSON out, so a pack that never
   reads a sheet does not compile a PDF parser.

3. **Staged content is a table of its own, `world_staged_content`.** A link
   from an actor points at world content or at a staged row, never both. Play
   joins world content by id, so it never sees a staged link. Adopting a piece
   inserts the world row with `origin = 'Uploaded'` and repoints every link to
   it, in one transaction.

### Y-statement

In the context of **bringing a character in from an exported sheet**,
facing **a review that must be fast, a write that must be trusted, and a
kept file that must be the file that was read**,
we decided for **one reader compiled for the browser and the server, a
neutral character in its own crate, and staged content in its own table**,
and against **uploading for review, trusting the browser's reading, two
parsers, and a staged flag on the world's content tables**,
to achieve **a review that costs the instance nothing, a write that only
ever matches what the player saw, and play that cannot see undecided
content by construction**,
accepting **two reads of every accepted sheet, a hash check that can ask a
player to review again after a version skew, and a second wasm package per
system that reads sheets**.

## Alternatives Rejected

- **Upload first and read on the server for review as well.** It keeps a
  personal document for a decline that never wanted it kept, and needs a
  sweep for files from abandoned reviews.
- **Trust the browser's reading, as book import does.** A book is the GM's
  own library. A character sheet writes onto an actor the GM runs, and
  FR-001 asks for a server reading.
- **Two parsers, one TypeScript and one Rust.** They would drift; the spec
  asks for one parser.
- **The neutral types in `thunderforge-content`.** That is the book
  importer's model of entries and prose. A character is a different shape,
  and each crate's changes would become the other's risk.
- **The neutral types in `thunderforge-canvas-core`.** The reader trait
  depends on the PDF crate, and canvas core must not pull a PDF parser into
  every pack.
- **A `staged` flag on `world_abilities` and `world_items`.** Every query
  that reads them would need a new `WHERE`, and the one that is missed leaks
  a declined spell to the table. A separate table is default-deny.

## Consequences

- A system that wants sheet import adds a `sheet/` crate and a `sheetImport`
  block in `system.json`. `scripts/check-packs.mjs` accepts `sheet/`, and
  `scripts/shared.mjs` builds every pack's reader into `dist/sheet-<id>`.
- The PDF crate gains a region API (`PageText`), size and page bounds, and
  refuses encrypted files. It still names no game.
- An accepted import reads the file twice. A reader version change between
  the two reads shows up as `PLAN_CHANGED`, never as a silent difference.
- The GM's "Brought by players" queue reads `world_staged_content` only;
  nothing in play joins it.

## What Was Built (2026-10-10)

Spec 048 built the decision as written.

- `crates/thunderforge-sheet-import` holds `ImportedCharacter`, the
  `SheetReader` trait, `plan` and `plan_hash`. Canvas core's `SheetImport`
  slot carries a pack's readers and its `refine` hook.
- Two readers: `ddb-pdf` (`packs/systems/dnd5e/sheet`, D&D Beyond's export)
  and `tf-rfs-pdf` (`packs/systems/roll_for_shoes/sheet`). Each is built to
  wasm as `dist/sheet-<system>` and linked natively by the server.
- `sheetImportPreview` plans without writing; `applySheetImport` uploads,
  reads again and writes only on a matching plan hash, else `PLAN_CHANGED`.
- `world_staged_content` holds undecided content, one row per world, kind,
  normalised name and content hash. Play joins world content only; a use of
  a staged piece is refused and reported to the GM.
- The file is kept per version (`sheet_import_versions`), downloadable by
  its uploader and the world's GM, carried in the account's export, and
  deleted with the account.
- `feature.sheet_import` gates the preview and the apply only; decisions,
  rollbacks and downloads stay available when it is off.
- On the owner's seven real exports, the reader read every sheet, with no
  uncertain leaf after one fix (two casting classes in one column).
