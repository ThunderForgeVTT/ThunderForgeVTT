# Feature Specification: Universal VTT (.dd2vtt) Map Import Fidelity & From-Scratch Map Editor Tooling

**Feature Branch**: `003-dd2vtt-map-fidelity`

**Created**: 2026-08-21

**Status**: Draft

**Input**: User description: "Improve dd2vtt (Universal VTT) map loading robustness and test coverage: add a true round-trip test proving scene data (walls, doors, lights, background image, shapes, tokens) survives persist-and-reload with no data loss, close known parsed-but-unused UVTT field gaps (portal rotation/freestanding, ambient light, movable-object occluders) or explicitly document them as out of scope, and decide whether map export/save-out (getting a scene back out as a .dd2vtt or similar file) is in scope for this spec or explicitly deferred." Extended: "look at our maps at vtt-maps, cherry pick a set of 5 of them and those will be our base note for the map editor tooling. A user should be able to create a map from scratch — building walls, doors, windows, torches, etc — all from our scene display tool in edit mode, but only the game master, and they should be able to do it on the fly mid-game in case a map needs tweaks. Default a blank scene as grassy-path-ambush.dd2vtt from that repo — it's our little gold standard."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A GM builds a map from scratch, live, during a session (Priority: P1)

A GM starts a scene with no imported map at all — just a background, or nothing yet — and, using the same canvas the players see (not a separate offline editor), places walls, toggles doors, adds windows, and places torches (light sources) directly, entirely by hand. This isn't a one-time setup step: a GM mid-session, realizing a corridor is missing a door or a room needs a torch for atmosphere, makes the tweak on the spot without pausing the game, restarting the server, or leaving the scene. Only the GM (the scene's owner) can do this; players never see these controls, only their effects (a wall blocks their vision, a lit torch illuminates a room).

**Why this priority**: This is the actual headline capability this spec exists to deliver — full from-scratch map construction, not just import-then-verify. Most of the underlying pieces already exist (spec 001/002 delivered hand-drawn wall/door/shape authoring and light-source placement, all GM-gated and live-synced), so this user story's job is to confirm the full "walls + doors + windows + torches, all mid-session" experience actually holds together end-to-end, and to close the one clearly-missing piece: **windows** are not yet a distinct authorable thing (today only a plain wall or a wall-with-a-door exists).

**Independent Test**: On an empty scene (no import), a GM places a wall, toggles part of it into a door, places a window, and places a torch (light source) — each individually, using only the live scene canvas — and confirms each appears immediately with correct behavior (wall blocks vision/movement; door is passable when open; window blocks movement but not vision; torch lights the area) and is visible with correct behavior to a second, concurrently-connected player session, without any server restart or page reload — independently verifiable without the round-trip/field-disclosure work in User Stories 2-3.

**Acceptance Scenarios**:

1. **Given** a GM on an empty scene, **When** they draw a wall, a door, and a window using the scene's authoring tools, **Then** all three appear immediately and behave distinctly: the wall blocks vision and movement, the door blocks only when closed, and the window blocks movement but not vision.
2. **Given** a GM on any scene, **When** they place a torch (light source) at a point on the canvas, **Then** it immediately illuminates the surrounding area for every connected session, GM and players alike.
3. **Given** an active session with a player connected and viewing a scene, **When** the GM adds a wall, door, window, or torch mid-session, **Then** the change appears for the player within a few seconds, with no reload, no disconnect, and no interruption to anything else happening in the session.
4. **Given** a player (non-GM) viewing a scene that has authoring controls available to its GM, **When** the player looks at the same scene, **Then** they see none of the wall/door/window/torch authoring controls — only the rendered results (walls blocking vision, lit areas, closed doors as obstacles).
5. **Given** a GM wants to start a brand-new scene with no imported map, **When** they create it, **Then** they can immediately begin placing walls/doors/windows/torches on it without first needing to import a `.dd2vtt` file — an empty or background-only scene is a fully valid starting point, not a degraded one.

