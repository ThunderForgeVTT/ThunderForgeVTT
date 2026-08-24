# Phase 0 Research: Genie House System

## R1: Does the engine already support a gridless/zone scene topology, or does Genie need to invent one?

**Decision**: Reuse the existing `GridType::Gridless` enum variant (`src/engine/src/resources/scene_data.rs`) — do not invent a new topology concept.

**Finding**: `GridType` already has three variants (`Square`, `Hexagonal`, `Gridless`), and `src/engine/src/plugins/grid.rs`'s render/interaction match already has a case for `GridType::Gridless => ()` — a documented placeholder, not a missing capability. The real gap is one level down: `src/server/migrations/2026-05-05-010000-0001_create_scenes_table/up.sql` defines `grid_type TEXT NOT NULL DEFAULT 'square' CHECK (grid_type IN ('square', 'hex'))` — `'gridless'` is not a permitted value at the database layer, so no scene can actually be created in that mode today even though the engine already models it.

**Rationale**: This is exactly the kind of gap Genie's spec (SC-004) claims it will surface — a facet the engine already anticipated in its type system but that was never wired end-to-end because nothing needed it yet. Fixing it is one additive migration (widening a CHECK constraint, never editing the already-applied original migration) plus replacing the `()` no-op with real zone-based token interaction — not new architecture.

**Alternatives considered**: Defining Genie's "Wish-Warped Zone" as an app-level (React-only) visual convention layered on top of a `Square` scene with grid rendering hidden — rejected; this would not exercise the engine's own topology-handling code at all, defeating User Story 2's entire purpose (spec.md explicitly frames this as forcing the canvas engine's gridless path to be real, not simulated in the presentation layer).

## R2: Does Genie need any new database tables for characters, NPCs, items, or lore?

**Decision**: No new tables. Genie characters/NPCs use the existing actor `data_types` contract (the same mechanism `dnd5e`'s `ability_data`/`resource_data`/`proficiency_data` blocks already use, per `packs/systems/dnd5e/system.json`); Wish-Granted Items are ordinary `world_items` rows (spec 013) with a formula-bearing `world_item_effects` entry; the Patron/Lineage link is an ordinary reference to a `world_lore_entries` row (spec 012).

**Rationale**: This is the point of the exercise — spec.md's Assumptions explicitly frame Genie as proof that the generic tables are sufficient, not as a reason to add Genie-specific storage. Every one of specs 011-014 already ships a generalized, system-agnostic data shape; a from-scratch table set would test nothing new and would violate the "no bespoke storage" premise the spec is built on.

**Alternatives considered**: A dedicated `genie_characters` table mirroring a hypothetical richer data model — rejected outright; it would prove nothing about the generic path's sufficiency and would contradict FR-003 (data_types-compatible) and FR-007 (existing item/effects model)'s explicit compatibility requirements.

## R3: How does the Manifestation roll (keep/drop + exploding + success-counting in one formula) map onto the existing dice engine?

**Decision**: Express the Manifestation roll as a single formula string resolved by `crates/thunderforge-dice` (spec 014), using that crate's existing keep/drop, exploding, and success-counting notation support — confirmed present in `specs/014-dice-rolling-engine/data-model.md`'s `DieOutcome`/`ResolutionKind::SuccessCount` types, which already exist specifically to support dice-pool-with-success-counting formulas.

**Rationale**: Spec 014 was built to support exactly this composition (its own data model calls out `SuccessCount` as an existing `ResolutionKind` variant, not something added for Genie). Genie's formula is `{skill}d6k{keep}!6cs>=4` in the notation spec 014 already defines (keep-highest-N, explode-on-6, count-successes-at-4-or-more) — the notation composition is new (no other system pack's formula needs all three at once in one string), the underlying engine capability is not.

**Alternatives considered**: Resolving keep/drop and exploding as one roll, then piping the result through a second, separate success-counting step — rejected; this would only exercise the dice engine's individual features in isolation, the opposite of spec.md User Story 1's explicit goal (composing all three in a single formula/roll record).

