# Feature Specification: Hand-Drawn Authoring & Per-Campaign Asset Storage

**Feature Branch**: `002-canvas-authoring-asset-storage`

**Created**: 2026-08-20

**Status**: Draft

**Input**: User description: "Close the remaining T067 gap and add asset storage infrastructure for the canvas authoring feature. Two related capabilities: (1) Hand-drawn wall and shape authoring via direct pointer interaction on the Bevy canvas, closing T067 from specs/001-bevy-canvas-authoring/tasks.md. (2) Image paste-to-canvas with automatic WebP transformation, backed by a new self-hosted RustFS object storage service, folder-separated per owning user then per campaign (world) then per scene, reusing existing world_members/world_invites ownership for access control, with server-side-only credentials and short-lived per-user scoped write access."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Draw walls by hand on the canvas (Priority: P1)

A GM is running a session and needs to block a corridor that wasn't part of an imported map, or fix a wall a `.dd2vtt` import got slightly wrong. Instead of only being able to get walls onto the canvas via map import, the GM clicks directly on the canvas to place a chain of wall points, closes or ends the chain, and the wall immediately exists as a line-of-sight/movement blocker — the same kind of wall the import pipeline already produces. The GM can also select an existing wall segment, toggle it into a door, or delete it, all with the mouse, without leaving the canvas.

**Why this priority**: This is the single largest remaining gap from the original canvas-authoring replacement of tldraw (spec 001, T067): walls exist as data and render correctly, but a GM cannot author one from scratch by hand today. Without this, "replace tldraw" is incomplete for any table that doesn't have a pre-made `.dd2vtt` file for every scene.

**Independent Test**: On a scene with no imported map, a GM opens the wall tool, clicks three points on the canvas, ends the chain, and confirms a 2-segment wall now blocks line of sight between two test tokens on either side of it — verifiable with zero dependency on shape drawing or asset storage.

**Acceptance Scenarios**:

1. **Given** a GM has the wall tool active on an empty scene, **When** they click three distinct points on the canvas and press the key/action that ends the chain, **Then** a new wall consisting of two connected segments is created and persisted, and appears identically after a page reload.
2. **Given** an existing hand-drawn or imported wall segment, **When** the GM selects it and chooses "toggle door", **Then** the segment becomes a door (passable when open, blocking when closed) and this state persists and is visible to players as a door, not a plain wall.
3. **Given** an existing wall segment, **When** the GM selects it and chooses delete, **Then** the segment is removed from the scene and no longer blocks line of sight for any connected player.
4. **Given** a GM is mid-chain (has placed at least one point but not closed it), **When** they press cancel/escape, **Then** no partial wall is persisted and the canvas returns to its prior state.

---

### User Story 2 - Draw shapes by hand on the canvas (Priority: P1)

A GM wants to sketch a freehand annotation (circling a trap, drawing an arrow toward an ambush direction, writing a short note like "locked") directly on the canvas during play, the way they previously did in the tldraw whiteboard. Using the shape tool, they pick a tool mode (freehand, rectangle, ellipse, line/arrow, text), draw it with the mouse directly on the canvas, and it appears immediately and persists with the scene.

**Why this priority**: This is the other half of T067 and the last piece of tldraw's tool set (per ADR-037) that has shape-plugin backend support but no direct hand-drawn end-to-end coverage. Equal priority to walls because both are required for "virtually playable" parity with the previous whiteboard.

**Independent Test**: On any scene, a GM selects the rectangle shape tool, drags a rectangle on the canvas, releases, and confirms the rectangle persists after switching away from and back to the scene — independently verifiable without walls or asset storage.

**Acceptance Scenarios**:

