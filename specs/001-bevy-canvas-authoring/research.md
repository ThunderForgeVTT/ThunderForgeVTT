# Phase 0 Research: Native Canvas Authoring

No `[NEEDS CLARIFICATION]` markers remained in the spec or Technical
Context, so this phase resolves the open technical-approach questions
identified while drafting the plan, rather than ambiguous requirements.

## 1. Wall backend reuse vs. rebuild

**Decision**: Reuse the existing `walls` table, `WallMutation`
(`create_wall`/`update_wall`/`delete_wall`), and `walls` scene query
unchanged. This feature only adds the missing authoring UI + engine-side
rendering/occlusion on top of it.

**Rationale**: Verified `mutations_walls.rs` already implements full CRUD
with scene-ownership enforcement matching Constitution Principle III, and
`queries/scene.rs::walls` already returns per-scene wall lists. No backend
gap exists for walls — confirmed by direct code inspection, not assumption.

**Alternatives considered**: Rebuilding wall mutations as part of this
feature — rejected, would duplicate working, already-shipped code (Phase 6
commit).

## 2. Light source and annotation persistence shape

**Decision**: Model `light_sources` and `annotations` as their own Diesel
tables/GraphQL types, structurally parallel to `walls` (per-scene,
`created_by`/`updated_by`, UUID v7 primary key), rather than folding them
into a generic "scene object" table.

**Rationale**: Each entity has a distinct, stable shape (light: position +
radius + intensity + optional token attachment; annotation: stroke/shape
geometry + visibility flag) and distinct query patterns (players never
query raw annotation/light authoring data, only rendered effects for
lights and player-visible annotations). A generic polymorphic table would
need a JSON payload per type anyway, adding indirection without saving
real duplication — three tables of ~8 columns each is not enough
repetition to justify a shared abstraction (project convention: prefer
three similar things over one premature generalization).

**Alternatives considered**: Single `scene_objects` table with a `kind`
discriminator and JSON payload — rejected as premature abstraction; would
also complicate ownership/occlusion queries that need typed columns
(radius, blocks_vision) for server-side or spatial-index filtering later.

## 3. Vision/light occlusion algorithm

**Decision**: Use 2D shadow-casting (ray/segment intersection against wall
segments) computed in the engine (WASM), per the existing `FogPlugin`
composite-rendering slot reserved in ADR-032, re-triggered whenever the
`WallSet` or `LightSet` resource changes (not every frame) plus once per
moved token/light.

**Rationale**: Standard, well-understood technique for tile/segment-based
VTT vision (used by Foundry VTT and similar tools); bounding the recompute
to "on change" rather than "every frame" keeps it cheap at the stated
tens-to-low-hundreds per-scene wall/light count (Technical Context: Scale/Scope).

**Alternatives considered**: GPU shader-based shadow volumes — rejected
for v1 as unnecessary complexity given the entity counts involved; can be
revisited later as a pure rendering-layer optimization without changing
the wall/light data model, so it isn't foreclosed.

## 4. Undo scope

**Decision**: Session-local undo stack per authoring plugin (wall, light,
annotation each keep their own bounded stack), applied by re-issuing the
inverse mutation (e.g. undo of "move wall" re-issues "update_wall" with
prior coordinates) through the same GraphQL path as a normal edit.

**Rationale**: Spec Assumption already scopes undo to "current GM's
current editing session" (FR-012). Routing undo through the normal
mutation path (rather than a special client-only rollback) means undone
edits propagate to other clients exactly like any other edit — no new
sync mechanism needed, consistent with existing optimistic-mutation /
conflict-handling systems already in `systems/optimistic.rs` and the
Phase 4.9.C client-conflict-handling work.

**Alternatives considered**: Client-only local undo (no server round-trip)
— rejected because it would leave other connected clients' fog/light view
stale/wrong until their next unrelated sync.

## 5. RxDB collection pattern

**Decision**: Add `worldWallsCollection.ts`, `worldLightsCollection.ts`,
`worldAnnotationsCollection.ts` following the exact structure of the
existing `worldTokensCollection.ts` (schema, replication pull/push,
world-store wiring).

**Rationale**: Consistency with the established sync layer; no new sync
transport or conflict-resolution strategy is introduced by this feature.

**Alternatives considered**: Skipping RxDB and querying GraphQL directly
per canvas load — rejected, breaks the existing offline-capable /
optimistic-update pattern all other world entities rely on.

## 6. tldraw removal sequencing

**Decision**: Remove `WorldWhiteboard.tsx` and the `tldraw` package
dependency only after `AnnotationPlugin` (User Story 3, P3) reaches
parity with the freeform-drawing capability tldraw currently provides —
not at the start of the feature. Walls (P1) and lighting (P2) do not
depend on tldraw today (it was never wired to walls/lighting), so they can
ship and be used independently and first.

**Rationale**: Matches FR-013 (each tool independently usable) and avoids
a period where GMs have neither the old drawing tool nor a working
replacement.

**Alternatives considered**: Removing tldraw immediately and building
annotations first — rejected because it inverts the spec's stated
priority order (walls are P1 for a reason: the backend already exists and
is the highest-value gap).
