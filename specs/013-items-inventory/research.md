# Phase 0 Research: Items & Inventory System

## 1. Item Effect representation (structured, system-agnostic, scaffolded for a future trigger)

**Decision**: One row per effect in a dedicated `world_item_effects` table (not a JSONB blob on `world_items`), with columns `effect_type` (enum: `heal` / `damage` / `modifier` / `attack_roll`, extensible), `formula` (`TEXT`, e.g. `"3d6"`, `"1d20 + STAT + MODIFIERS"`, `"2d8"`), `target` (`TEXT`, freeform resource/attribute name, e.g. `"Hit Points"`, `"STAT"`), `trigger_kind` (enum: `on_use` / `passive`, nullable/defaulted per FR-004a — stored now, evaluated by nothing until a future dice-roller spec), and `sort_order` (`INT`, so a weapon's attack-roll effect and its paired damage effect display in authored order).

**Rationale**: A dedicated table (rather than a JSONB array column) makes "add/remove/edit effects independently" (FR-005) a normal row-level CRUD operation instead of a read-modify-write of a JSON array, and makes future querying (e.g. "which items have a `heal` effect") a plain indexed query instead of a JSONB scan. The `trigger_kind` column exists specifically because Clarifications required the shape to anticipate on-use-vs-passive without implementing it — adding a nullable enum column now costs nothing and avoids an ALTER TABLE + backfill when the dice-roller spec arrives. `sort_order` is needed because a single Item can carry more than one effect (attack-roll + damage) and the UI/spec examples imply a natural authored order (roll to hit, then roll damage).

**Alternatives considered**:
- JSONB array column (`world_items.effects JSONB`): rejected — FR-005's "add/edit/remove independently" is more naturally a row per effect than a JSON-array splice, and a future ruleset layer validating/joining against specific effect rows (e.g. "which of this item's effects apply on use") is easier against real rows than a JSON blob's internal array indices.
- No `trigger_kind` column at all (defer entirely to the future spec, add the column then): rejected per the explicit Clarification — the user asked for scaffolding now ("scaffolded but not implemented"), and a day-one column is strictly cheaper than a later migration on a table that may already have production rows.
- A generic `effect_type: modifier` used for both stat boosts AND stat detriments via a signed formula (e.g. `"-1d4"`) rather than separate `buff`/`debuff` types: chosen — the Clarification said "stat boosters or detriments" should be covered, and a signed dice/number formula on the existing `modifier` type already covers both without doubling the enum; a bare `-1d4` or `-2` formula on a `modifier` effect is a detriment, a positive one is a boost. No separate `buff`/`debuff` effect type is introduced.

## 2. Item ↔ Actor inventory (`world_actor_inventory`)

**Decision**: A join table `world_actor_inventory (id, actor_id, item_id, quantity, created_at, updated_at)` with a unique constraint on `(actor_id, item_id)` — enforcing "at most one row per distinct Item per Actor" (Key Entities) at the database level, not just in application logic. Adding an Item that already has a row for that Actor is an `UPDATE quantity = quantity + :n` (upsert via `ON CONFLICT (actor_id, item_id) DO UPDATE`), never an `INSERT`. Reducing quantity to 0 deletes the row (FR-011) rather than storing a zero.

**Rationale**: A DB-level unique constraint makes "no duplicate rows for the same Item on the same Actor" (SC-002) a guarantee rather than a hope, and `ON CONFLICT ... DO UPDATE` is the standard Postgres/Diesel idiom for exactly this "merge or insert" shape — no read-then-write race condition between two concurrent "add 1 potion" mutations.

**Alternatives considered**:
- Application-level "check if exists, then update or insert" (two round trips): rejected — race-prone under concurrent adds (two simultaneous "add 1 potion" requests could both see "not found" and insert two rows) unless wrapped in a serializable transaction, which is more complex than the DB-level upsert Postgres already provides.
- Storing quantity=0 rows instead of deleting them (soft "empty slot"): rejected — FR-011 explicitly requires the entry to be removed, not retained at zero, and no spec requirement calls for remembering "this actor used to carry potions."

## 3. "Did you mean?" name-similarity check on Item creation (Clarifications)

**Decision**: Enable the built-in Postgres `pg_trgm` extension (`CREATE EXTENSION IF NOT EXISTS pg_trgm`) via a migration, add a trigram GIN index on `world_items.name`, and expose a new GraphQL query (`suggestItemName(worldId: ID!, name: String!): [GraphQLItem!]!`) that runs `SELECT * FROM world_items WHERE world_id = :world_id AND similarity(name, :name) > 0.4 ORDER BY similarity(name, :name) DESC LIMIT 5`. The frontend debounces this query as the DM types a new Item's name and renders "Did you mean {existing item}?" as a non-blocking inline hint (FR-020) — selecting a suggestion is a UX convenience (e.g. "open that item instead"), never a save-blocking validation.

**Rationale**: `pg_trgm` is a first-party Postgres extension (no new Rust dependency, no new external service), already the right tool for "fuzzy name similarity" and cheap to add via GIN index at this table's expected scale (a world's item catalog, not a million-row corpus). It keeps the "did you mean?" logic entirely server-side and query-driven, consistent with Principle III (server as authoritative) and avoids duplicating fuzzy-string-matching logic in the browser.

