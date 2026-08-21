# Feature Specification: Native Canvas — Full tldraw Replacement (Walls, Map Import, Lighting, Shapes)

**Feature Branch**: `001-bevy-canvas-authoring`

**Created**: 2026-08-20
**Updated**: 2026-08-20 — expanded from an authoring-tools-only scope to a
full replacement of tldraw: its shape/drawing tools, plus a new map-import
capability (Universal VTT / `.dd2vtt`) that was not in the original draft.
This is a hard scope expansion, not a rewording — see Success Criteria and
User Story 2 (new) below.

**Status**: Draft

**Input**: User description: "This is a full replacement for tldraw — its
shapes, its walls, its maps, the works. Part of this spec is being able to
load `.dd2vtt` maps (Universal VTT format). We want to support that spec
out of the gate, in a layered canvas built with Bevy."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - GM authors walls that block vision and movement (Priority: P1)

A Game Master, while preparing or running a scene, draws wall segments directly on the game canvas to mark doors, room boundaries, and obstacles. Each wall can be configured to block line-of-sight, movement, or both. Once drawn, walls immediately affect what players can see (fog of war / vision) and where tokens can move.

**Why this priority**: Walls are the foundation of fog-of-war and tactical play; the backend already exists (Phase 6 walls API) but is unusable without an in-canvas authoring tool. This is the highest-value, highest-urgency gap and the direct reason tldraw is being retired first for this workflow. It is also the rendering/occlusion foundation every other story in this spec (import, lighting) builds on.

**Independent Test**: A GM can open a scene, draw a wall segment between two points, mark it as vision-blocking, and confirm that a player's view is occluded beyond that wall without any other tool (import, lighting, shapes) being present.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene canvas, **When** they select the wall tool and click-drag from point A to point B, **Then** a wall segment is created between A and B and persists after the GM navigates away and returns.
2. **Given** an existing wall segment, **When** the GM selects it and toggles "blocks vision" off, **Then** players on the far side of that segment can immediately see through it.
3. **Given** an existing wall segment, **When** the GM drags one of its endpoints to a new position, **Then** the wall's position updates for all connected clients within a few seconds.
4. **Given** an existing wall segment, **When** the GM deletes it, **Then** it no longer blocks vision or movement for any client.
5. **Given** a player (non-GM) is viewing the same scene, **When** they look at the canvas, **Then** they do not see wall-editing handles or a wall tool, only the resulting vision/movement effects.
6. **Given** an existing wall segment, **When** the GM marks it as a door and sets it open, **Then** it stops blocking vision/movement while open, and resumes blocking when closed, without needing to be deleted and redrawn.

---

### User Story 2 - GM imports a Universal VTT (`.dd2vtt`) map (Priority: P2)

A Game Master imports a map file exported in the Universal VTT format
(`.dd2vtt`, produced by DungeonDraft and consumed by most VTT platforms)
into a scene. The import brings in the background art, wall/vision
geometry, doors, and light sources already authored in that file, so the
GM does not have to manually retrace them by hand with the wall and
lighting tools from User Story 1 / User Story 3.

**Why this priority**: This is explicitly called out as required "out of
the gate" — a large library of existing, pre-authored maps in this format
is the primary way scene content will be populated, and manually
retracing walls/lights for every imported map would make User Story 1's
tools far less valuable in practice. It is P2 (not P1) because it depends
on User Story 1's wall/occlusion rendering already existing to be
verifiable end-to-end, and a scene can still be built by hand without it.

**Independent Test**: With User Story 1 shipped, a GM can import a sample
`.dd2vtt` file (e.g. `examples/maps/demo.dd2vtt`) into a new scene and,
without drawing anything by hand, see the background art appear with
matching walls (occluding player vision correctly) and lit areas, and
confirm the imported walls/lights can subsequently be edited with the same
tools User Story 1/3 provide (import is a starting point, not a
read-only/locked state).

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene, **When** they choose to import a
   `.dd2vtt` file, **Then** the scene's background shows the map art from
   that file, correctly scaled to the scene's grid.
2. **Given** a `.dd2vtt` file with wall/vision geometry, **When** it is
   imported, **Then** wall segments matching that geometry exist on the
   scene and block player vision the same as a hand-drawn wall would.
3. **Given** a `.dd2vtt` file with doors, **When** it is imported, **Then**
   each door imports as a wall marked as a door, in the open/closed state
   recorded in the file.
4. **Given** a `.dd2vtt` file with light sources, **When** it is imported,
   **Then** light sources matching that file's position/radius/color exist
   on the scene and are occluded by the imported walls.
5. **Given** an imported wall or light, **When** the GM edits or deletes it
   with the normal wall/lighting tools, **Then** it behaves identically to
   a hand-authored one — import does not create a special locked or
   read-only object type.
