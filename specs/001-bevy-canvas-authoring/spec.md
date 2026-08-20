# Feature Specification: Native Canvas Authoring (Walls, Lighting, Annotations)

**Feature Branch**: `001-bevy-canvas-authoring`

**Created**: 2026-08-20

**Status**: Draft

**Input**: User description: "Replace the wrapped tldraw whiteboard with a native, canvas-owned authoring layer. GMs need to draw and manage scene annotations, walls (line-of-sight/movement blockers, already backed by the Phase 6 walls API), and lighting sources directly on the game canvas, with each tool built as an independently modular capability rather than a wrapped third-party editor. Players see the resulting fog-of-war/vision effects but only GMs get authoring tools. This supersedes the prior 'wrapped tldraw' decision and extends the existing canvas rendering architecture with wall and lighting authoring, alongside a native replacement for freeform annotation drawing. Token editing/selection already lives on the canvas and should not be redesigned here except where the new tools need to interoperate with it."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - GM authors walls that block vision and movement (Priority: P1)

A Game Master, while preparing or running a scene, draws wall segments directly on the game canvas to mark doors, room boundaries, and obstacles. Each wall can be configured to block line-of-sight, movement, or both. Once drawn, walls immediately affect what players can see (fog of war / vision) and where tokens can move.

**Why this priority**: Walls are the foundation of fog-of-war and tactical play; the backend already exists (Phase 6 walls API) but is unusable without an in-canvas authoring tool. This is the highest-value, highest-urgency gap and the direct reason tldraw is being retired first for this workflow.

**Independent Test**: A GM can open a scene, draw a wall segment between two points, mark it as vision-blocking, and confirm that a player's view is occluded beyond that wall without any other tool (lighting, annotations) being present.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene canvas, **When** they select the wall tool and click-drag from point A to point B, **Then** a wall segment is created between A and B and persists after the GM navigates away and returns.
2. **Given** an existing wall segment, **When** the GM selects it and toggles "blocks vision" off, **Then** players on the far side of that segment can immediately see through it.
3. **Given** an existing wall segment, **When** the GM drags one of its endpoints to a new position, **Then** the wall's position updates for all connected clients within a few seconds.
4. **Given** an existing wall segment, **When** the GM deletes it, **Then** it no longer blocks vision or movement for any client.
5. **Given** a player (non-GM) is viewing the same scene, **When** they look at the canvas, **Then** they do not see wall-editing handles or a wall tool, only the resulting vision/movement effects.

---

### User Story 2 - GM places and manages lighting sources (Priority: P2)

A Game Master places light sources on the canvas (e.g., a torch, a magical glow, a window) with a position, radius, and optional color/intensity. Light sources illuminate the area within their radius, are occluded by vision-blocking walls the same way player sight is, and can be attached to a token (so the light moves with a torch-bearer) or left static in the scene.

**Why this priority**: Lighting is the natural second pillar of atmospheric, tactical fog-of-war play and directly depends on User Story 1's wall/occlusion foundation, but the game is usable without it (scenes can run fully lit or with manual fog).

**Independent Test**: With walls already authored (Story 1), a GM can place a static light source in a room and confirm the room is lit while an adjacent, wall-separated room remains dark; the light can then be deleted and the room returns to its prior (unlit or GM-default) state.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene canvas, **When** they select the lighting tool and click a point, **Then** a light source is created at that point with a default radius and appears lit on the canvas.
2. **Given** an existing light source, **When** the GM adjusts its radius or intensity, **Then** the illuminated area updates for all clients within a few seconds.
3. **Given** a light source and a vision-blocking wall between it and a room, **When** rendering is evaluated, **Then** the room on the far side of the wall is not illuminated by that light.
4. **Given** a light source attached to a token, **When** the token moves, **Then** the light moves with it.
5. **Given** a player (non-GM) is viewing the same scene, **When** they look at the canvas, **Then** they see the illumination effect but not lighting-editing handles or a lighting tool.

---

### User Story 3 - GM draws freeform annotations on the scene (Priority: P3)

A Game Master draws freeform marks, shapes, and notes on the scene canvas (the capability tldraw previously provided) to call out points of interest, sketch a plan, or leave GM-only notes, using tools native to the game canvas instead of a separate embedded editor.

**Why this priority**: Annotation is useful but is the least tactically critical of the three capabilities and has the most viable fallback (GMs can describe things verbally or use out-of-band tools) while walls/lighting are being built.

**Independent Test**: A GM can draw a freehand mark or a labeled shape on a scene canvas, have it persist across a page reload, and remove it, independent of whether any walls or lights exist on that scene.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a scene canvas, **When** they select a draw tool and drag across the canvas, **Then** a freeform stroke appears and persists after reload.
2. **Given** an existing annotation, **When** the GM selects and deletes it, **Then** it is removed for all clients.
3. **Given** an annotation marked GM-only, **When** a player views the same scene, **Then** the annotation is not visible to them.
4. **Given** an annotation marked visible-to-players, **When** a player views the same scene, **Then** they see the annotation but cannot edit or delete it.

---

### Edge Cases

