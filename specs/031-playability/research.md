# Phase 0 Research: Playability 001

Each entry resolves an unknown from Technical Context or an assumption the spec
recorded but did not settle. Findings marked **verified** were checked against
the running system or the source during the 2026-09-01 playtest.

---

## R1 — Preload versus the server-authoritative active scene

**Decision**: Preload prepares *without* changing the table's active scene.
Launch is the only action that changes what players see.

**Rationale**: ADR-046 makes the active scene server-authoritative and
broadcast, so anything that sets it is immediately visible to every connected
player. The spec's requirement is "sets the scene without launching the field"
and its success criterion (SC-004) is that no connected player observes a
change — those are only simultaneously satisfiable if Preload does not touch
the active scene. Preload therefore means: fetch and warm this scene's content
on the GM's client so that a later Launch is instant.

**Alternatives considered**:
- *Set the active scene, don't navigate the GM.* Rejected: players would see
  the new scene while the GM is still preparing, defeating the purpose.
- *A "staged scene" concept distinct from the active scene.* Deferred. It is a
  genuine third option and a larger one — a second piece of server state with
  its own broadcast rules. If Preload-as-warming proves insufficient in play,
  this is the escalation, and it needs its own ADR.

**Existing machinery**: spec 028's world cache prefetch already fetches and
stores a world's art; Preload is that, aimed at a chosen scene.

---

## R2 — How a token survives a scene change

**Decision**: Deferred to an ADR, which must land with the feature (Constitution
Check IV.1). Research narrows it to two candidates.

**Constraint (verified)**: ADR-040 unified the token backing store onto the
**scene-scoped `tokens` table**. A token belongs to exactly one scene. So
"bring the party" cannot simply mean "leave them where they are".

**Candidate A — re-create on arrival**: on scene change, create tokens in the
new scene for the selected player characters, preserving art, ownership and
size but not position. Simple, no schema change, and honest about what is
happening. Costs: token identity is not preserved, so anything referencing a
token id across the change breaks.

**Candidate B — party membership**: introduce the notion of a token that
follows the party, resolved per scene. Preserves identity and reads better
conceptually, but adds a second way tokens come into existence and touches the
ownership boundary ADR-040 settled.

**Recommendation**: A, unless something already depends on token identity
across scenes. The spec's edge case — a character who already has a token in
the destination scene — must be answered either way (do not duplicate).

---

## R3 — Item pickup as a contributed effect

**Decision**: `item.pickup` is contributed by the **item subsystem**, not added
to the interaction plugin.

**Rationale (verified)**: ADR-054 establishes that `InteractionPlugin` owns
placement, hit-testing, trigger detection, permission resolution and `once`
bookkeeping, and **owns no effect at all**. Doors are contributed from
`wall.rs`; lore contributes `lore.open`. `scripts/verify.mjs` greps the
interaction plugin for subsystem words to enforce this. Adding pickup to the
core would fail that check by design.

**Verified today**: the declared effect vocabulary is `door.reveal`,
`door.set_lock`, `door.set_state`, `light.toggle`, `lore.open`,
`nav.request_scene`. There is no `item.*` effect — this is net-new.

**Consequence for FR-016 (concurrent pickup)**: ADR-054 also says the engine may
apply the visible change optimistically but the server stays authoritative. A
refused pickup must restore the token. The race is the same shape as spec 017's
character-claim race (two claimants, exactly one winner), which is settled at
the database boundary — reuse that pattern rather than inventing one.

---

## R4 — Actor imagery: two columns or rows keyed by role

**Decision**: rows keyed by role. Recorded as an ADR (Constitution Check IV.2).

**Verified**: `world_actors` has **no image columns at all** (id, world_id,
scene_id, actor_type, game_system_id, label, created_by, owned_by, is_public,
is_npc, created_at, updated_at, description, available_for_claim). By contrast
`world_items` already has `icon_asset_id` — an asymmetry worth resolving in
passing.