6. **Given** a `.dd2vtt` file that declares a format version this system
   does not support, **When** the GM attempts to import it, **Then** the
   import is rejected with a clear error and no partial scene data is
   created.
7. **Given** a non-GM user, **When** they attempt to import a map into a
   scene they do not own, **Then** the import is rejected server-side.

---

### User Story 3 - GM places and manages lighting sources (Priority: P3)

A Game Master places light sources on the canvas (e.g., a torch, a magical glow, a window) with a position, radius, and optional color/intensity. Light sources illuminate the area within their radius, are occluded by vision-blocking walls the same way player sight is, and can be attached to a token (so the light moves with a torch-bearer) or left static in the scene.

**Why this priority**: Lighting is the natural pillar of atmospheric, tactical fog-of-war play and directly depends on User Story 1's wall/occlusion foundation. Imported maps (User Story 2) already populate lights automatically, so hand-authoring is for scenes without an import source or fine-tuning after one — the game is usable without it.

**Independent Test**: With walls already authored (Story 1), a GM can place a static light source in a room and confirm the room is lit while an adjacent, wall-separated room remains dark; the light can then be deleted and the room returns to its prior (unlit or GM-default) state.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene canvas, **When** they select the lighting tool and click a point, **Then** a light source is created at that point with a default radius and appears lit on the canvas.
2. **Given** an existing light source, **When** the GM adjusts its radius or intensity, **Then** the illuminated area updates for all clients within a few seconds.
3. **Given** a light source and a vision-blocking wall between it and a room, **When** rendering is evaluated, **Then** the room on the far side of the wall is not illuminated by that light.
4. **Given** a light source attached to a token, **When** the token moves, **Then** the light moves with it.
5. **Given** a player (non-GM) is viewing the same scene, **When** they look at the canvas, **Then** they see the illumination effect but not lighting-editing handles or a lighting tool.

---

### User Story 4 - GM draws and manages shapes/annotations, full tldraw parity (Priority: P4)

A Game Master draws freeform marks, shapes (rectangle, ellipse, line/arrow,
freehand stroke), and text labels on the scene canvas — the full range of
drawing tools tldraw previously provided — to call out points of interest,
sketch a plan, or leave GM-only notes, using tools native to the game
canvas instead of a separate embedded editor. Once this reaches parity
with tldraw's drawing capability, tldraw and its wrapper component are
removed from the project entirely.

**Why this priority**: Annotation/shape drawing is useful but is the least
tactically critical of the four capabilities and has the most viable
fallback (GMs can describe things verbally or use out-of-band tools) while
walls, import, and lighting are being built. It is also the exit condition
for tldraw's removal, so it is sequenced last on purpose — the project is
never left without a working drawing tool.

**Independent Test**: A GM can draw a freehand stroke, a rectangle, and a
text label on a scene canvas, have each persist across a page reload, move
or resize each, and remove each, independent of whether any walls, lights,
or imported map data exist on that scene.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene canvas, **When** they select a shape
   tool (rectangle, ellipse, line/arrow, freehand, or text) and draw on
   the canvas, **Then** the corresponding shape appears and persists after
   reload.
2. **Given** an existing shape, **When** the GM selects it, **Then** they
   can move, resize (where applicable), and restyle it (color/line weight)
   without recreating it.
3. **Given** an existing shape, **When** the GM selects and deletes it,
   **Then** it is removed for all clients.
4. **Given** a shape marked GM-only, **When** a player views the same
   scene, **Then** the shape is not visible to them.
5. **Given** a shape marked visible-to-players, **When** a player views
   the same scene, **Then** they see the shape but cannot select, move, or
   delete it.
6. **Given** all four user stories in this spec are complete and this
   story's shapes are verified at parity with tldraw's prior drawing
   capability, **When** the project is inspected, **Then** no page,
   component, or dependency in the codebase references tldraw.

---

### Edge Cases