## R4: What does a "Wish Points" leveled resource table look like structurally?

**Decision**: A per-level array table on the manifest, structurally identical to `dnd5e`'s existing `spellSlots` field (`packs/systems/dnd5e/system.json`) — a JSON object keyed by level string, each value an array (for Genie, a single-element array of the Wish Points total at that level, since Genie has no multi-tier resource split the way 5e's spell slots do).

**Rationale**: FR-004 explicitly requires structural compatibility with the existing leveled-table pattern; reusing the exact shape means the same manifest-loading and derived-data-recalculation code path that already handles `spellSlots` for `dnd5e` handles Genie's `wishPoints` table with no new parsing logic.

**Alternatives considered**: A formula-derived Wish Points value (e.g. `level * 2 + CHA_mod`) instead of a static table — rejected for v1; spec.md's Assumptions call out that Genie should stay structurally close to existing patterns rather than adding a new derived-value mechanism, and a static table is what `spellSlots` already is.

## R5: Does the `legal` manifest field already support "wholly original, no attribution" as a case, or does it need new optional-field handling?

**Decision**: No changes needed. `crates/pack_system_spec::SystemManifestLegal` (already implemented per spec 016) has `required_notice`, `disclaimer`, `trademark_restrictions` (defaults to empty `Vec`), and `source_url` all as optional/defaulted fields — only `license_name` and `attribution_text` are required non-empty strings. Genie's `legal` object sets `licenseName: "ThunderForgeVTT Original Content"` and `attributionText` to a short first-party statement (e.g. "Genie is an original system created for ThunderForgeVTT. No external attribution required."), satisfying the struct's validation with all the optional fields genuinely empty/absent.

**Rationale**: Confirms FR-010/FR-011 need no manifest-contract changes — spec 016's existing implementation already anticipated a system with nothing to attribute, it just hadn't had a real example exercise that path until Genie.

**Alternatives considered**: Making `license_name`/`attribution_text` fully optional in the shared struct for Genie's sake — rejected; Genie can satisfy the existing required-non-empty-string constraint trivially with first-party text, so no contract change is needed for one system's sake.

## R6: Does the size-category → token-footprint path already exist, or is new engine work needed?

**Decision**: Reuse the existing token `scale` field (`apps/web/src/types/token.ts`, also present on the Bevy-side token component per `src/engine/src/systems/token.rs`) as the mechanism, and add Genie-specific data — a manifest-level lookup from size category name to a default `scale` value — rather than any new engine capability.

**Finding**: Tokens already carry a free numeric `scale` field; there is no existing discrete "size category" concept (no Tiny/Small/Medium/Large/Huge-style enum, no automatic multi-square-footprint snapping) anywhere in the engine or web token types today. So the gap is purely data-driven, not architectural: nothing prevents a size category from resolving to a `scale` multiplier today, there's just no named-category table yet for any system pack to draw one from.

**Rationale**: This is a lighter-weight gap than R1's gridless-scene finding — no schema/engine change needed, only a small manifest-level lookup table (`sizeCategories: { diminutive: { scale: 0.5 }, ..., colossal: { scale: 4 } }`, structurally similar to how `spellSlots`/`wishPoints` are already keyed lookup tables on the manifest) and a small web component (`SizeCategoryBadge.tsx`, per plan.md's project structure) that reads a Genie NPC's size category and applies the corresponding `scale` when placing its token.