**Rationale**: the deferred VTuber set (talking, not-talking, background) is
*n* images per actor. Two scalar columns would force that later feature into a
second, parallel mechanism. Rows keyed by role cost little now and make the
later set additive. This is the cheapest decision to get right and an expensive
one to get wrong.

**Existing machinery (verified)**: `storage/transcode.rs` provides
`transcode_to_webp`, `transcode_scene_preview` and `transcode_to_lore_renditions`;
`graphql/mutations_lore_images.rs` is a complete precedent — size limit,
transcode to webp renditions, `write_object` into RustFS, permission checks, and
tests covering the happy path, a viewer-level caller and an oversized upload.
Lore images already produce two renditions from one upload, which is
structurally what portrait + token needs.

---

## R5 — Item price versus a system's own economy

**Decision**: the generic price is **presentational** — a value a GM records to
role-play from. Systems keep their own economies.

**Verified**: `world_genie_shop_listings` already models pricing with
`price_kind`, `price_resource_type`, `price_resource_amount`, `price_item_id`,
`price_item_quantity`, keyed by `actor_id` + `item_id`. Genie put its economy in
its **own table** rather than in `world_items` — the strongest existing evidence
that pricing is system-specific.

**Granularity difference that must not be glossed**: genie's pricing is
**per-vendor** (this NPC sells this item at this price); a GM "suggested price"
is **per-item** and vendor-independent. They can coexist, but the generic layer
must not reimplement the vendor model, and a system's view should be free to
ignore or override the suggested price.

---

## R6 — The stray marker on tool switch

**Decision**: treat as input routing until proven otherwise; confirm before
fixing.

**Evidence (verified)**: the real `<canvas>` is inserted by Bevy/winit as a
child of `<body>` with `position: fixed`; the React tool rail is drawn over it.
The defect occurs for every tool **except text**, and text is the only shape
sub-tool handled in the DOM — every other tool is engine-side. The one tool that
does not respond to an engine-level click is exactly the one that does not
misfire.

**Cheapest confirmation**: check whether the stray marker lands at the tool
*button's* screen position rather than where the pointer last was on the map. If
so, the rail's click is reaching the engine.

**Note for FR-029**: canvas right-click is being added in the same feature.
Whatever suppresses the browser context menu must be scoped to the canvas and
must not deepen this routing problem.

---

## R7 — Browser support, and the cache reporting nothing on Firefox

**Decision**: the supported-browser set must be stated before FR-042 can be
implemented. This is a product decision, not an engineering one, and is called
out as a prerequisite rather than assumed.

**Evidence (verified)**: on a Firefox session against a world whose active
scene has two placed, content-hashed assets, the diagnostics panel reported
nothing served and 0 B downloaded. The boring explanations were ruled out — the
assets exist, are hashed, are placed, and sit on the world's active scene; the
other scene in that world has none.

**Why it was invisible**: `playwright.config.ts` declares exactly one project,
`chromium`. Every e2e run is Chromium. `thunderforge-cache-browser` raises
`CacheError::Unsupported` for a missing WebCrypto/IndexedDB/navigator and notes
that OPFS's fast path (`createSyncAccessHandle`) is worker-only — it is built to
degrade rather than crash, which is exactly what an all-zeros panel looks like.

**Stakes**: spec 028's offline, peer-distribution and server-isolated
adjudication features all rest on this cache. If it silently no-ops in a
browser, so do they.

**Actions**: (a) determine the intended browser set; (b) confirm the Firefox
behaviour against the console; (c) either support it or report inability
plainly per FR-042; (d) consider whether the e2e suite should gain a second
project, noting that would multiply an already-27-minute run.

---

## R8 — Snapping across grid types

**Decision**: snapping is a rule in `thunderforge-canvas-core`, consumed by the
engine, parameterised by the scene's grid type.

**Rationale**: it is pure maths with a correctness answer, and the engine crate
cannot execute its own tests under wasm32. Putting the rule in the native crate
makes hex correctness testable in milliseconds instead of through a browser.