- What happens when a GM draws a wall or places a light with zero length/radius (e.g., a click without drag)? The system should reject or ignore the degenerate shape rather than persist an invisible, unremovable object.
- How does the system handle two GMs (or a GM with two open tabs) editing the same wall, light, or shape simultaneously? The later write should win without corrupting the object, consistent with existing scene-mutation conflict handling.
- What happens to tokens that become "trapped" inside a movement-blocking wall shape after a GM draws or edits walls around them (by hand or via import)? Movement blocking should not retroactively teleport or destroy tokens; it only constrains future movement.
- What happens when a light source's radius or a wall's endpoints extend outside the scene's defined bounds? The system should clamp or allow it without crashing, and rendering should not leak the effect into undefined space.
- How does the system behave when the authoring tools are used on a scene that has no walls at all yet? Vision should default to fully visible (or the scene's existing default fog behavior), not fail.
- What happens if a non-GM (player) attempts to invoke a wall, lighting, shape, or import mutation directly (e.g., via a replayed or forged request)? The request must be rejected server-side regardless of what the client UI shows.
- What happens when a GM undoes an action immediately after another client has already observed and reacted to it (e.g., a player's fog updated)? The undo should re-apply the prior state and propagate it like any other edit, not require a special-cased rollback.
- What happens when an imported `.dd2vtt` file's background image is very large (tens of megabytes)? The import must not crash the browser tab or the server request handler; it should either succeed within a reasonable time or fail with a clear "file too large" error rather than hanging.
- What happens when an imported `.dd2vtt` file's grid resolution (pixels-per-grid) does not match the target scene's existing grid size? Imported geometry must be scaled to the scene's grid so walls/lights land in the correct place, not just copied at face value.
- What happens if a GM imports a second `.dd2vtt` file into a scene that already has hand-drawn walls/lights/shapes? The import should add to the existing scene content, not silently delete or overwrite what's already there (a GM who wants a clean slate should clear the scene explicitly first).
- What happens when an imported file's `line_of_sight` polygon is malformed (e.g., fewer than 2 points, self-intersecting)? Degenerate segments are skipped/reported rather than crashing the import or corrupting the wall set.

## Requirements *(mandatory)*

### Functional Requirements

**Walls (User Story 1)**

- **FR-001**: The system MUST allow a GM to create, reposition, and delete wall segments directly on the scene canvas.
- **FR-002**: The system MUST allow a GM to configure, per wall segment, whether it blocks line-of-sight, movement, or both.
- **FR-003**: The system MUST recompute and propagate vision/fog-of-war effects to all connected clients within a few seconds of a wall being created, moved, reconfigured, or deleted.
- **FR-017**: The system MUST allow a GM to mark a wall segment as a door with an open/closed state; an open door does not block vision or movement even if its underlying wall would otherwise block them; state changes propagate like any other wall edit.

**Map Import (User Story 2)**

- **FR-018**: The system MUST allow a GM to import a Universal VTT (`.dd2vtt`, JSON-based, format version 0.3) file into a scene they own.
- **FR-019**: The system MUST convert an imported file's background image into the scene's background art.
- **FR-020**: The system MUST convert an imported file's wall/vision geometry into wall segments equivalent to hand-drawn ones (FR-001-003 apply to them identically post-import).
- **FR-021**: The system MUST convert an imported file's doors into door-flagged wall segments (FR-017) in the state recorded in the file.
- **FR-022**: The system MUST convert an imported file's light sources into light sources equivalent to hand-placed ones (User Story 3's requirements apply to them identically post-import).
- **FR-023**: The system MUST scale imported geometry to the target scene's grid so imported walls/lights/background align correctly regardless of the source file's grid resolution.
- **FR-024**: The system MUST reject import of a file whose declared format version is unsupported, with a clear error, and MUST NOT leave partially-created scene data behind on a rejected or failed import.
- **FR-025**: The system MUST enforce the same scene-ownership check on import as on any other scene-mutating action (FR-010 applies).
- **FR-026**: Imported walls, lights, and background art MUST remain fully editable/deletable by the normal wall/lighting/shape tools after import — import creates ordinary scene content, not a special locked type.

**Lighting (User Story 3)**

- **FR-004**: The system MUST allow a GM to create, reposition, resize (radius/intensity), and delete light sources directly on the scene canvas.
- **FR-005**: The system MUST occlude light from a source by any wall segment configured to block vision (including a closed door, per FR-017), consistent with how player vision is occluded, unless the GM has explicitly marked that light as non-shadow-casting (FR-027).
- **FR-006**: The system MUST support attaching a light source to a token so that the light's position follows the token's position.
- **FR-027**: The system MUST allow a GM to mark a light source as non-shadow-casting (an ambient/ubiquitous light that ignores wall occlusion), matching the equivalent flag on imported Universal VTT light sources (FR-022).

**Shapes / Annotations (User Story 4)**

- **FR-007**: The system MUST allow a GM to draw, reposition, resize, restyle, and delete shapes on the scene canvas, covering at minimum: freehand strokes, rectangles, ellipses, lines/arrows, and text labels — the set of tools tldraw previously provided for this purpose.
- **FR-008**: The system MUST allow a GM to mark an individual shape as GM-only or visible-to-players.

**Cross-cutting**

- **FR-009**: The system MUST NOT present wall, lighting, shape, or import authoring controls to non-GM (player) users; players only see the resulting rendered effects and any player-visible shapes.
- **FR-010**: The system MUST reject wall, lighting, shape, import, and GM-only-content mutations from non-GM users at the server, independent of what the client displays.
- **FR-011**: The system MUST persist walls, light sources, shapes, and imported background art per-scene so they are present on subsequent loads of that scene.
- **FR-012**: The system MUST support undo of the most recent GM authoring action (wall, light, shape, or door-state create/move/delete/toggle) within the current editing session.
- **FR-013**: Each of the wall, import, lighting, and shape capabilities MUST be independently usable — a scene using any subset of them (including none) MUST render and function correctly.
- **FR-014**: The system MUST continue to support existing token creation, selection, and movement on the canvas unchanged by the introduction of these capabilities.
- **FR-015**: The system MUST NOT depend on the previously wrapped third-party whiteboard editor (tldraw) for any of walls, import, lighting, or shapes going forward.
- **FR-016**: The canvas MUST render its content as an explicit, ordered set of layers (at minimum: background/map art, grid, walls, lighting, shapes, tokens, fog-of-war) so that layer order and per-layer GM/player visibility are consistent and predictable across all four capabilities in this spec, rather than each capability managing its own ad hoc draw order.

### Key Entities

- **Wall**: A line segment on a scene with a start point, end point, independent flags for whether it blocks vision and/or movement, and an optional door state (none / open / closed). Already has a persisted backend representation for the non-door fields (Phase 6); this feature adds authoring, rendering/occlusion, door semantics, and import population on top of it.
- **Light Source**: A point on a scene with a radius, intensity/color, and an optional attachment to a token (so it can be static or mobile). Determines what area is illuminated, subject to occlusion by vision-blocking walls (including closed doors). Populated either by hand or by map import.
- **Shape**: A freeform stroke, geometric shape (rectangle/ellipse/line/arrow), or text label placed on a scene, with a visibility flag (GM-only or visible-to-players), style (color/line weight), and ownership/authorship — the direct replacement for the whiteboard-document concept previously provided by tldraw.
- **Map Import**: A one-time ingestion of a Universal VTT (`.dd2vtt`) file into a scene, producing background art plus a batch of Wall and Light Source entities scaled to the scene's grid. Not persisted as its own ongoing entity — it is a creation event, not a live document; once imported, its output is indistinguishable from hand-authored content.
- **Canvas Layer**: An explicit ordering concept (background/map art, grid, walls, lighting, shapes, tokens, fog-of-war) that all four capabilities in this spec render into, rather than each owning its own ad hoc z-order.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can draw a wall and see a player's fog-of-war view update to reflect it in under 5 seconds, without a page reload.
- **SC-002**: A GM can light a room and have an adjacent, wall-separated room remain dark, verified visually with zero manual fog painting required.
- **SC-003**: A GM can complete "draw a wall around a room and place one light inside it" in under 2 minutes on a scene they have not annotated before.
- **SC-004**: 100% of wall/lighting/shape/import-authoring mutation attempts by a non-GM user are rejected, verified independent of client behavior.
- **SC-005**: A scene with zero walls, lights, shapes, or imported content continues to render and support token movement with no errors or degraded performance.
- **SC-006**: After this feature ships, zero remaining product surfaces depend on the previously wrapped third-party whiteboard editor.
- **SC-007**: A GM can import a representative `.dd2vtt` sample map (e.g. `examples/maps/demo.dd2vtt`, which contains background art, 8 wall polygons, 2 doors, and 12 lights) and have all of it appear correctly positioned on the scene in under 30 seconds, with zero manual retracing.
- **SC-008**: A GM can, using only this feature's shape tools, reproduce each of the five drawing operations tldraw's wrapper previously exposed (freehand stroke, rectangle, ellipse, line/arrow, text label) — verified against a fixed checklist of those five operations, one GM session, zero operations unreproducible.

## Assumptions

- "GM" refers to the existing scene/world-owner role already enforced elsewhere in the system (e.g., wall mutation ownership checks); no new role model is introduced by this feature.
- Real-time propagation ("a few seconds") follows the same synchronization pattern already established for tokens and walls, rather than requiring a new transport.
- Door open/closed toggling is GM-only in this iteration; whether players can open doors themselves is a follow-on decision, not required for this spec's completion.
- Undo scope is limited to the current GM's current editing session (not a persistent multi-user undo/redo history across reconnects).
- Light source falloff/rendering fidelity (e.g., soft edges vs. hard-edged radius) is a visual-quality detail left to implementation, not a functional requirement.
- Map import supports Universal VTT format version 0.3 (the version used by the provided example fixtures in `examples/maps/`); other exporters or older/newer format versions are out of scope unless they also produce 0.3-compatible output.
- Map import is a one-shot ingestion, not a live/two-way sync with the source file — re-importing the same file after the GM has edited its imported content will add a second copy, not merge or overwrite (consistent with the "imports add to the scene" edge case above).
- `objects_line_of_sight` entries (movable-object occluders in the source format) import as ordinary walls; distinguishing them as "attached to an object" for dynamic occlusion is out of scope for this spec.