1. **Given** the shape tool is set to freehand, **When** the GM drags the mouse across the canvas and releases, **Then** a freehand stroke matching the drag path is created and persisted.
2. **Given** the shape tool is set to rectangle, ellipse, or line/arrow, **When** the GM drags from a start point to an end point and releases, **Then** a shape of that type sized to the drag is created and persisted.
3. **Given** the shape tool is set to text, **When** the GM clicks a point and types a short label, **Then** a text annotation is placed at that point and persisted.
4. **Given** an existing hand-drawn shape, **When** the GM selects it and deletes it, **Then** it is removed from the scene and no longer visible to any player viewing that scene.
5. **Given** a GM switches from Scene A (with hand-drawn shapes) to Scene B and back, **When** they return to Scene A, **Then** all of Scene A's hand-drawn shapes are still present and Scene B's shapes are not shown on Scene A.

---

### User Story 3 - Paste an image onto the scene as a persisted asset (Priority: P2)

A GM has a monster portrait, a handout, or a piece of reference art on their clipboard (copied from a browser or image editor) and wants it on the scene canvas without a multi-step file-upload dialog. They click on the canvas and paste (Ctrl/Cmd+V); the image appears as a placed asset on the scene, is automatically converted to an efficient web format, and is available to players viewing that scene.

**Why this priority**: High value for actual play (handouts, portraits) but depends on the new storage backend (User Story 4) existing first, and is not required to close the T067 hand-drawn-authoring gap — hence P2, after the two drawing stories.

**Independent Test**: With the asset storage backend available, a GM copies a PNG image, focuses the scene canvas, pastes, and confirms an image element appears on the canvas and is still present after a page reload — independently verifiable without exercising the RBAC/invite edge cases of User Story 4.

**Acceptance Scenarios**:

1. **Given** a GM has an image on their system clipboard and the scene canvas is focused, **When** they paste, **Then** the image is uploaded, transformed, and appears as a new placed image element on the current scene within a reasonable wait.
2. **Given** a pasted image was not already in an efficient web format, **When** the upload completes, **Then** the stored asset is in that efficient web format rather than the original format, without visible quality loss a player would notice at normal viewing distance.
3. **Given** a pasted image is too large (exceeds the configured maximum upload size), **When** the GM attempts to paste it, **Then** the GM sees a clear error and no partial/corrupt asset is persisted.
4. **Given** an image has been pasted onto Scene A, **When** a player who is a member of that world loads Scene A, **Then** they see the pasted image rendered on the canvas.

---

### User Story 4 - Assets are private to the owning campaign unless shared (Priority: P2)

A player or GM's uploaded/pasted assets (map backgrounds, pasted images) must never be readable or writable by another user's unrelated campaign. Only the world's owner and its accepted members can add assets to that world's scenes; a user who is not a member of a world cannot write assets into it, and the system never hands out broad, long-lived storage credentials that would let any authenticated user reach another campaign's files.

