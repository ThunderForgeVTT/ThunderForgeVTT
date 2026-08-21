# Feature Specification: Universal VTT (.dd2vtt) Map Import Fidelity & Round-Trip Verification

**Feature Branch**: `003-dd2vtt-map-fidelity`

**Created**: 2026-08-21

**Status**: Draft

**Input**: User description: "Improve dd2vtt (Universal VTT) map loading robustness and test coverage: add a true round-trip test proving scene data (walls, doors, lights, background image, shapes, tokens) survives persist-and-reload with no data loss, close known parsed-but-unused UVTT field gaps (portal rotation/freestanding, ambient light, movable-object occluders) or explicitly document them as out of scope, and decide whether map export/save-out (getting a scene back out as a .dd2vtt or similar file) is in scope for this spec or explicitly deferred."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A GM trusts that an imported map is exactly what they see after any reload (Priority: P1)

A GM imports a `.dd2vtt` map into a scene: walls, doors, light sources, and the background image all appear correctly. Days later, the GM (or a player) reloads the page, the server restarts, or the GM switches away from the scene and back. Today, nothing actually verifies that the scene the GM sees after any of these events is identical to what was imported — only that parsing a fixture file "succeeds" and produces the expected *counts* of walls/lights. A GM needs confidence that their table's map is durable: what they imported is what stays there, indefinitely, not just immediately after import.

**Why this priority**: This is the foundational trust guarantee underneath every other map-related feature (including spec 002's canvas authoring, which now lets a GM hand-edit an imported map's walls and shapes). Without a real test proving persisted map data survives a reload unchanged, a regression in any adjacent change (schema migration, serialization tweak, storage migration) could silently corrupt or drop map data with nothing catching it before a real GM does.

**Independent Test**: Import a `.dd2vtt` fixture into a fresh scene, record every wall segment (with door state), light source, and the background image's identity, then reload the scene's data from the database exactly as a fresh page load would, and confirm every recorded value matches exactly — verifiable independent of any UI, export capability, or the field-handling work in User Story 2.

**Acceptance Scenarios**:

1. **Given** a `.dd2vtt` fixture with multiple walls (including at least one door) and multiple light sources, **When** it is imported into a scene and the scene's data is then reloaded from persisted storage, **Then** every wall's start point, end point, and door state, and every light source's position and properties, are identical to what was imported — no additions, omissions, or value changes.
2. **Given** a `.dd2vtt` fixture with a background image, **When** the scene is reloaded after import, **Then** the background image reference resolves to the same visual image that was imported (not a placeholder, a different file, or a broken reference).
3. **Given** an imported map's walls and shapes are then edited by a GM through hand-drawn authoring (spec 002) — a wall added, a shape deleted — **When** the scene is reloaded afterward, **Then** the reloaded state reflects the edits exactly (the edited state is itself durable, not just the originally-imported state).
4. **Given** the round-trip verification from Scenario 1, **When** it is run repeatedly (e.g., in CI on every change), **Then** it reliably passes or fails based on actual data fidelity, with no flakiness tied to timing, ordering, or environment.

---

### User Story 2 - A GM is never silently missing part of their map (Priority: P2)

Some fields defined in the Universal VTT format are read by the importer but never actually used anywhere after that: a portal's rotation and whether it's "freestanding" (not attached to a wall segment), a map's ambient lighting hint, and object-based line-of-sight occluders (blocking shapes that aren't walls). Today a GM importing a map that relies on any of these gets no indication that part of their map didn't come through — the import reports success either way. A GM needs to either see that data actually work correctly, or be told plainly that it was skipped, so they can manually recreate it instead of discovering the gap mid-session.

**Why this priority**: Lower priority than User Story 1 because it's about import completeness/transparency for a known-narrow set of fields, not the core durability guarantee every existing map already depends on. Still valuable because silent partial import is a trust-eroding failure mode — a GM who finds out mid-session that a trap-blocking occluder was never imported has already lost time and immersion.

**Independent Test**: Import a `.dd2vtt` fixture that exercises each of the three currently-unused field categories (a freestanding portal, an ambient-light hint, an object-based occluder) and confirm the import result clearly communicates, per field category, whether it was applied or skipped — independently verifiable without User Story 1's persistence-round-trip machinery.

**Acceptance Scenarios**:

1. **Given** a `.dd2vtt` file containing a freestanding portal (not attached to a wall's line-of-sight polygon), **When** it is imported, **Then** the system either creates a correct, usable door/wall from it, or the import result explicitly reports that this portal was not imported and why.
2. **Given** a `.dd2vtt` file containing an `environment.ambient_light` value, **When** it is imported, **Then** the system either applies it to the scene's lighting, or the import result explicitly reports that ambient light was not imported.
3. **Given** a `.dd2vtt` file containing `objects_line_of_sight` occluder shapes, **When** it is imported, **Then** the system either creates corresponding vision-blocking geometry, or the import result explicitly reports that these occluders were not imported.
4. **Given** a `.dd2vtt` file that uses none of these three field categories (the common case today), **When** it is imported, **Then** the import result shows no skipped-field notices at all — the reporting only appears when relevant, never as noise.

---

### Edge Cases

- What happens when a `.dd2vtt` file's format version is not `0.3`? Already handled today (explicitly rejected before any data is written) — this spec's round-trip verification (User Story 1) must not regress that rejection.
- What happens if a scene is reloaded while an import is still in progress? Out of scope for this spec — import is already a single synchronous request; this spec only verifies data already committed.
- What happens if the underlying background-image storage (RustFS, per spec 002) is temporarily unreachable during a round-trip verification run? The verification should surface this as a clear infrastructure failure, distinct from a genuine data-fidelity failure, so a flaky environment doesn't get mistaken for a real regression (or vice versa).
- What happens to a scene's existing map data if a `.dd2vtt` re-import is performed on a scene that already has walls/lights from a prior import? Out of scope for this spec (re-import/merge semantics are a separate concern from verifying that a single import's data survives reloads) — assumed to follow whatever the existing import endpoint already does today, unchanged.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide an automated, repeatable verification that every wall (including door state), light source, shape, and token belonging to a scene survives being persisted and reloaded with all field values identical to what was written — not merely that the right *number* of each entity exists.
- **FR-002**: The round-trip verification in FR-001 MUST cover data that originated from a `.dd2vtt` import specifically (not only data created through other authoring paths), since import is the primary way map data enters the system today.
- **FR-003**: The round-trip verification in FR-001 MUST include the scene's background image reference, confirming it resolves to the same image content that was imported.
- **FR-004**: The round-trip verification in FR-001 MUST also cover a scene whose imported data has since been modified through hand-drawn authoring (spec 002's wall/shape editing), confirming edited state — not just originally-imported state — survives a reload.
- **FR-005**: The system MUST NOT report a `.dd2vtt` import as fully successful if any recognized-but-unhandled field category (freestanding portals, ambient light, object-based occluders) was present in the source file and not applied to the resulting scene — the import result MUST distinguish "fully imported" from "imported with some fields skipped."
- **FR-006**: When the import result reports skipped fields (FR-005), it MUST identify which field category was skipped, without requiring the GM to inspect server logs or source code to find out.
- **FR-007**: A `.dd2vtt` file that does not use any of the currently-unhandled field categories MUST continue to import and report success exactly as it does today, with no new skipped-field noise introduced by this feature.
- **FR-008**: The existing rejection of unsupported UVTT format versions (only `0.3` is accepted) MUST continue to function unchanged after this feature's changes.
- **FR-009**: Map export (producing a `.dd2vtt` or other file from a scene's current state) is explicitly OUT OF SCOPE for this feature — see Assumptions.

### Key Entities *(include if feature involves data)*

- **Wall segment / door**: Already-persisted entity (spec 001/002); this feature verifies its round-trip durability, does not change its shape.
- **Light source**: Already-persisted entity; same verification-only treatment.
- **Shape annotation, token**: Already-persisted entities; included in the round-trip verification per FR-001 for completeness, though they are not themselves produced by `.dd2vtt` import.
- **Canvas image asset (background image)**: Already-persisted entity (spec 002); this feature verifies the scene's reference to it survives a reload and still resolves correctly.
- **Import result**: Extended (not newly introduced) to be able to carry a per-field-category "skipped" notice, per FR-005/FR-006 — a GM-visible signal, not just an internal log line.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of a `.dd2vtt` fixture's walls (with door state), lights, and background image reference match exactly after a persist-and-reload cycle, verified by an automated check that runs on every relevant code change, not just manually.
- **SC-002**: A GM importing a map that uses none of the three currently-unhandled field categories sees identical import-success behavior before and after this feature ships (zero regression, zero new noise).
- **SC-003**: A GM importing a map that does use one of the three currently-unhandled field categories can determine, from the import result alone (no server access), which part of their map was not imported.
- **SC-004**: The round-trip verification from SC-001 fails reliably and specifically when a real data-fidelity bug is deliberately introduced (validated at least once by intentionally breaking a field and confirming the check catches it), so the check is known to have teeth, not just be present.

## Assumptions

- **Map export/save-out is deferred, not in scope.** The user request asked this to be explicitly decided rather than left ambiguous: producing a `.dd2vtt` (or other) file from a scene's current state is a materially larger feature (a full serialization-out path, format fidelity in the reverse direction, a new UI affordance) than "verify existing import data is durable and complete," which is this spec's actual focus. The round-trip verification in User Story 1 proves durability via internal persist-and-reload, not via a file-based export/re-import cycle — it does not require an export capability to exist.
- **Closing the three known field gaps (freestanding portals, ambient light, object occluders) is deferred to "detect and disclose," not "fully implement."** Building correct handling for all three (e.g., real vision-blocking geometry for object occluders, ambient-light rendering) is separate follow-on work with its own design questions; this spec's obligation is that a GM is never left unaware that a field was skipped, per FR-005/FR-006. A future spec may pick up implementing any of them fully.
- Existing `.dd2vtt`/UVTT format support (walls, portals attached to walls treated as doors, lights, background image, format version `0.3` only) is unchanged by this feature except for the field-category disclosure in User Story 2.
- The round-trip verification is a testing/CI concern, not a new user-facing feature — no new GraphQL mutation, REST endpoint, or UI screen is implied by User Story 1 beyond the "skipped fields" signal in User Story 2's import result.
- This feature builds on spec 001 (native canvas authoring, wall/light/shape data model) and spec 002 (canvas image asset storage, `.dd2vtt` background-image migration to RustFS) — both already merged.
