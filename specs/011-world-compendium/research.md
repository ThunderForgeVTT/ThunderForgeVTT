# Research: World Compendium

All items below were resolved directly from the existing codebase (spec 010's precedent) rather than requiring new investigation — this feature is deliberately scoped to reuse established patterns.

## 1. Where does "Last Session Notes" live in the data model?

**Decision**: A single nullable `session_notes TEXT` column directly on the `worlds` table.

**Rationale**: The spec's own Assumptions section settled this as "a single per-world freeform text value (the latest recap), not a per-session historical log." A single scalar column on the existing `worlds` row is the simplest storage that satisfies that — no new table, no foreign keys, no join. This exactly mirrors how `world_actors.description` was added in spec 010's follow-up work (a single nullable `TEXT` column via a one-line migration).

**Alternatives considered**:
- A `world_session_notes` table keyed by world + session number — rejected: over-engineered for "the latest recap only"; the spec explicitly defers a real per-session log as future work.
- Storing notes in `world_actor_system_data`-style JSON blob — rejected: notes aren't actor-scoped or game-system-scoped; a plain column is more discoverable and query-able.

## 2. Who can write Last Session Notes, and how is that enforced?

**Decision**: Reuse the exact "DM or GM" check already established for NPC creation in spec 010 (`is_dm_of_world`/`useWorldRole`'s `isGm` = Owner-or-GM), applied server-side in the new `updateWorldSessionNotes` mutation.

**Rationale**: The spec explicitly says this should mirror "the read/write split already established for other DM-curated world content (e.g. the NPC catalog's add/edit rights)." `is_dm_of_world` (`auth/actor_permissions.rs`, spec 010) already does exactly this check and is reusable as-is — no new authorization primitive needed.

**Alternatives considered**: A new dedicated "world settings" permission — rejected: no other world-level DM-only field exists yet to justify a general permission concept; the existing DM/GM role check is sufficient and consistent with precedent.

## 3. How does the NPCs tab's row-select preview panel get its data?

**Decision**: The preview panel renders from the same in-memory `WorldActorRecord` array the table already fetched (via `getWorldActors`) — selecting a row just sets `selectedActorId` in the parent `WorldCompendiumPage` component's state; no extra network round trip per selection.

**Rationale**: `getWorldActors` already returns every field the preview needs (label, description, isNpc, actorType, gameSystemId, myPermissionLevel). Fetching per-row on selection would add latency and complexity for zero benefit, since the full roster is already client-side for the table/search to work against.

**Alternatives considered**: A dedicated `getActor(actorId)` fetch on selection — rejected as unnecessary; that function already exists (spec 010, used by `ActorDetailPage`) and remains available for the full view/edit routes reached *from* the preview panel, but the panel itself doesn't need it.

## 4. How is the tab shell structured to stay extensible for future content types?

**Decision**: `WorldCompendiumPage` renders the existing `Tabs` UI primitive (`@/components/ui/tabs/Tabs`, already used for Session Setup's Trackers/Settings split in spec 009/010) driven by a plain array of `{ value, label, icon, content }` tab definitions. The NPCs tab's `content` is `<NpcCompendiumTab />`; Items/Abilities are `<ComingSoonTab label="Items" />` / `<ComingSoonTab label="Abilities" />`.

**Rationale**: Adding a future tab (e.g. Maps, Lore) is then a one-line addition to that array plus a new tab-content component — no restructuring of the NPCs tab, routing, or preview-panel pattern, directly satisfying SC-005. This is the same `Tabs` component already proven for exactly this "extensible named sections" job elsewhere in the app.

**Alternatives considered**: Separate sub-routes per tab (`/compendium/npcs`, `/compendium/items`) — rejected: the spec frames tabs as sections of one page ("tabbed layout"), not independently-linkable routes; a future spec could add deep-linkable sub-routes without conflicting with this decision if ever needed, but nothing in this spec requires it.

## 5. Does the NPC catalog's existing FlexSearch integration (spec 010) need to change?

**Decision**: No. `@/search/actorSearch`'s `indexActors`/`searchActorIds` functions are reused unchanged by the new `NpcCompendiumTab`; only the *rendering* (split-view table + preview panel instead of Session Setup's plain table) and the *row interaction* (select-to-preview instead of Link-to-navigate) change.

**Rationale**: The search behavior itself (instant, client-side, per-world FlexSearch index) already satisfies FR-003 as written; the spec's own Assumptions section confirms "no new NPC-specific capability is introduced, only a new surface."