**Why this priority**: This is a security/correctness requirement rather than a new user-facing feature — it constrains how User Stories 3 (and the existing map-import path) are implemented, so it's tested alongside them rather than as a separate flow, but is called out on its own because it's independently verifiable and independently valuable (it's what makes the storage migration safe to ship at all).

**Independent Test**: Two separate users each own a separate world. User A attempts to author a request that would write an asset into User B's world without being a member of it; the request is rejected before any object is written, verifiable purely at the API/storage boundary without needing the canvas UI at all.

**Acceptance Scenarios**:

1. **Given** User A owns World 1 and User B owns World 2 with User A not a member of World 2, **When** User A's session attempts to write an asset under World 2's storage path, **Then** the write is rejected and no object is created.
2. **Given** User B invites User A to World 2 and User A accepts, **When** User A's session then attempts to write an asset under World 2's storage path, **Then** the write succeeds.
3. **Given** User B later removes User A from World 2, **When** User A's session attempts to write a new asset under World 2's storage path afterward, **Then** the write is rejected.
4. **Given** any successful asset write, **When** the credential used to perform it is inspected, **Then** it is a short-lived, per-request credential scoped only to that user's permitted campaign paths, not the storage service's permanent root/admin credential.

---

### Edge Cases

- What happens if a GM starts drawing a wall or shape, then the browser tab loses focus or the connection drops mid-draw? The in-progress, unsaved shape/wall is discarded; nothing partial is persisted (same as Acceptance Scenario 4 of User Story 1).
- What happens if two GMs (co-owner and invited member) draw walls on the same scene at the same time? Each completed wall/shape is persisted independently as soon as it's finished; last-write-wins is acceptable for overlapping edits to the same element, consistent with how token edits are already handled.
- What happens if a pasted clipboard item is not an image (e.g., pasted text or a file the browser reports without image data)? The paste is ignored for canvas purposes; no upload is attempted.
- What happens when a world member is removed while they have an in-progress paste upload? The in-flight request is authorized against membership state at the time the server receives it; a request that arrives after removal is rejected per User Story 4.
- What happens to a scene's existing background/pasted images if the underlying storage service is temporarily unavailable? Previously-rendered images already cached client-side continue to display; new pastes and new scene loads that need fresh asset fetches show a clear loading/error state rather than silently failing.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST let a GM place a new wall by clicking a sequence of points directly on the scene canvas and explicitly ending the chain, producing one wall with a segment between each consecutive pair of points.
- **FR-002**: The system MUST let a GM cancel an in-progress, not-yet-ended wall chain such that nothing is persisted.
- **FR-003**: The system MUST let a GM select an existing wall segment (hand-drawn or imported) and toggle it between a plain wall and a door.
- **FR-004**: The system MUST let a GM select an existing wall segment and delete it, immediately removing its line-of-sight/movement-blocking effect.
- **FR-005**: The system MUST let a GM draw a freehand shape by dragging on the canvas, producing a shape that follows the drag path.
- **FR-006**: The system MUST let a GM draw a rectangle, ellipse, or line/arrow shape by dragging from a start point to an end point on the canvas.
- **FR-007**: The system MUST let a GM place a text annotation by clicking a point on the canvas and entering text.
- **FR-008**: The system MUST let a GM select and delete an existing hand-drawn shape.
- **FR-009**: Hand-drawn walls and shapes MUST persist per-scene: switching away from and back to a scene MUST show exactly that scene's walls/shapes, matching the existing per-scene isolation already proven for map-imported content.
- **FR-010**: Only GMs (world owners/authorized members with GM standing) MUST be able to author (create, edit, delete) walls and shapes; players MUST be able to view their rendered effects (line-of-sight blocking, visible shape annotations) but MUST NOT get authoring controls.
- **FR-011**: The system MUST let a GM paste an image from the system clipboard directly onto the focused scene canvas to create a new placed image asset.
- **FR-012**: The system MUST automatically transcode every newly uploaded/pasted canvas image asset to a modern, efficient web image format before persisting it, regardless of the format the source image arrived in.
- **FR-013**: The system MUST reject pasted/uploaded images above a configured maximum size with a clear error and MUST NOT persist a partial asset in that case.
- **FR-014**: The system MUST store every campaign's assets (map backgrounds, pasted images) under a path that is unique to that campaign's owning user and world, and MUST NOT allow one campaign's assets to be listed or read at another campaign's path by default.
- **FR-015**: The system MUST authorize every asset write against the requesting user's current membership (owner or accepted invited member) of the target world at the time of the request, using the same ownership/invite model already governing other per-world mutations.
- **FR-016**: The system MUST reject an asset write attempt from a user who is not an owner or accepted member of the target world, before any object is created in storage.
- **FR-017**: The system MUST NOT expose the storage service's permanent root/administrative credential to any client; all client-facing asset uploads MUST use short-lived, per-request credentials scoped to only the paths that request's authorized user may write.
- **FR-018**: The existing map-import background-image storage path MUST be migrated to use the same asset storage backend and the same per-campaign path/authorization rules as newly pasted images, so there is exactly one asset storage mechanism, not two.
- **FR-019**: The system MUST make stored assets (backgrounds, pasted images) readable by any user who is an owner or accepted member of the owning world, viewing any scene that references them.
- **FR-020**: The local development environment MUST be able to start the new asset storage service and have it fully configured (service credentials, storage location bootstrap) via the project's existing local-dev provisioning flow, without manual out-of-band setup steps.

### Key Entities *(include if feature involves data)*

- **Wall segment**: A line-of-sight/movement-blocking (or door, when toggled) edge belonging to a scene, with a start point, end point, and open/closed door state. Already exists as a persisted entity from spec 001; this feature adds a hand-authoring path to create/edit/delete them directly, in addition to the existing map-import path.
- **Shape annotation**: A freehand stroke, rectangle, ellipse, line/arrow, or text label belonging to a scene. Already exists as a persisted entity from spec 001 (`ShapePlugin`); this feature adds the direct pointer-driven authoring path.
- **Canvas image asset**: A newly-introduced persisted record representing an image (map background or pasted image) placed on a scene: its storage location, owning world/scene, uploading user, and format. Distinct from wall/shape entities in that its content lives in the new object storage service rather than inline in the database.
- **World membership** *(existing, reused)*: The owner/accepted-invited-member relationship between a user and a world, already governing other mutations; this feature reuses it, unchanged, as the authorization source of truth for asset writes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can hand-draw a complete multi-segment wall on an empty scene and see it block line of sight for a test token within 10 seconds of starting the interaction.
- **SC-002**: A GM can hand-draw each of the five shape types (freehand, rectangle, ellipse, line/arrow, text) and have each persist correctly across a scene switch, with zero manual retries needed in normal operation.
- **SC-003**: 100% of hand-drawn walls and shapes created in one scene remain correctly isolated from every other scene across at least 3 consecutive scene switches, with no cross-scene bleed.
- **SC-004**: A GM can paste a typical clipboard image (under the configured size limit) and see it appear on the canvas within 10 seconds under normal network conditions.
- **SC-005**: 100% of pasted/uploaded canvas images are stored in the efficient web format, verified by inspecting the stored asset's format, regardless of the source format pasted.
- **SC-006**: In a two-user adversarial test, 100% of asset-write attempts by a user with no ownership or membership relationship to the target world are rejected before any object is created.
- **SC-007**: After a user is granted membership via invite, their next asset write to that world succeeds; after that membership is revoked, their next asset write to that world is rejected — both within one request, with no stale-permission window beyond the single in-flight request edge case already called out.
- **SC-008**: The local dev environment stands up the asset storage service and it is ready to accept authorized writes within the same single provisioning command already used to stand up the rest of the local stack, with no additional manual configuration step.

## Assumptions

- "Client" in the original request refers to the owning user account, not a new multi-tenant organization concept; storage paths are separated by user, then by world (campaign), then by scene — there is no new "organization" entity above users.
- GM standing for authoring purposes reuses the existing world-owner/member role model already governing other per-world mutations (e.g., wall/shape/token GraphQL mutations); this feature does not introduce a new permission tier.
- "Efficient web image format" means WebP, consistent with the format already used by some existing `.dd2vtt` map source images and already supported by the rendering engine.
- The maximum pasted-image upload size reuses the existing map-import upload size ceiling already enforced on the server, unless a smaller size proves necessary during implementation.
- Concurrent edits to the same wall/shape/asset by multiple authorized users use simple last-write-wins semantics, consistent with existing token-edit behavior; no new conflict-resolution/locking model is introduced.
- The new object storage service runs as part of the existing local development stack (alongside the database and other already-provisioned services) and is not, in this feature's scope, being specified for production/hosted deployment topology.
- Deleting a world or revoking a user's membership does not, in this feature's scope, retroactively delete or garbage-collect that user's previously-written assets; asset lifecycle/cleanup beyond access-control enforcement is out of scope.
