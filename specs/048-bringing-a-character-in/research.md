# Research: Bringing a Character In

Read on 2026-10-08 against `main` at `caafdd6d`. Every path below is the one
in that tree. Where the spec names a path that has since moved, the path here
is the right one. For example, the map importer lives at
`crates/thunderforge-server/src/map_import/`, not `src/server/src/map_import/`.

The spec's context section (lines 41-47) is older than specs 045, 046 and
084. The 5e pack already holds armour class, speed and senses, and the 5e
sheet can be edited. The gap this spec closes is the rest of the sheet, plus
the door into it.

---

## R1. Where the sheet is read: in the browser for review, on the server for the write

**Decision**: one reader runs in two places.

- **Review.** The browser reads the PDF with the reader compiled to wasm.
  It sends the server only the *reading*, which is the neutral
  `ImportedCharacter` as JSON, plus the player's corrections. The server's
  `sheetImportPreview` maps and resolves that reading and returns a plan.
  The preview writes nothing, and the file is not uploaded.
- **Accept.** `applySheetImport` uploads the file. The server reads it again
  natively with the same crate, applies the corrections and plans again. It
  writes only if the plan's hash matches the hash the player reviewed.
- **Decline.** Nothing is uploaded and nothing is written.

**Why**:

- FR-001 says the parser runs on the server. Spec 049's "Parsing in the
  browser" section shows that one crate can do both jobs.
- Q2=B keeps the file. A file that is kept must be the file that was read,
  so the server reads it again rather than trusting the reading the client
  sent.
- The review is fast because the read is local. The write is trustworthy
  because the server re-reads.
- The plan hash closes the gap between the two reads. If a player's client
  and the server disagree, because of a version skew or a crafted reading,
  the server refuses with "review again" and writes nothing. That honours
  SC-003: no value is written that the person was not shown.

**Alternatives rejected**:

- *Upload first and read on the server for review as well.* This keeps a
  personal document for a decline that never wanted it kept. It also needs
  an orphan sweep for files from abandoned reviews.
- *Trust the browser's reading, as book import does.* A book is the GM's own
  library. A character sheet writes onto an actor the GM runs, and FR-001
  asks for a server reading.
- *Two parsers, one TypeScript and one Rust.* They would drift. The spec
  says one parser.

## R2. What `thunderforge-pdf` already gives, and what it gains

**Reused as it is**:

- `Document::open` and `from_bytes`, the lopdf 0.34 `nom_parser` path, and
  `repair.rs` (`rebuild_xref`, `absorb_object_streams`). A DDB export reads
  through these today.
- `text.rs` `runs_on_page` and `TextRun{text,x,y,size,width,font,bold,italic}`,
  `font.rs` and the ToUnicode `cmap.rs`.
- `layout.rs` `lines()`, `reading_order`, `normalise` and `body_size`.
- `page.rs` `PageGeometry`.
- The wasm build: `buildPdf()` in `scripts/shared.mjs:341`, which produces
  `@thunderforge/pdf`, and the Dockerfile's pre-cook at lines 130-132.
- The diagnostic examples (`probe`, `peek`, `diagnose`, `survey`) for
  measuring the owner's sheets.

**New, and still game-agnostic** (FR-001a):

- **`region.rs`**: positioned queries over a page's runs and lines.
  - `find(label)` returns the rectangles where a label's text sits.
  - `right_of(rect, within)`, `below(rect, within)` and `within(rect)` return
    the runs inside a region.
  - `Rect{x0,y0,x1,y1}` uses page coordinates.
  - `PositionedLine` keeps `x0` and `x1`. `layout::Line` already has them,
    but the wasm JSON drops them.
  - This is what a form-shaped document needs. A source book reads in
    order, but a sheet reads by place. Spec 049 can use it later for stat
    blocks in two columns.