**Alternatives considered**:
- Client-side fuzzy matching (e.g. a JS Levenshtein/Fuse.js pass over an already-fetched item list): rejected — would require shipping the world's entire Item name list to the client on every keystroke (or a separate list-fetch), duplicating server-side matching logic for no benefit, and doesn't scale as cleanly as an indexed DB query as an item catalog grows.
- A Rust-side fuzzy-matching crate (e.g. `strsim`) computing similarity in the application layer over a full-table scan: rejected — reinvents what `pg_trgm` already does natively and indexed, and would require pulling every item name back to the app process on every keystroke rather than letting Postgres filter/rank server-side.
- Blocking creation on an exact or near-exact name match: rejected outright per Clarifications — names are explicitly allowed to collide; the feature is a nudge, not a gate.

## 4. Item icon/image storage (optional, reuses existing asset pipeline)

**Decision**: When present, an Item's icon reuses the exact upload/transcode path already used for Actor portraits/images (existing RustFS-backed `storage/` module + `transcode.rs`, per ADR-039) rather than introducing a second image pipeline. `world_items.icon_asset_id` is a nullable FK to whatever asset-row shape actors already use for their image (or a lightweight `world_item_image_assets` row if actors don't already have a directly reusable table — an implementation detail for tasks.md, not a data-model fork). No new resize/thumbnail logic is needed since it's inherited from the existing path.

**Rationale**: Clarifications settled that the icon is optional (not gating creation), and the spec's own Assumptions state icon handling should reuse the same asset-upload infrastructure "where practical" — there is no functional requirement calling for anything Item-specific about image handling, so reusing the actor path exactly is the lowest-risk choice.

**Alternatives considered**:
- A dedicated Item-image pipeline mirroring spec 012's lore-image processing (full-size + thumbnail WebP renditions): considered, but rejected as unnecessary duplication — Items don't have lore's "pasted inline in rich text" requirement, just a single icon slot, which is exactly the actor-portrait shape already solved.

## 5. Item share-link and cross-world copy (Clarifications: "same way" as actors)

**Decision**: Directly mirror `world_actor_shares`/`mutations_actor_shares.rs` (spec 010) as `world_item_shares`/`mutations_item_shares.rs`: same `share_code` generation scheme (`generate_share_code` — UUID-derived, uppercased, truncated), same revoke-by-`created_by`-or-DM rule, same `SharedItemPreview` GraphQL projection pattern (excludes `id`/`world_id`/`created_by`/ownership block), same `copySharedItemToWorld` transaction shape (new `world_items` row + cloned `world_item_effects` rows, empty ownership block, no live reference back to source), reusing the existing `myDmWorlds` query as-is (already world-type-agnostic).

**Rationale**: Clarifications were explicit that Items should get "the full sharing system the same way" as actors — this is a direct precedent-reuse decision, not a new design; copying the actor implementation's shape file-for-file minimizes the chance of behavioral drift between the two share mechanisms (e.g. accidentally forgetting the revoke-by-DM-too rule).

**Alternatives considered**:
- A generalized `world_content_shares` polymorphic table serving both actors and items (and future lore): rejected for this pass — spec 010's own Assumptions already flagged this as a future generalization candidate once multiple content types need it; building that abstraction now, on the second content type, is premature relative to this feature's scope and risks a bigger, riskier migration than two parallel, well-understood tables.

## 6. Compendium "Items" tab (replaces spec 011 placeholder)

**Decision**: `ItemCompendiumTab.tsx` + `ItemPreviewPanel.tsx` directly mirror `NpcCompendiumTab.tsx` + `ActorPreviewPanel.tsx`'s existing shape (search-as-you-type over name/description, row-select-opens-right-side-preview, DM-only "Add Item" affordance, Edit gated on Editor/Owner). `WorldCompendiumPage.tsx`'s `items` tab entry (`apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx:75`) swaps `<ComingSoonTab label="Items" />` for the new tab's `<div className="grid gap-4 lg:grid-cols-[2fr_1fr]">...</div>` composition, matching the `npcs` tab's existing structure line-for-line.

**Rationale**: Spec 011 explicitly designed the Compendium's tab array so "adding a new top-level content type ... requires adding a new tab, not restructuring" (spec 011 SC-005) — Items is the first real test of that promise, so the plan takes it at its word rather than inventing a new UX pattern.

**Alternatives considered**: None seriously considered — the precedent is explicit and recent (spec 011 shipped this exact extension point for this exact purpose).