**Existing machinery**: tokens already snap (`snapWorld` in the token specs), and
a Default Scene Grid Type control exists from spec 022 US4, so the grid kind is
already a known, authored property. What is missing is a GM-facing toggle and
application to walls and lights.

---

## R9 — Two loaders on the play view

**Decision**: the route-level fallback and the engine loader must not be
visible simultaneously; one owns "not ready yet".

**Verified**: three loading surfaces exist on that route — the route Suspense
fallback (`renderLazyPage(<WorldPage />, "Loading world workspace")`, full
screen), the `EngineLoader` (`engine-load-indicator`, "Downloading the game
engine…" / "Starting the engine…"), and `scene-load-indicator` ("Loading
scene…"). The engine and scene loaders are sequential and legitimate; the
overlap is the route fallback with the engine loader.

**Test worth adding with the fix**: "exactly one loading indicator is visible at
any moment" (SC-007). The existing `engine-loading.spec.ts` asserts that *a*
loader appears within 1s and nothing asserts uniqueness — which is why this
shipped.

---

## R10 — Selection filter persistence

**Decision**: per-user, per-device preference; not world state.

**Rationale**: it is a working preference of the person at the keyboard, not a
property of the world, and two GMs on the same world should not fight over it.
Nothing else about it needs to be authoritative or shared, so the server does
not need to hold it.

**Consequence**: FR-009's "persist across sessions" is satisfied by durable
client-side storage. A GM on a second machine starts from the default (all
kinds on), which is the safe state.

---

## R11 — Engine modes belong to `bevy_state`

**Decision**: enable Bevy's `bevy_state` feature and express placement, scene
transition and authoring mode as states. Do not build a bespoke mode flag, and
do not let React chrome hold the active mode.

**Verified**:
- `bevy_state` is a real feature of Bevy 0.19.1
  (`bevy_state = ["bevy_internal/bevy_state"]`).
- It is **absent from this project's feature list** in `src/engine/Cargo.toml`.
- There is **no** `States` / `NextState` / `OnEnter` / `OnExit` / `in_state`
  usage anywhere in `src/engine/src`.

So the idiomatic machinery exists, is already a dependency's feature, and is
switched off.

**Rationale**: three behaviours in this feature are modes with transitions
rather than booleans.

- *Placement* — `OnExit(carrying)` guarantees "leave no trace" once, instead of
  at every path out of a carry, including the dropped-connection edge case.
- *Scene transition* — `OnEnter`/`OnExit` own unload and load, so no system has
  to ask whether the switch has finished.
- *Authoring mode* — gives FR-040a its "exactly one authority" by construction.

**Why authoring mode matters most.** R6 records that switching tools places a
stray marker for every tool *except* text — and text is the only tool handled
in the DOM rather than the engine. That is consistent with the active tool
living in chrome while the engine acts on ambient input. An engine-owned mode,
changed only by explicit transition, addresses the class; a patch to the rail's
click handling would address one symptom.

This remains a hypothesis about the defect's cause. R6's confirmation step
still applies, and this decision stands on its own merits regardless of what
that confirms.

**Alternatives considered**:
- *A plain resource holding the current tool.* Rejected: it is a state machine
  with the transitions left implicit, which is where the ordering bugs live.
- *Keep the mode in React and send it down.* Rejected under Principle I —
  canvas state with two owners is the exact failure the constitution was
  written about.
- *A new `thunderforge-rx` crate for client state.* Rejected as a much larger
  answer to a different question. Offline sync is already solved by spec 028
  (OPFS cache, offline queue, reconcile, catch-up); session replay and
  telemetry are server-side concerns — `world_events` carries a payload only
  for token events today, so richer replay is a server schema question, not a
  client state machine.

**Cost**: one feature flag, plus the discipline of using it. `bevy_state` is
small relative to what is already compiled in, but the bundle delta should be
measured when it lands, consistent with how the release bundle is already
tracked.

**Explicit non-goals**: `bevy_state` does nothing for offline sync, session
replay, or app tracking. It is for engine modes only.