- **Bounds**: `Limits{max_bytes, max_pages}`, checked before parsing, with
  new `PdfError::TooLarge{bytes,limit}` and `PdfError::TooManyPages{pages,limit}`.
  The defaults are 10 MB and 20 pages (see the plan's open items). A DDB
  export is five pages and well under 1 MB.
- **Encryption**: `PdfError::Encrypted`, from lopdf's `is_encrypted()`. The
  crate does not try to decrypt. Asking for a password is out of scope, and
  refusing is the honest answer (FR-004).
- **Form XObjects**: `content.rs` does not follow `Do` into a Form XObject.
  Whether DDB's flattened export draws its field values inside one is a
  measurement, not a guess. Task T010 measures the owner's seven exports
  with `examples/probe.rs`. If any value is in a Form XObject, T011 adds `Do`
  traversal with the XObject's matrix applied. If none is, T011 records the
  measurement and closes.

**Not added**: AcroForm field reading. FR-002 targets the flattened export.
A sheet that still has form fields is read by its drawn text like any other
sheet, if it draws any. A sheet that draws none is refused as unreadable.

## R3. The neutral character and the reader contract live in a new crate

**Decision**: `crates/thunderforge-sheet-import`, which depends on
`thunderforge-pdf` and serde. It names no system. It holds:

- `ImportedCharacter`: identity, classes and levels, ability scores,
  proficiencies, defences, resources, speeds and senses, spellcasting,
  named content and notes (FR-005, contracts/sheet-import-core.md);
- `Field<T>{value, certainty, source}`, where `certainty` is `Read`,
  `Uncertain{reason}` or `Unread`, and `source` is `{page, rect, text}`
  (FR-003);
- the `SheetReader` trait, with `id()`, `version()`,
  `recognise(&Document) -> Recognition` and `read(&Document) -> Result<ImportedCharacter, ReadError>`;
- the mapping engine: `plan(mapping, reading, current, world) -> ImportPlan`;
- the `ImportPlan` types and `plan_hash`.

**Why**:

- FR-001a keeps the PDF crate ignorant of character sheets. FR-011 keeps the
  importer ignorant of 5e.
- A crate between the two is where "what a character is" lives without
  either rule bending.
- The mapping engine is shared so that SC-006 holds: a second system adds
  a reader and a declaration and touches nothing else.

**Alternatives rejected**:

- *Put the neutral types in `thunderforge-content`.* That crate is the book
  importer's content model, which is entries and prose. A character is a
  different shape, and coupling the two would make each one's changes the
  other's risk.
- *Put them in `thunderforge-canvas-core`.* It is the shared system
  contract. The reader trait depends on `thunderforge-pdf`, and canvas-core
  must not pull a PDF parser into every pack.

## R4. The D&D Beyond reader belongs to the 5e pack

**Decision**: a new crate, `packs/systems/dnd5e/sheet/`
(`thunderforge-system-dnd5e-sheet`), cdylib+rlib with a `wasm` feature.

- It implements `SheetReader` for `ddb-pdf`.
- It recognises a D&D Beyond export by the anchors it prints:
  "CHARACTER NAME", "CLASS & LEVEL", "FEATURES & TRAITS", "EQUIPMENT" and
  "SPELLCASTING CLASS". It does not rely on page numbers, because a
  non-caster's export has no spell page.
- It reads each field from the region relative to its label. It never reads
  fixed coordinates, so a layout shift of a few points does not break it.
- Proficiency marks are glyphs, and a glyph table maps them to
  none, proficient, expertise or half.
- Each `Uncertain` carries a reason a player can act on. For example: "the
  class line reads 'Fighter 3 / Wizard', with no level for Wizard".

The pack's server crate registers the reader with a new `SystemContribution`
slot, `sheet_import: Option<&'static SheetImport>` (contracts/sheet-import-core.md).
The pack's web package lazily imports the wasm build,
`@thunderforge/sheet-dnd5e`, built by a new step in `scripts/shared.mjs`
beside `buildPdf()`. The host finds it through the pack, so the host names no
system (`scripts/check-system-registry.mjs` keeps that true).

**Why**:

- FR-001b says the reader is built on the PDF crate.
- "Shared code may never name a system" says the reader is the pack's.
- The pack already owns its stat-block mapping (`stat_blocks.rs`). The
  sheet reader is the same kind of knowledge.

**Alternatives rejected**:

- *A reader module inside `thunderforge-system-dnd5e`.* That crate is
  compiled into the server and has no wasm build. Adding a wasm target and
  lopdf to it would put a PDF parser in every server build path of the
  pack's validators. A sibling crate keeps the weight where it is used.

## R5. The mapping is declared by the pack, as data first

**Decision**: `system.json` gains a `sheetImport` block
(contracts/sheet-mapping-5e.md). It holds:

- `readers`: the reader ids this system accepts;
- `fields`: neutral path to `dataType.field`;
- `content`: neutral content kind to an ability vocabulary type or an item;
- `derived`: values the pack calculates and the import only cross-checks;
- `playState`: the fields a re-import or a rollback never overwrites
  silently;
- `notes`: where unmapped values go.

A Rust hook on the `SheetImport` slot, `refine(reading, plan)`, covers what
data cannot express. The 5e hook needs it for three things: summing class
levels into `level`, folding hit-dice pools into the legacy `hit_dice`
string, and choosing an item over an ability for an attack. ADR-096 set this
precedent for content patterns.

A system whose `system.json` has no `sheetImport` block refuses the import
whole, before any reading is mapped (FR-012). The button does not appear for
that system either.

**Why**: US4.3 says a mapping is declared by the pack "the way its data types
and checks already are". `contentPatterns` with `refine_content` is exactly
that shape, and it already works.

## R6. Q1=C: the 5e pack grows to hold the whole sheet

**Decision**: these fields are added to the existing data types. There is no
new data type, because a data type is a column on `world_actor_system_data`
and the five that exist cover the sheet. data-model.md lists every field and
its validator rule.

- **`trait_data`**:
  - `classes[{name, subclass, level, hit_die}]`. The validator requires the
    top-level `level` to equal the sum, and `class` stays as the first
    class's name for the code that reads it today (SC-004).
  - Appearance: `age`, `height`, `weight`, `eyes`, `skin`, `hair`, `gender`
    and `faith`.
  - Personality: `personality_traits`, `ideals`, `bonds`, `flaws` and
    `backstory`.
  - Defences: `resistances`, `immunities`, `vulnerabilities` and
    `condition_immunities`, as string lists of damage types or condition ids.
  - `allies_and_organizations`.
- **`resource_data`**:
  - `hit_dice_pools[{die, total, used}]`. `hit_dice` and `hit_dice_used`
    are kept and derived from it by the refine hook, so nothing that reads
    them breaks.
  - `coins{cp, sp, ep, gp, pp}`.
- **`spell_data`**:
  - `spellcasting_classes[{class, ability, save_dc, attack_bonus}]`, for a
    multiclass caster.
  - `pact_slots{level, total, used}`.
- **Ability vocabulary** (`abilityVocabulary`): `feature` and
  `species_trait`, beside `spell`, `feat` and `enchantment`.
- **Links** (one host migration, system-neutral columns):
  - `world_actor_abilities` gains `prepared`, `granted_by`, `uses_max`,
    `uses_used` and `recharge`;
  - `world_actor_inventory` gains `equipped` and `attuned`;
  - `world_items` gains `weight`.

Named things arrive as **content, not text**. Spells, feats, class features,
species traits, items and attacks become linked abilities and items, either
the world's own or staged (R8). The free-text `feats`, `traits` and
`spells_known` lists are left alone. They are where a GM's notes went before
this spec, and an import does not touch them.

Values the pack **derives** are cross-checked, not stored. These are skill
and save modifiers, passives, initiative and the proficiency bonus from
level. If the sheet's number disagrees with the pack's own calculation, the
field is `Uncertain`, with both numbers in the reason. The value written is
the base the pack derives from, and the derived number is never written.
AGENTS.md section 5 and FR-003 both point this way.

An **attack** on the sheet is treated in one of two ways:

- If the attack names a weapon in the equipment list, the attack becomes
  that item, staged or matched, with spec 046's attack columns.
- Otherwise it becomes an ability with `attack_roll` and `damage` effects,
  the way `stat_blocks.rs` already maps a monster's attack.

**Alternatives rejected**:

- *A new `character_data` data type.* It would need a new JSONB column, a
  new allow-list entry in `mutations_actor_system_data.rs:137-146`, and a
  second home for fields that already have one.
- *The unused model in `packs/systems/dnd5e/server/src/models.rs:13-48`.*
  It is not wired to any data type. This spec deletes it in favour of the
  fields above, so there is one model, not two.

## R7. Who may import, and who may decide

The server checks every rule below. The web only hides controls.

- **Import onto an actor**:
  - allowed for Editor or Owner on that actor (`require_actor_permission`).
    Since ADR-110, a claim grants Editor, so a player who claimed the actor
    can import onto it;
  - allowed for the world's GM, who is always Owner;
  - refused for everyone else (FR-040 to FR-042).
  - Import goes onto an actor that already exists. A player who has no
    actor yet creates and claims one through the existing path first. An
    actor is never itself staged.
- **Trusted Player** (`Role::TrustedPlayer`, `manages_content()` in
  `crates/thunderforge-authz/src/role.rs:117`, from commit 283f4143):
  - may see all staged content and adopt or decline it (FR-032, FR-033,
    FR-036b);
  - may not roll back, and may not import onto actors they do not hold
    (decision 4).
  - A new async helper, `require_manages_content(conn, world_id, user_id)`,
    goes in `auth/world_membership.rs` beside `is_dm_of_world`. No such
    helper exists today.
- **Rollback**: the GM only (`is_dm_of_world`, FR-044b).
- **A kept file** may be downloaded by the character's owner and by the GM of
  a world where a version of it was applied. Nobody else may download it,
  Trusted Players included (FR-043b).
- **The name printed on a sheet grants nothing.** The importing account is
  the only identity that counts.

## R8. Staged content is a table of its own, and play never joins it

**Decision**: `world_staged_content` (data-model.md). Each row is one piece
of character-added content in one world:

- `kind`, `name` and a normalised name;
- `content_hash`, which is a hash of the normalised fields;
- `field_values`, which is the reading of that piece;
- `origin`, which is always `Uploaded`;
- `state`: `pending`, `adopted` or `declined`;
- the player who brought it, the first actor it came with, and who decided
  and when;
- once adopted, the `world_abilities` or `world_items` row it became.

The two link tables gain a nullable `staged_id`. A link points at
`ability_id` (or `item_id`), or at `staged_id`, never at both. A check
constraint enforces that.

- **Pending or declined**: the link carries `staged_id`. Every play path
  joins `world_abilities` by `ability_id` and does not see the link. This
  covers the play field, the combat panel, the roll buttons and the world
  compendium. It is FR-037 by construction, not by a filter someone can
  forget.
- **Adopted**: one transaction inserts the `world_abilities` (or
  `world_items`) row with `origin = 'Uploaded'` and repoints every link
  carrying that `staged_id` to the new id (FR-034, no duplicate). The staged
  row keeps `adopted_ability_id` for the record (FR-033c).
- **Declined**: the row stays and the link stays. The character's sheet
  shows the piece as "declined by the GM". Play does not see it. A GM or
  Trusted Player can revisit the decision (FR-036b).
- **The same thing from two characters** (FR-035): two pieces match when
  their kind, normalised name and hash are all equal, and they become one
  row with two links, so there is one decision. The same name with
  different content becomes two rows. Both are flagged "differs from
  another character's", so the GM sees why there are two.
- **Already in the world**: a piece that matches world content by kind and
  normalised name links to that content as it is (FR-031). If the fields
  differ, the review says the world's own version is used.

"A compendium named for the player" (FR-030a) is how the GM's screen groups
this table: one heading per player, listing what they brought. It is not a
`compendiums` row. That table holds books with an immutable origin and a
source hash, and staged content is neither.

**Why a separate table rather than a flag on `world_abilities`**: every
query that reads `world_abilities` today would need a new `WHERE`, and the
one that is missed leaks a declined spell to the table. A separate table is
default-deny.

## R9. Origin is a column, and nothing defaults it

**Decision**: one migration adds `origin "ContentOrigin" NOT NULL` to
`world_actors`, `world_items` and `world_abilities`.

- It backfills `'Authored'`, then runs `DROP DEFAULT`, so every insert from
  then on must state an origin (FR-033d: "no permissive Authored default").
- `content_origin()`, last redefined in
  `2026-09-13-140000-0000_additions_outlive_the_book/up.sql:92-125`, reads
  the columns instead of hard-coding `'Authored'` for actors, items and
  abilities.
- An immutability trigger on each column follows `compendiums.origin`'s
  model, with one exception. An actor's origin may move from `Authored` to
  `Uploaded` when an import is applied, and never back. A rollback does not
  revert it, because the actor did carry imported values (ADR-097).
- Every Diesel `Insertable` that inserts into these tables must state the
  origin. The compiler finds them once the field is not `Option`, and the
  server's `origin.rs` mirror (`origin_of` at :130) reads the column.
- `a56fe3fc` set the wording: "uploaded" means a book or a sheet, and a map
  is not uploaded content. Map import is unchanged.

## R10. The account holds the characters; each world holds its decisions

**Decision** (FR-030b, FR-043, FR-043a):

- **`brought_characters`** holds one row per character a person has brought
  in: `owner_user_id`, `system_id` and `name`. The first import onto an
  actor creates the row. A later import of the same sheet onto the same
  actor reuses it.
- **`sheet_import_versions`** holds one row per file read:
  - the character;
  - a version number;
  - the RustFS object key `sheets/{owner}/{character}/{version}.pdf`;
  - the file's sha256, byte size and page count;
  - the reader's id and version;
  - the server's reading, and the player's corrections, stored separately.
- **`actor_imports`** holds one row per write onto an actor:
  - the world, the actor and the version (null for a rollback);
  - the kind, `import` or `rollback`;
  - the before-snapshot (sheet fields and links), so that every version can
    be rolled back to;
  - what was written, who applied it, and, for a rollback, which import it
    restored.

Taking a brought character to a second world without re-reading is FR-030b's
purpose. The rows make it possible, but the screen to do it is not in this
spec's phases (see the plan, "What is deliberately not built").

## R11. The kept file in RustFS

**Decision**:

- Objects go in the existing bucket (`thunderforge-canvas-assets` by
  default) under a `sheets/` prefix, using `write_object` (`rustfs.rs:415`)
  and `read_object` (:284).
- `object_key` is webp-only. A new `sheets::object_key` follows the model of
  `feedback/mod.rs` (`STORAGE_PREFIX` and `object_key`).
- `delete_object` (:477) refuses anything outside `feedback/` because image
  objects are deduplicated across the instance. It gains `sheets/`, which is
  not deduplicated: every version has its own key.
- **Write order**: the object is written first under a fresh key, then one
  transaction writes every row. If the transaction fails, the object is
  deleted, so a failure leaves no orphan.
- **Access**: a server route, `GET /api/sheet-imports/{version_id}/file`,
  checks R7's rule and streams the object with `Content-Disposition:
  attachment`. There is no public URL and no presigned link.
- **Retention**: until the character is deleted (Q2=B). There is no actor
  deletion mutation today, so in practice the file stays until the account
  is deleted.
- **Self-hosted first**: RustFS only. There is no S3 connector in this spec.

## R12. Export and account deletion cover the files

**Decision**:

- `export_user_data_payload` (`users/mod.rs:187`) gains the person's
  brought characters, versions and import records, and moves the manifest to
  v4.
- `build_zip_export` (:502) adds each kept file under `sheets/`.
- `delete_user_data_on` (:406) deletes the rows in its transaction and
  collects the object keys. The objects are deleted after the transaction
  commits, the way `feedback/schedule.rs:179` deletes feedback objects.
- **Neither path touches stored objects today**, so this is new work in both.
- A rescued character (`collections/rescue.rs:47`) keeps its actor and the
  `actor_imports` rows that describe what was written. The file follows the
  account that uploaded it (open item 1 in the plan).

## R13. Unadopted content is refused, reported, and never reported unfairly

**Decision**:

- **Refusal** (FR-036a): any mutation that names a staged piece, by a link
  id or a staged id, is refused. Examples are a roll from it, an attack with
  it, or sharing it. The error code is `CONTENT_NOT_ADOPTED`, with a message
  saying it came in with the character and the GM has not adopted it. Play
  paths cannot name a staged piece by an ability id, because it has none.
  The refusal covers the paths that take a link id.
- **Reporting** (FR-038, FR-038a): a refused attempt writes a
  `world_unadopted_use_attempts` row. It also posts a GM-only chat message
  (`world_chat_messages.gm_only`, the nearest existing channel to the GM)
  that states only facts: who, which character, what, and when. It never
  states a motive. Reports are rate-limited to one per (actor, piece) per 10
  minutes. Later attempts in the window are counted on the row but not
  posted.
- **Staleness** (FR-038b):
  - The web client sends `x-tf-last-event: <id>` with every GraphQL
    request. The id is the newest world event its store has applied.
  - Each decision that withdraws content records a world event (R14).
  - If the attempt's header is older than the withdrawing event, the
    attempt is refused and *not* reported.
  - A piece that was never delivered to this viewer, because it was always
    pending, has no withdrawing event, so its attempt is always reported.
  - A request without the header is treated as stale, never as an
    accusation. Missing an attempt is better than accusing a player.
  - This is the first per-viewer "last delivery" in the product. The header
    is cheap, the store already tracks the id, and no server state is
    needed.

## R14. World events

New codes after `EVENT_CODE_AUTHORING_TOOLS_CHANGED = 38`:

| Code | Name | Payload (no content, ids only) | Reaches |
| --- | --- | --- | --- |
| 39 | `SHEET_IMPORT_APPLIED` | `actor_id`, `import_id` | everyone who can see the actor |
| 40 | `STAGED_CONTENT_DECIDED` | `staged_id`, `state`, `actor_ids` | everyone in the world |
| 41 | `ACTOR_ROLLED_BACK` | `actor_id`, `import_id` | everyone who can see the actor |

Each is recorded with `record_world_event` (`world_events.rs:344`) inside
the transaction that made the change. The event sync for actors refetches
the actor's sheet and links. Code 40 is the withdrawing event that R13's
staleness check compares against. The ids are world-event payloads, not
telemetry, so Principle VII is not involved.

## R15. Feature flag

**Decision**: `feature.sheet_import` (`THUNDERFORGE_FEATURE_SHEET_IMPORT`),
declared like `feature.book_import` (`declarations.rs:800-815`).

- It defaults to **false** while the phases land, and T097 flips it to
  **true** with the proof.
- `applySheetImport` and `sheetImportPreview` check it. `useFeatureFlag`
  hides the button.
- Turning it off stops new imports. Kept files stay downloadable and staged
  content stays decidable, so a GM is never left with pending pieces they
  cannot clear.

**Why a flag**: the feature holds personal documents at rest. An operator
must be able to stop new uploads without a release.

**What is limited**: "Players cannot bring a character in from a PDF."
Flags are instance-wide, so there is no per-world switch.

## R16. Telemetry, against spec 086's contract

Spec 086 is planned, not built. `crates/thunderforge-telemetry-policy` and
`packages/telemetry` do not exist yet.

The names below are declared now and added to 086's lists in the same change
that adds them. Until 086 lands, the same points are `tracing` spans and
fields, so the instrumentation is not lost.

**Server**, from `opentelemetry::global::meter("thunderforge")`:

| OTel name | Kind | Attributes |
| --- | --- | --- |
| `thunderforge.sheet_imports` | counter | `system`, `reader`, `outcome` = `applied`, `refused_flag`, `refused_permission`, `refused_bounds`, `refused_unrecognised`, `refused_unmapped_system`, `refused_plan_changed`, `failed` |
| `thunderforge.sheet_import.read_duration` | histogram, `s` | `reader` |
| `thunderforge.sheet_import.fields` | counter | `certainty` = `read`, `uncertain`, `unread`, `corrected` |
| `thunderforge.staged_content.decisions` | counter | `decision` = `adopt`, `adopt_all`, `decline`, `revisit` |
| `thunderforge.unadopted_use_attempts` | counter | `result` = `reported`, `suppressed_stale`, `rate_limited` |

`system` and `reader` are pack-declared ids from a closed set, so their
cardinality is bounded. The table carries no names, no ids and no content.

**Browser**: a new event, `sheet_import.step`.

- Its attribute `step` is one of `opened`, `read`, `reviewed`, `applied`,
  `declined` or `failed`.
- Its attribute `reason` appears on `failed` only. It is one of
  `encrypted`, `too_large`, `too_many_pages`, `unrecognised`, `unreadable`
  or `plan_changed`.
- It is added to 086's events table and to `ALLOWED_ATTRIBUTES`.
- It is not the landing `funnel`, whose steps are the demo's.

Tests run with `TELEMETRY=false`.

## R17. Fixtures: deterministic, synthetic, and nobody's sheet

**Decision** (FR-060 to FR-062):

- **Committed**: PDFs generated from code. A fixture generator in the reader
  crate's `tests/fixtures/gen.rs`, using lopdf's writer, lays out invented
  characters in D&D Beyond's layout. The positions and labels come from the
  owner's exports, but the fixtures contain none of their values.
  - The four corpus characters are:
    - a single-class Fighter 5;
    - a multiclass Fighter 3 / Wizard 2;
    - a Cleric 7 who prepares spells;
    - a non-caster Rogue 4 with no spell page.
  - Three more fixtures cover the edge cases and the re-import: a
    Warforged with a poison resistance, a sheet with a mark the reader
    cannot place, and the multiclass character again at level 6.
  - Generation is deterministic: same bytes every run, no timestamps, and a
    fixed `/ID`. A test asserts the generated bytes' hash, so a change to
    the generator is a visible diff.
- **Not committed**: the owner's seven real exports. A
  measurement example that no test runs, `examples/measure_corpus.rs`, reads them from a path
  given in `THUNDERFORGE_SHEET_CORPUS`. It prints read, uncertain and unread
  counts per field without printing any value, and it is run by hand, the
  way spec 049's book measurements were. `.gitignore` gains
  `sheet-corpus/`.
- **Other games**: a page with none of D&D Beyond's anchors, such as a 049
  book fixture, proves the "not a D&D Beyond sheet" refusal.

## R18. The door: the actor screen, and the Players screen beside it

- **Actor screen** (`ActorDetailPage.tsx`): "Bring in a sheet" goes in the
  header button group at about line 405, beside Edit and Share. It shows
  when `mayEditActor` (line 191) holds, the flag is on, and the actor's
  system declares `sheetImport`. It opens the review as a route,
  `/world/:id/actor/:actorId/import`, loaded with `React.lazy`, so the wasm
  reader is fetched only when somebody uses it.
- **The dependency on `hotfix-player-hero-edit`**:
  - That branch (`ca568bff`, `cc28cd0e`, not yet on `main`) gives a player a
    way from the Players screen to their own hero's builder and sheet
    (`player-hero-build-${actorId}`, `player-hero-sheet-${actorId}`). It
    also makes the 5e sheet editable for a player who holds the actor.
  - US1's "a player viewing an actor they hold" depends on that path
    existing.
  - Phase 3 starts after the branch merges. T041 adds the import entry to
    the same Players-screen row, as `player-hero-import-${actorId}`.
- **Review screen**: per-field rows with certainty, the old value, the new
  value and the source text, plus a correction input on uncertain and
  unread rows. It reuses `components/import/` patterns
  (`BookReview.tsx`'s approve and correct row), not its code. A book entry
  is not a sheet field, and sharing a component across the two would bend
  both.

## R19. Slices

- **New slice: `sheet-import`.** It owns `sheet-import-` e2e specs. Its
  paths are:
  - `crates/thunderforge-sheet-import/**`;
  - `packs/systems/dnd5e/sheet/**`;
  - `apps/web/src/pages/world/actor/import/**`;
  - `apps/web/src/components/world/staged/**`;
  - the server's `sheet_import/**`, `staged_content/**` and
    `mutations_sheet_import*.rs`.
- **Neighbours**:
  - `actors`: `actor-detail-routes.spec.ts`, and `players-hero-edit.spec.ts`
    once the hotfix lands. The door sits on those screens.
  - `compendium`: `abilities-compendium.spec.ts` and `world-compendium.spec.ts`.
    An adopted piece appears in the world's compendium, and a staged one
    does not.
  - `combat`: `combat-attack.spec.ts` (an imported attack used in a
    fight) and `dnd5e-sheet.spec.ts` (the sheet grows new sections).
  - `book-import`: `pdf-reader.spec.ts`. The PDF crate changed under it.
  - `collections`: `content-origin.spec.ts` (origin becomes a column) and
    `library-account-deletion.spec.ts` (deletion removes kept files).
  - `accounts`: `user-data-export.spec.ts` (the export carries them).
- **Standalone half**: `e2e:sheet-import:standalone` runs
  `pnpm -F @thunderforge/dnd5e test:sheet-reader`, as `hero-builder` does for
  its app. A harness page reads the generated fixtures in the browser with
  no server, and checks that the result matches the native reading. `cargo test` covers the native reader.
- **Summary correction**: `hero-builder`'s summary names
  `specs/048-bringing-a-character-in`, which it is not. T004 removes the
  mention.
- **How a change is proven**: the migration and `schema.rs` are cross-cutting,
  so `pnpm e2e:which --diff` prints FULL SUITE. Run the slices it names
  instead. The owner has ruled out the full suite as a gate.
- **External-stack runs** need `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and
  `--workers=1`.