- What happens when a GM draws a wall or places a light with zero length/radius (e.g., a click without drag)? The system should reject or ignore the degenerate shape rather than persist an invisible, unremovable object.
- How does the system handle two GMs (or a GM with two open tabs) editing the same wall or light simultaneously? The later write should win without corrupting the object, consistent with existing scene-mutation conflict handling.
- What happens to tokens that become "trapped" inside a movement-blocking wall shape after a GM draws or edits walls around them? Movement blocking should not retroactively teleport or destroy tokens; it only constrains future movement.
- What happens when a light source's radius or a wall's endpoints extend outside the scene's defined bounds? The system should clamp or allow it without crashing, and rendering should not leak the effect into undefined space.
- How does the system behave when the authoring tools (wall/light/annotation) are used on a scene that has no walls at all yet? Vision should default to fully visible (or the scene's existing default fog behavior), not fail.
- What happens if a non-GM (player) attempts to invoke a wall, lighting, or GM-only annotation mutation directly (e.g., via a replayed or forged request)? The request must be rejected server-side regardless of what the client UI shows.
- What happens when a GM undoes an action immediately after another client has already observed and reacted to it (e.g., a player's fog updated)? The undo should re-apply the prior state and propagate it like any other edit, not require a special-cased rollback.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST allow a GM to create, reposition, and delete wall segments directly on the scene canvas.
- **FR-002**: The system MUST allow a GM to configure, per wall segment, whether it blocks line-of-sight, movement, or both.
- **FR-003**: The system MUST recompute and propagate vision/fog-of-war effects to all connected clients within a few seconds of a wall being created, moved, reconfigured, or deleted.
- **FR-004**: The system MUST allow a GM to create, reposition, resize (radius/intensity), and delete light sources directly on the scene canvas.
- **FR-005**: The system MUST occlude light from a source by any wall segment configured to block vision, consistent with how player vision is occluded.
- **FR-006**: The system MUST support attaching a light source to a token so that the light's position follows the token's position.
- **FR-007**: The system MUST allow a GM to draw, reposition, and delete freeform annotations (strokes/shapes with optional text) directly on the scene canvas.
- **FR-008**: The system MUST allow a GM to mark an individual annotation as GM-only or visible-to-players.
- **FR-009**: The system MUST NOT present wall, lighting, or annotation authoring controls to non-GM (player) users; players only see the resulting rendered effects and any player-visible annotations.
- **FR-010**: The system MUST reject wall, lighting, and GM-only-annotation mutations from non-GM users at the server, independent of what the client displays.
- **FR-011**: The system MUST persist walls, light sources, and annotations per-scene so they are present on subsequent loads of that scene.
- **FR-012**: The system MUST support undo of the most recent GM authoring action (wall, light, or annotation create/move/delete) within the current editing session.
- **FR-013**: Each of the wall, lighting, and annotation authoring tools MUST be independently usable — a scene with only walls, only lights, or only annotations (in any combination, including none) MUST render and function correctly.
- **FR-014**: The system MUST continue to support existing token creation, selection, and movement on the canvas unchanged by the introduction of these authoring tools.
- **FR-015**: The system MUST NOT depend on the previously wrapped third-party whiteboard editor for any of walls, lighting, or annotations going forward.

### Key Entities

- **Wall**: A line segment on a scene with a start point, end point, and independent flags for whether it blocks vision and/or movement. Already has a persisted backend representation (Phase 6); this feature adds authoring and rendering/occlusion behavior on top of it.
- **Light Source**: A point on a scene with a radius, intensity/color, and an optional attachment to a token (so it can be static or mobile). Determines what area is illuminated, subject to occlusion by vision-blocking walls.
- **Annotation**: A freeform stroke, shape, or note placed on a scene, with a visibility flag (GM-only or visible-to-players) and ownership/authorship, replacing the whiteboard-document concept previously provided by the wrapped editor.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can draw a wall and see a player's fog-of-war view update to reflect it in under 5 seconds, without a page reload.
- **SC-002**: A GM can light a room and have an adjacent, wall-separated room remain dark, verified visually with zero manual fog painting required.
- **SC-003**: A GM can complete "draw a wall around a room and place one light inside it" in under 2 minutes on a scene they have not annotated before.
- **SC-004**: 100% of wall/lighting/annotation-authoring mutation attempts by a non-GM user are rejected, verified independent of client behavior.
- **SC-005**: A scene with zero walls, lights, or annotations continues to render and support token movement with no errors or degraded performance.
- **SC-006**: After this feature ships, zero remaining product surfaces depend on the previously wrapped third-party whiteboard editor.

## Assumptions

- "GM" refers to the existing scene/world-owner role already enforced elsewhere in the system (e.g., wall mutation ownership checks); no new role model is introduced by this feature.
- Real-time propagation ("a few seconds") follows the same synchronization pattern already established for tokens and walls, rather than requiring a new transport.
- There is no pre-existing production data authored in the wrapped whiteboard editor that requires migration; this feature is additive/replacement for the authoring workflow, not a data migration project.
- Undo scope is limited to the current GM's current editing session (not a persistent multi-user undo/redo history across reconnects).
- Light source falloff/rendering fidelity (e.g., soft edges vs. hard-edged radius) is a visual-quality detail left to implementation, not a functional requirement.
- Annotation content is limited to freeform strokes/shapes and optional text labels; rich text, images, or embedded documents are out of scope for this feature.