**Alternatives considered**: Building a first-class, engine-level size-category system (a proper enum with automatic grid-square-occupancy snapping, closer to what dnd5e/pathfinder2e's digests describe) — considered, but out of scope for this plan; spec.md's FR-005 only requires that Genie's size category "determines a token's default grid footprint," which a manifest-level scale lookup satisfies without a new engine primitive. A richer, discrete-grid-square-occupancy system would be a good candidate for a *separate* future feature informed by what Genie's simpler version surfaces in practice.

## R7: How does session-scoped shared state (Session Wish Pool, Doom Clock, Puzzle Clocks) get kept live-synced across every connected player, per FR-013/FR-015?

**Decision**: Reuse the existing `world_events` table and `record_world_event` function (`src/server/src/world_events.rs`), and the existing `worldEventsCreated(worldId)` GraphQL subscription (spec 005) that every world-member client already subscribes to — add one new `event_code` value (15, `"genie_session_state"`) whose JSON payload carries a discriminated shape (`{ "kind": "wish_pool" | "doom_clock" | "puzzle_clock" | "resource_trade", ... }`), following the exact convention the existing `token_event` payload column already uses for codes 10-14.

**Finding**: `world_events` already exists specifically as a generic, per-world broadcast mechanism — confirmed by data-model.md and research.md of spec 005 stating "This feature introduces zero new tables... `world_events` already exist[s] and already fire[s] on every wall/light/shape/token mutation." The event codes already documented are 10 (wall), 11 (light), 12 (shape), 13 (map import), 14 (token) (per `apps/web/src/engine/world/sync/tokens.ts`'s own doc comment) — no session/clock/shared-party-state concept exists yet anywhere in the schema, confirmed by an explicit search for `clock`/`session_state`/`world_state` across `src/server/src` and `apps/web/src/types` returning no hits.

**Rationale**: This is the third instance in this plan of the same pattern (R1's gridless scene, R6's token scale): the engine/server already has a generic mechanism built for a different original purpose that Genie's session loop is simply the first system to need for this specific case. No new subscription, no new WebSocket wiring, no new client reconnect/resync logic — the client-side `LiveSyncState` machine and reconnect-resync behavior spec 005 already built work identically for event_code 15 as they do for 10-14.

**Alternatives considered**: A dedicated GraphQL subscription for session state (`genieSessionUpdated(worldId)`) — rejected; it would duplicate spec 005's connection-management, reconnect, and resync logic for no benefit, and would need its own client-side wiring where reusing `worldEventsCreated` needs none. Polling instead of push-based sync — rejected outright; the whole point of the Session Wish Pool and clocks is that every player sees changes the instant they happen, which is exactly what live sync (not polling) is for, and spec 005 already solved this problem.

## R8: Who is authorized to spend a wish, advance a clock, or trade a Session Resource?

**Decision**: `spendWish`, `advanceDoomClock`, and `advancePuzzleClock` are GM-only mutations, following the repo's existing "DM-only" convention already established for content mutations in specs 011 (lore: "Created (DM-only, FR-002)") and 013 (items: "Created (DM-only, FR-002)"). `tradeSessionResource` is a two-party-consent mutation — one player proposes a trade, the other must explicitly accept it before it applies — a genuinely new authorization shape not yet used elsewhere in the codebase.

**Rationale**: Spec.md's own Edge Cases section is explicit that "party agreement" for spending a wish is "a social convention the GM enforces, not a system-enforced rule" — so the *system* only needs one authorized actor to execute the mutation once the table has verbally agreed, and the GM is the natural, precedented choice (matching how GM-adjudicated consequences already work for Doom Clock advancement per FR-014's "GM-adjudicated narrative effect" language). Resource trading is different in kind: FR-017/FR-018 describe it as a *player-to-player* negotiation layer, so gating it behind the GM would contradict the point of the mechanic — but a single player also shouldn't be able to unilaterally debit another player's holdings, hence two-party consent.

**Alternatives considered**: Making all four mutations GM-only (simplest to build) — rejected for `tradeSessionResource` specifically, since routing every trade through the GM would turn a fast, informal negotiation into a GM bottleneck, undermining the Catan-style feel that was the explicit point of adding Session Resources (spec.md Clarifications, Q5). Making `spendWish`/clock-advancement player-callable with a client-side vote UI — rejected as unnecessary system complexity for a decision spec.md already says the system doesn't need to enforce; a vote-counting mutation would be speculative engineering against a requirement that explicitly disclaims needing one. This two-party-consent pattern for trades is new enough to the codebase (Constitution Principle IV, plan.md) to warrant its own short ADR during implementation, documenting the pattern for any future feature that needs player-to-player consent (e.g. item trading in the base items/inventory system, if ever added).
