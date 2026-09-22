# ADR-106: A Stored Image Remembers the Spec That Drew It

**Date:** 2026-09-21
**Status:** **ACCEPTED** 2026-09-21, once spec 044 Phase 6 proved it (tasks T087, T090, T096 and T097: the server rules for B7 and B8, the copy and export rules for B9, the saved-look e2e, and the proof run). Proposed 2026-09-21 with spec 044 phase (d). Extends ADR-057.
**Participants:** ThunderForgeVTT Team
**Related:** spec 044 (FR-035–FR-040, SC-011; research R4; contracts §5 B5a, B7, B8, B9), ADR-057 (actor imagery as rows keyed by role), ADR-105 (who may change a character's look)

---

## Problem Statement

Spec 044's builder draws a portrait and a token from a hero spec, and the
server stores what it is sent: an SVG, rasterised to WebP. The spec itself is
lost at that moment. Re-opening the builder on a saved NPC therefore starts
from nothing, or from a guess made from the actor's name, and a Game Master
who wanted to change one hat has to rebuild the whole face.

Phase (d) keeps the spec, so the builder re-opens with every choice intact
(FR-036), and the spec travels wherever the actor does (FR-039, FR-040). That
raises three questions: where the spec lives, what the server lets into it,
and what storing it does to the catalogue it names.

## Decision

1. **The spec hangs on the image row.** `world_actor_images` gains
   `hero_spec JSONB NULL`, meaning "this image was drawn from this spec". It
   is written by the same upsert that stores the image, through an optional
   `heroSpec: JSON` argument on `uploadActorImage`, and read back as
   `GraphQLActorImage.heroSpec`.

2. **An upload without a spec has none (B8).** Omitting the argument writes
   NULL. Replacing a built token with an uploaded file therefore clears the
   token's spec in the same write, and the builder can say truthfully that the
   token no longer comes from a built hero.

3. **The server checks the spec at the door (B7).** Before anything is
   transcoded or written, the spec must be a JSON object, at most 4 KB
   serialised, and valid against `HERO_SPEC_SCHEMA`
   (`src/server/src/heroes/spec_schema.rs`). A refusal names what was wrong —
   each problem by field — carries a stable `reason` extension
   (`HERO_SPEC_NOT_OBJECT`, `HERO_SPEC_TOO_LARGE`, `HERO_SPEC_INVALID`), and
   stores neither the spec nor the image.

4. **The schema is the package's, vendored and held to it.**
   `packages/heroes` builds `HERO_SPEC_SCHEMA` (JSON Schema, draft 2020-12)
   from the same lists `validateHero` reads. The server keeps a copy,
   `src/server/src/heroes/hero_spec_schema.json`, written by
   `pnpm -F @thunderforge/heroes run schema`; the package's `schema.test.ts`
   fails, inside `pnpm verify`, whenever the copy differs from what the
   package would write. The crate builds without node, and the copy cannot
   drift. Where JSON Schema measures text differently from `validateHero`
   (code points against UTF-16 code units), the server measures `name` and
   `title` again the client's way, so nothing the client refuses is stored.

5. **The race a roll used is never stored (B5a).** It is a roll setting, not
   part of the look. The schema's `additionalProperties: false` refuses a
   spec that carries it, so the rule is enforced rather than hoped for.

6. **The catalogue becomes append-only (FR-038).** A stored spec names parts
   and choices. `packages/heroes/src/shipped.json` records every field (with
   its kind) and every choice ever shipped, and `shipped.test.ts` fails when
   one is renamed, removed or changes kind. A new choice is welcome and is
   appended to the record in the same change; a choice may change how it
   draws, because only names are recorded.

7. **The spec travels with its image (B9).** A collection copy selects and
   inserts `hero_spec` with each image row, and the copy then owns its own
   rows, so rebuilding the look in the destination changes the copy alone. A
   personal export carries it on each exported image. A shared-actor copy
   carries no images, and so no specs.

## Rationale

- **A spec describes an image, not an actor.** On the image row the spec and
  the picture cannot disagree: whatever replaces one replaces or clears the
  other in the same statement.
- **It keeps ADR-057's bargain.** Imagery-related data stays on imagery rows,
  and `world_actors` stays free of presentation columns.
- **It needs no new permission.** Writing the spec is writing the image, under
  the Editor gate or ADR-105's holder right.
- **The server is the data boundary** (Constitution Principle III). A JSONB
  column takes anything, and the spec arrives in a request anyone who may
  change the image can write by hand. The client's `validateHero` still runs
  before any stored spec is drawn (FR-037), because a spec valid when stored
  can outlive a catalogue change despite Decision 6.

## Consequences

- **Duplication, accepted.** The usual save stores one spec twice, on the
  portrait row and on the token row. A spec is well under a kilobyte, and the
  duplication is what lets the two roles diverge honestly.
- **Two places name the column.** `collections/copy.rs` and
  `users/export_content.rs` enumerate columns, so the column travels only
  because they name it; a future column on this table must be added there too.
- **The server never checks that a spec matches the image beside it**, and
  cannot. A mismatch needs a hand-made request from someone who could already
  upload any image, and it misleads only the builder's starting point.
- **Retiring a choice is now a migration question**, not an edit. What should
  happen to stored specs if one is ever retired despite FR-038 is left to the
  owner (spec 044 plan.md).
- Tests: `heroes/spec_schema_tests.rs` (B7, B8), the copy-and-export case in
  `collections/copy_asset_and_link_tests.rs` (B9), and `schema.test.ts` and
  `shipped.test.ts` in `packages/heroes`.

## Alternatives Considered

- **`hero_spec` on `world_actors`.** One spec per actor and the simplest read,
  but it silently disagrees with the images the moment either role is
  replaced by a file, and it puts presentation data on the actor row against
  ADR-057.
- **A `world_actor_hero_specs` table**, unique per actor. Leaves both tables
  alone, but has the same disagreement problem, another join, and another
  table for the collection copy to learn about.
- **Inside the uploaded SVG**, as a `<metadata>` element. No schema change,
  but the server rasterises and discards the SVG, so the spec would be lost at
  the moment it was meant to be kept.
- **Browser storage only.** No server change, but it is per device and per
  browser, gone on another machine, and cannot travel in a collection. That is
  not "saved".
- **Reading the schema from the package at build time.** Keeps one file, but
  makes the server crate's build depend on node and on the package's layout; a
  vendored copy held to the package by a test gives the same guarantee.