---

### User Story 2 - A GM trusts that an imported or hand-built map is exactly what they see after any reload (Priority: P1)

A GM imports a `.dd2vtt` map into a scene, or builds one from scratch by hand (User Story 1): walls, doors, windows, light sources, and the background image all appear correctly. Days later, the GM (or a player) reloads the page, the server restarts, or the GM switches away from the scene and back. Today, nothing actually verifies that the scene the GM sees after any of these events is identical to what was there before — only that parsing a fixture file "succeeds" and produces the expected *counts* of walls/lights. A GM needs confidence that their table's map is durable: what's on it is what stays there, indefinitely, whether it arrived via import or by hand.

**Why this priority**: Equal priority to User Story 1 because it's the trust guarantee underneath it — a GM who spends a session hand-building a map (User Story 1) needs just as much confidence that it survives a reload as one who imported a file. Without a real test proving persisted map data survives a reload unchanged, a regression in any adjacent change (schema migration, serialization tweak, storage migration) could silently corrupt or drop map data with nothing catching it before a real GM does.

**Independent Test**: Import a `.dd2vtt` fixture into a fresh scene, record every wall segment (with door state), light source, and the background image's identity, then reload the scene's data from the database exactly as a fresh page load would, and confirm every recorded value matches exactly — verifiable independent of any UI, export capability, or the field-handling work in User Story 3.

**Acceptance Scenarios**:

1. **Given** a `.dd2vtt` fixture with multiple walls (including at least one door) and multiple light sources, **When** it is imported into a scene and the scene's data is then reloaded from persisted storage, **Then** every wall's start point, end point, and door state, and every light source's position and properties, are identical to what was imported — no additions, omissions, or value changes.
2. **Given** a `.dd2vtt` fixture with a background image, **When** the scene is reloaded after import, **Then** the background image reference resolves to the same visual image that was imported (not a placeholder, a different file, or a broken reference).
3. **Given** an imported map's walls and shapes are then edited by a GM through hand-drawn authoring (spec 002/User Story 1) — a wall added, a window placed, a shape deleted — **When** the scene is reloaded afterward, **Then** the reloaded state reflects the edits exactly (the edited/hand-built state is itself durable, not just the originally-imported state).
4. **Given** the round-trip verification from Scenario 1, **When** it is run repeatedly (e.g., in CI on every change), **Then** it reliably passes or fails based on actual data fidelity, with no flakiness tied to timing, ordering, or environment.

---

### User Story 3 - A GM is never silently missing part of their map (Priority: P2)

Some fields defined in the Universal VTT format are read by the importer but never actually used anywhere after that: a portal's rotation and whether it's "freestanding" (not attached to a wall segment), a map's ambient lighting hint, and object-based line-of-sight occluders (blocking shapes that aren't walls). Today a GM importing a map that relies on any of these gets no indication that part of their map didn't come through — the import reports success either way. A GM needs to either see that data actually work correctly, or be told plainly that it was skipped, so they can manually recreate it instead of discovering the gap mid-session (which User Story 1 now makes easy to do).

**Why this priority**: Lower priority than User Stories 1-2 because it's about import completeness/transparency for a known-narrow set of fields, not the core authoring or durability guarantees. Still valuable because silent partial import is a trust-eroding failure mode — a GM who finds out mid-session that a trap-blocking occluder was never imported has already lost time and immersion.

**Independent Test**: Import a `.dd2vtt` fixture that exercises each of the three currently-unused field categories (a freestanding portal, an ambient-light hint, an object-based occluder) and confirm the import result clearly communicates, per field category, whether it was applied or skipped — independently verifiable without User Story 2's persistence-round-trip machinery.

**Acceptance Scenarios**:

1. **Given** a `.dd2vtt` file containing a freestanding portal (not attached to a wall's line-of-sight polygon), **When** it is imported, **Then** the system either creates a correct, usable door/wall from it, or the import result explicitly reports that this portal was not imported and why.
2. **Given** a `.dd2vtt` file containing an `environment.ambient_light` value, **When** it is imported, **Then** the system either applies it to the scene's lighting, or the import result explicitly reports that ambient light was not imported.
3. **Given** a `.dd2vtt` file containing `objects_line_of_sight` occluder shapes, **When** it is imported, **Then** the system either creates corresponding vision-blocking geometry, or the import result explicitly reports that these occluders were not imported.
4. **Given** a `.dd2vtt` file that uses none of these three field categories (the common case today), **When** it is imported, **Then** the import result shows no skipped-field notices at all — the reporting only appears when relevant, never as noise.

---

### Edge Cases

- What happens if a non-GM (player) attempts to invoke wall/door/window/torch authoring directly (e.g. a crafted request, not through the UI)? Must be rejected server-side, not just hidden client-side — consistent with the existing GM-only enforcement already in place for walls/shapes (spec 001/002) and light sources.
- What happens if a GM edits a map (adds a wall, places a torch) while another GM-role user (a co-owner/invited GM, per spec 002's `world_members`) is doing the same on the same scene at the same time? Same as existing wall/shape behavior: last-write-wins on overlapping edits to the same element; each completed edit persists independently as soon as it's finished (no new conflict-resolution model introduced).
- What happens to an in-progress wall/window chain if the GM's browser tab loses focus or the connection drops mid-draw? Same as existing wall-authoring behavior (spec 002): nothing partial is persisted.
- What happens when a GM places a window directly overlapping or replacing part of an existing wall? Out of scope for this spec to define exact geometric merge behavior — treated the same as two overlapping walls today (both persist as drawn; the GM is responsible for placement), unless planning finds an existing constraint that already governs this.
- What happens when a `.dd2vtt` file's format version is not `0.3`? Already handled today (explicitly rejected before any data is written) — this spec's round-trip verification (User Story 2) must not regress that rejection.
- What happens if a scene is reloaded while an import is still in progress? Out of scope for this spec — import is already a single synchronous request; this spec only verifies data already committed.
- What happens if the underlying background-image storage (RustFS, per spec 002) is temporarily unreachable during a round-trip verification run? The verification should surface this as a clear infrastructure failure, distinct from a genuine data-fidelity failure, so a flaky environment doesn't get mistaken for a real regression (or vice versa).
- What happens to a scene's existing map data if a `.dd2vtt` re-import is performed on a scene that already has walls/lights from a prior import (including ones the GM hand-built via User Story 1)? Out of scope for this spec (re-import/merge semantics are a separate concern from verifying that data survives reloads) — assumed to follow whatever the existing import endpoint already does today, unchanged.

## Requirements *(mandatory)*

### Functional Requirements

**From-scratch map editor tooling (User Story 1)**

- **FR-001**: A GM MUST be able to author a **window** on a scene — a wall-like segment that blocks movement but, unlike a plain wall, does not block vision — using the same direct-on-canvas interaction already available for plain walls and doors (spec 002).
- **FR-002**: A GM MUST be able to select an existing wall segment and set it to plain wall, door, or window, and change it between those states, consistent with the existing door-toggle interaction (spec 002).
- **FR-003**: A GM MUST be able to place a light source ("torch") directly on the canvas at any point, with immediate, live-visible illumination — reusing the existing light-source placement capability (spec 001) under this feature's "torches, placed by hand, mid-session" framing, extending it only where User Stories 1-3's acceptance scenarios reveal a real gap.
- **FR-004**: All wall/door/window/torch authoring MUST be usable on a scene with no imported map at all — an empty or background-only scene MUST be a fully valid starting point for hand-built map construction, not a degraded one requiring an import first.
- **FR-005**: Every wall/door/window/torch change a GM makes MUST propagate to every other connected session (GM or player) viewing the same scene within a few seconds, without requiring a page reload — consistent with the existing real-time sync already used for walls/shapes/lights.
- **FR-006**: Only the scene's GM (owner, or a co-owner/invited GM per spec 002's `world_members`) MUST be able to author walls/doors/windows/torches; this MUST be enforced server-side, not only hidden in the client UI, consistent with existing GM-only enforcement for walls/shapes/lights.
- **FR-007**: A GM MUST be able to perform all of the above (add/edit/remove walls, doors, windows, torches) while a session is actively in progress (players connected, viewing the scene), without needing to pause, restart, or otherwise interrupt the session.

**Round-trip durability (User Story 2)**

- **FR-008**: The system MUST provide an automated, repeatable verification that every wall (including door/window state), light source, shape, and token belonging to a scene survives being persisted and reloaded with all field values identical to what was written — not merely that the right *number* of each entity exists.
- **FR-009**: The round-trip verification in FR-008 MUST cover data that originated from a `.dd2vtt` import specifically (not only data created through other authoring paths), since import is the primary way map data enters the system today.
- **FR-010**: The round-trip verification in FR-008 MUST include the scene's background image reference, confirming it resolves to the same image content that was imported.
- **FR-011**: The round-trip verification in FR-008 MUST also cover a scene whose data has since been modified through hand-drawn authoring — walls, doors, windows, torches, shapes (User Story 1, spec 002) — confirming edited/hand-built state, not just originally-imported state, survives a reload.

**Field-gap disclosure (User Story 3)**

- **FR-012**: The system MUST NOT report a `.dd2vtt` import as fully successful if any recognized-but-unhandled field category (freestanding portals, ambient light, object-based occluders) was present in the source file and not applied to the resulting scene — the import result MUST distinguish "fully imported" from "imported with some fields skipped."
- **FR-013**: When the import result reports skipped fields (FR-012), it MUST identify which field category was skipped, without requiring the GM to inspect server logs or source code to find out.
- **FR-014**: A `.dd2vtt` file that does not use any of the currently-unhandled field categories MUST continue to import and report success exactly as it does today, with no new skipped-field noise introduced by this feature.
- **FR-015**: The existing rejection of unsupported UVTT format versions (only `0.3` is accepted) MUST continue to function unchanged after this feature's changes.
- **FR-016**: Map export (producing a `.dd2vtt` or other file from a scene's current state) is explicitly OUT OF SCOPE for this feature — see Assumptions.

### Key Entities *(include if feature involves data)*

- **Wall segment**: Already-persisted entity (spec 001/002), currently a two-state segment (plain wall / door). This feature adds a third state — **window** (blocks movement, not vision) — to the same entity, not a new table.
- **Light source ("torch")**: Already-persisted entity (spec 001), already hand-placeable. This feature does not change its shape; "torch" is this spec's user-facing framing for GM-placed light sources during from-scratch map building.
- **Shape annotation, token**: Already-persisted entities; included in the round-trip verification per FR-008 for completeness, though they are not themselves produced by `.dd2vtt` import.
- **Canvas image asset (background image)**: Already-persisted entity (spec 002); this feature verifies the scene's reference to it survives a reload and still resolves correctly, and includes the cherry-picked reference fixtures (see Assumptions) as its canonical examples/test data.
- **Import result**: Extended (not newly introduced) to be able to carry a per-field-category "skipped" notice, per FR-012/FR-013 — a GM-visible signal, not just an internal log line.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can go from an empty scene to a room with at least one wall, one door, one window, and one lit torch, entirely by hand, in under a few minutes, with zero import step required.
- **SC-002**: A change a GM makes to walls/doors/windows/torches mid-session is visible to a connected player within the same few-seconds window already achieved for existing wall/shape/light sync (spec 001/002) — no perceptible new lag introduced by adding windows/torches to the authoring surface.
- **SC-003**: 100% of a `.dd2vtt` fixture's walls (with door/window state), lights, and background image reference match exactly after a persist-and-reload cycle, verified by an automated check that runs on every relevant code change, not just manually.
- **SC-004**: A GM importing a map that uses none of the three currently-unhandled field categories sees identical import-success behavior before and after this feature ships (zero regression, zero new noise).
- **SC-005**: A GM importing a map that does use one of the three currently-unhandled field categories can determine, from the import result alone (no server access), which part of their map was not imported.
- **SC-006**: The round-trip verification from SC-003 fails reliably and specifically when a real data-fidelity bug is deliberately introduced (validated at least once by intentionally breaking a field and confirming the check catches it), so the check is known to have teeth, not just be present.

## Assumptions

- **Map export/save-out is deferred, not in scope.** The user request asked this to be explicitly decided rather than left ambiguous: producing a `.dd2vtt` (or other) file from a scene's current state is a materially larger feature (a full serialization-out path, format fidelity in the reverse direction, a new UI affordance) than this spec's actual focus. The round-trip verification in User Story 2 proves durability via internal persist-and-reload, not via a file-based export/re-import cycle — it does not require an export capability to exist.
- **Closing the three known UVTT field gaps (freestanding portals, ambient light, object occluders) is deferred to "detect and disclose," not "fully implement."** Building correct handling for all three (e.g., real vision-blocking geometry for object occluders, ambient-light rendering) is separate follow-on work with its own design questions; this spec's obligation is that a GM is never left unaware that a field was skipped, per FR-012/FR-013. A future spec may pick up implementing any of them fully. Notably, none of the 64 real-world map files surveyed while picking this spec's reference fixtures actually used `freestanding` portals or `objects_line_of_sight` — testing those two will need a hand-crafted fixture, not a real one.
- **The five newly cherry-picked reference fixtures** (`examples/maps/`: `grassy-path-ambush.dd2vtt`, `azheim-meeting.dd2vtt`, `road-side-in.dd2vtt`, `dwarven-forge.dd2vtt`, `little-fish-academy.dd2vtt`, alongside the two already present from spec 001) are this feature's canonical test/reference data, chosen for variety: two background-only "blank canvas" fixtures, one richly-detailed multi-room building (walls/doors/lights), one walls-only dungeon, and one with a non-default `ambient_light` value.
- **`grassy-path-ambush.dd2vtt` is the project's reference "blank scene" gold standard, not an auto-applied production default.** It has zero walls/portals/lights — background art only — which is exactly the shape of a scene ready for from-scratch authoring (User Story 1). It's used as: (a) the canonical example when demonstrating/testing "start from nothing and build by hand," and (b) an optional local-dev/demo starter a GM may choose. It is explicitly **not** made the automatic background for every newly created scene in production — `examples/maps/README.md` already documents this whole directory as dev/test-only due to unconfirmed DungeonDraft asset licensing, and auto-applying a specific licensed art asset to every user's new scene by default would contradict that existing constraint. A genuinely license-clear default background (or none at all, i.e. today's actual blank/empty default) remains the production behavior unless a future decision explicitly revisits asset licensing.
- **The "edit mode" the user description asked for already exists implicitly, not as a separate mode to build.** GM-only authoring tools (walls/shapes/lights) are already always available to a scene's owner whenever they're viewing it (spec 001/002) — there is no separate "play mode" a GM has to leave to start editing, and this spec does not introduce one. User Story 1 is about closing the window/torch-as-scratch-authoring gaps within that existing always-available-to-GM model, not about building a new mode-switching UI. If planning finds a real product reason to add an explicit mode toggle (e.g. to visually distinguish "I am about to edit the map" from normal play), that's a scope decision for `/speckit-plan`, not assumed here.
- Existing `.dd2vtt`/UVTT format support (walls, portals attached to walls treated as doors, lights, background image, format version `0.3` only) is unchanged by this feature except for the field-category disclosure in User Story 3.
- This feature builds on spec 001 (native canvas authoring, wall/light/shape data model, hand-drawn authoring) and spec 002 (canvas image asset storage, `.dd2vtt` background-image migration to RustFS, closing most of the hand-drawn-authoring gap) — both already merged.
