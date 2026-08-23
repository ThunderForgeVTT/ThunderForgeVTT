# Feature Specification: System Pack Legal & Attribution Compliance

**Feature Branch**: `016-system-pack-legal-compliance`

**Created**: 2026-08-23

**Status**: Draft

**Input**: User description: "Fix the remaining findings from the open-license compliance review (research/vtt_open_license_game_systems.md): (2) no UI attribution mechanism exists anywhere in the app — the license requires a splash screen or permanent settings tab showing TASL attribution / ORC Notice / 'Compatible with the Cypher System' badge, not buried in ToS; and (3) the system pack manifest contract (packs/systems/*/system.json, governed by ADR 20260504-027-game_system_packaging_and_manifest_contract.md) has no structured place for attribution text, ORC-notice/compatibility-badge fields, trademark restrictions, or a disclaimer — only a loose free-text license string, and the ADR doesn't mention licensing/attribution/trademarks at all. A `legal` object modeled on what's already been built into the research digests at research/system_*.json should be added to the manifest contract, and every shipped system pack must supply one, and it must be rendered to GMs when they select or configure a system for their world."

## Clarifications

*(No clarification session held — the request references an existing, already-designed reference shape (the `legal` object in `research/system_*.json`) and an existing governing document (ADR 027) to extend, leaving no critical ambiguity that couldn't be resolved with the reasonable defaults recorded in Assumptions below.)*

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A GM sees required legal/attribution notices when choosing a system for their world (Priority: P1)

When a GM creates a new world (or changes an existing world's game system), they pick from the available system packs (5E System Core, Pathfinder 2e, Cypher System, Fate Core, Blades in the Dark, Year Zero Engine, etc.). Before or as they confirm that choice, they see the specific legal notice that system's license requires — e.g. the Pathfinder 2e pack shows its ORC Notice referencing the Library of Congress registration, the Cypher System pack shows the "Compatible with the Cypher System" notice and MCG non-affiliation disclaimer, the 5E pack shows its TASL (Title/Author/Source/License) attribution. This notice is also reachable afterward from a permanent, easily discoverable place (a system/module settings view) — not buried in a Terms of Service page nobody reads.

**Why this priority**: This is the actual legal obligation named in the compliance review — every open license reviewed (CC-BY, ORC, Cypher System Open License) conditions continued legal use of the content on this attribution being visibly displayed, not just recorded internally. Without this, the platform is out of compliance the moment any of these system packs ships to real users, regardless of how correct the underlying data is.

**Independent Test**: Select each of the six system packs when creating or reconfiguring a world; confirm each shows its own distinct, correct legal notice at selection time, and confirm the same notice remains reachable afterward from a persistent settings location for that world.

**Acceptance Scenarios**:

1. **Given** a GM is choosing a system for a new world, **When** they select a specific system pack, **Then** that pack's required legal notice (attribution text, and any license-specific badge/notice such as the ORC Notice or Cypher compatibility phrase) is displayed before or as part of confirming the selection.
2. **Given** a world already has a system assigned, **When** the GM opens that world's system/module settings, **Then** the same legal notice for the active system is available there at any time, not only at creation time.
3. **Given** two different worlds using two different system packs, **When** their respective legal notices are displayed, **Then** each shows the correct notice for its own system — no cross-contamination between systems' attribution text.
4. **Given** a system pack whose license has a specific required UI placement rule (e.g. Cypher System's requirement that the compatibility notice appear on the system-selection screen itself, not just a buried settings tab), **When** that pack is shown in the system-selection flow, **Then** the placement rule is honored, not just satisfied elsewhere in the app.

---

### User Story 2 - A system pack author (internal or future third-party) fills in one standard `legal` field and the app handles the rest (Priority: P1)

Someone authoring or maintaining a system pack's `system.json` manifest — today that's the ThunderForgeVTT team, but the manifest contract should hold up if community/third-party system packs are supported later — needs one clear, documented place in the manifest to declare their license's required attribution text, any license-specific notice/badge, trademark restrictions, a non-affiliation disclaimer, and where the license requires it to be shown. They should not have to invent a shape per system or guess what "compliant" looks like from a legal essay.

**Why this priority**: Without a structured contract field, every pack either omits this information (today's state — a gap the review already found) or invents an ad hoc shape, making it impossible for the app to render attribution consistently across systems. This is a hard prerequisite for User Story 1.

**Independent Test**: Given the manifest contract's documentation/schema alone (no other context), author a `legal` block for a new hypothetical system pack and confirm it validates and renders correctly without needing to read source code.

**Acceptance Scenarios**:

1. **Given** the manifest contract documentation, **When** a pack author fills in the `legal` object for their system, **Then** they can express: license name, attribution text, an optional license-specific required notice/badge (e.g. ORC Notice, Cypher compatibility phrase), a disclaimer of non-affiliation, trademark restrictions, and a note on required UI placement — without needing any system-specific custom fields.
2. **Given** a `system.json` that omits the `legal` object entirely, **When** the pack is loaded, **Then** the system either fails validation with a clear error identifying the missing legal metadata, or (for packs still in development) is visibly flagged as non-compliant/incomplete rather than silently shipping without attribution.
3. **Given** the six existing research digests at `research/system_*.json` (each already containing a `legal` object) and the six system packs intended to be built from them, **When** each pack's manifest is authored, **Then** its `legal` object can be populated directly from the corresponding digest's `legal` object without reshaping the data.

---

### User Story 3 - The governing packaging contract documents the legal-metadata requirement (Priority: P2)

A developer reading ADR `20260504-027-game_system_packaging_and_manifest_contract.md` to understand what a valid `system.json` must contain should find the `legal` object's shape and its "why" documented there, alongside the other manifest fields it already governs — so the requirement isn't only enforced by validation code (or worse, only known informally) but is part of the recorded architectural decision.

**Why this priority**: Lower priority than Stories 1-2 because it's documentation, not runtime behavior — but without it, the requirement will quietly bit-rot the next time someone edits the manifest contract, since nothing in the ADR currently signals that licensing/attribution is even a concern for this contract.

**Independent Test**: A developer unfamiliar with this compliance work reads ADR 027 alone and can correctly describe what legal metadata a new system pack must supply and why, without consulting `research/vtt_open_license_game_systems.md` or this spec.

**Acceptance Scenarios**:

1. **Given** ADR 027, **When** a developer reads it, **Then** it describes the `legal` object's required/optional fields and states the compliance rationale (visible attribution is a condition of the CC-BY / ORC / Cypher System Open License / Free League FTL licenses referenced by the shipped packs).
2. **Given** the existing `dnd5e` and `basic-game-system` packs, **When** this work is complete, **Then** `dnd5e`'s manifest has been updated to include a compliant `legal` object (derived from `research/system_dnd5e.json`), and `basic-game-system`'s empty manifest is either populated with a minimal valid manifest (including a `legal` object appropriate for original, non-SRD-derived content) or explicitly tracked as a separate known gap outside this spec's scope.

### Edge Cases

- What happens when a system pack has no external license at all (e.g. a fully original, in-house system with no SRD derivation)? (The `legal` object should still be present but can state "original content, no third-party license" rather than being omitted — omission is what causes silent non-compliance.)
- What happens when a license's required notice text changes upstream (e.g. Paizo revises the ORC Notice wording)? (Out of scope for this spec's runtime behavior, but the manifest contract should make it obvious which field to update and that pack maintainers are responsible for keeping it current — noted as an assumption below.)
- What happens if a GM switches a world from one system to another mid-campaign? (The new system's legal notice must display at the point of switching, same as initial selection — not just at world creation.)
- How does the UI handle a system whose license has no special required-placement rule (most CC-BY systems), versus one that does (Cypher System, ORC)? (The default settings-tab placement satisfies the former; the latter's stricter placement requirement must still be honored on top of, not instead of, the default.)
- What happens to the `dnd5e` pack's already-completed trademark-naming fix (title changed from "Dungeons & Dragons 5th Edition" to "5E System Core", done as part of this same compliance effort) — does this spec re-touch it? (No — that fix is already applied across the 7 affected files; this spec only adds the missing `legal` object to the manifest, it does not redo the naming fix.)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `system.json` manifest contract MUST define a structured `legal` object as a required top-level field, distinct from the existing free-text `license` string, capable of expressing: license name, full attribution text, an optional license-specific required notice/badge, a non-affiliation disclaimer, a list of trademark restrictions, and guidance on required UI placement.
- **FR-002**: ADR `20260504-027-game_system_packaging_and_manifest_contract.md` MUST be updated to document the `legal` object's shape and the compliance rationale for its existence.
- **FR-003**: Every system pack shipped by the platform (starting with `dnd5e`, and each of the five packs to be built from the other `research/system_*.json` digests: Pathfinder 2e, Cypher System, Fate Core, Blades in the Dark, Year Zero Engine) MUST supply a complete, non-empty `legal` object in its manifest.
- **FR-004**: The application MUST render a system pack's `legal` object to the GM at the point of selecting that system for a world (world creation or system change), before or as part of confirming the selection.
- **FR-005**: The application MUST make a world's active system's `legal` object reachable at any later time from a persistent, easily discoverable location (a system/module settings view for that world) — not only at selection time, and not buried inside a general Terms of Service document.
- **FR-006**: When a system pack's `legal` object specifies a stricter required UI placement (e.g. a notice that must appear on the system-selection screen itself, not only in settings), the application MUST honor that placement in addition to the default settings-view display.
- **FR-007**: The system pack loader/validator MUST reject or clearly flag (per the project's existing validation conventions) a manifest that is missing a `legal` object, so non-compliant packs cannot silently ship.
- **FR-008**: Populating a pack's `legal` object FROM its corresponding `research/system_*.json` digest MUST require no reshaping of the data — the manifest contract's `legal` shape MUST be compatible with the shape already established in the digests.

### Key Entities

- **System Pack Legal Metadata (`legal` object)**: The structured, per-system-pack record of license name, attribution text, license-specific required notice/badge, disclaimer, trademark restrictions, and required UI placement — lives inside `system.json` alongside the existing loose `license` string.
- **System Pack Manifest (`system.json`)**: The existing per-system contract (id, title, version, license, skills, abilities, data_types, etc.) that this spec extends with the `legal` object as a new required field.
- **World System Settings View**: The persistent, per-world UI location where a world's active system's `legal` object is displayed on demand, outside of the initial selection flow.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of shipped system packs (all six systems covered by the research digests, once built) have a non-empty, schema-valid `legal` object in their manifest.
- **SC-002**: A GM can view the correct legal/attribution notice for any system within two interactions from either the system-selection flow or the world's settings view.
- **SC-003**: Zero system packs can be loaded by the application without a `legal` object present — validation catches the omission before the pack reaches a GM.
- **SC-004**: For every system pack whose license imposes a specific required placement (Pathfinder 2e's ORC Notice, Cypher System's compatibility badge), that placement rule is independently verifiable as satisfied, not merely inferred from the general settings-view display.

## Assumptions

- The `legal` object's field shape should closely mirror what was already designed and validated across the six `research/system_*.json` digests (each contains a working `legal` object per system) rather than being redesigned from scratch — this spec formalizes and contracts that existing shape rather than inventing a new one.
- Building the five remaining system packs (Pathfinder 2e, Cypher System, Fate Core, Blades in the Dark, Year Zero Engine) from their research digests into full `packs/systems/*/` implementations (the way `dnd5e` already exists) is a separate, larger effort outside this spec's scope; this spec only requires that whenever those packs are built, they carry a compliant `legal` object — it does not itself schedule that build-out.
- `packs/systems/basic-game-system/system.json` is currently an empty file (a pre-existing, unrelated gap found during the compliance review); this spec treats making it schema-valid (including a `legal` object once it has content) as in scope only if/when it's populated, and does not require populating it now.
- Keeping a license's required notice text current if it changes upstream (e.g. a publisher revises their ORC Notice wording) is an ongoing content-maintenance responsibility for pack maintainers, not a runtime feature this spec needs to build (e.g. no live-fetch-from-publisher mechanism is implied).
- "Rendering" the legal notice means displaying the human-readable text/notice content itself; formatting/visual design of that display is an implementation decision for the planning phase, not constrained here beyond "not buried in ToS" and "honors any license-specific placement rule."
