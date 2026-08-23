# Phase 0 Research: System Pack Legal & Attribution Compliance

## R1: What shape should the manifest's `legal` object take?

**Decision**: Adopt the shape already validated across all six `research/system_*.json` digests (built and reviewed in the preceding compliance work), rather than designing a new one: `licenseName`, `attributionText`, an optional `requiredNotice` (for license-specific badges/notices like the ORC Notice or the Cypher System compatibility phrase), `disclaimer`, `trademarkRestrictions: string[]`, and `requiredUiPlacement`. Field names are camelCased for the manifest (the digests used snake_case as research artifacts; the manifest contract follows the existing `system.json` convention, e.g. `esmodules`, `spellSlots`).

**Rationale**: Per FR-008, populating a pack's `legal` object from its digest must require no reshaping. Six real, license-distinct examples (CC-BY straightforward attribution for 5E/Fate/Blades, the Cypher System's mandatory-badge requirement, Pathfinder 2e's ORC Notice, Year Zero Engine's video-game/NFT carve-out) already exercised this shape successfully — inventing a different shape now would just require re-deriving what's already been derived once.

**Alternatives considered**: A single free-text `legal` string (minimal change) — rejected, this is exactly the status quo gap the compliance review found (the existing `license` string already does this and was insufficient); a per-license-type discriminated union (`type: "cc-by" | "orc" | "csol" | ...` with type-specific fields) — rejected as over-engineering for six known systems with substantial field overlap; a flat optional-everything object satisfies all six without a variant type, and a new license type can add fields without a breaking schema change.

## R2: How does `legal` get typed and threaded through the existing manifest-loading path?

**Decision**: Extend the existing `SystemManifest` TypeScript type (`apps/web/src/contexts/GameSystemContext.tsx`, currently `{ id, title, version, [key: string]: any }`) with a typed `legal` field, and extend the Rust-side manifest struct in `src/server/src/systems.rs` correspondingly. No change to the loading mechanism itself (`useGameSystemManifest`, `SystemHooksProvider`) — `legal` rides along with the rest of the manifest exactly as `skills`/`abilities`/`data_types` already do.

**Rationale**: The manifest is already lazy-loaded and cached per system (per `GameSystemContext`'s own doc comment); `legal` is static data with the same lifecycle as everything else in the manifest, so it needs no new loading path, just a typed field and two render call-sites.

**Alternatives considered**: A separate `GET /systems/:id/legal` endpoint fetched independently — rejected, adds a redundant round-trip and cache-invalidation surface for data that's already delivered with the manifest.

## R3: Where exactly does "system-selection screen" live, and how does a stricter placement rule (Cypher System) get honored?

**Decision**: `SystemLegalNotice` is one component, rendered from two call sites: (a) wherever a GM currently picks/confirms `gameSystemId` for a world (world creation flow / world settings), and (b) a new persistent `SystemSettingsPanel` reachable from world settings at any time. A small helper (`systemLegalPlacement.ts`) reads `legal.requiredUiPlacement` and, when it names the selection screen specifically (as Cypher System's license does), asserts the notice is shown at call site (a) and not only (b) — implemented as a lint-level dev-time assertion/test rather than new runtime branching, since both call sites already render the same component.

**Rationale**: FR-006 requires honoring stricter per-license placement "in addition to" the default settings display, not instead of it — so both call sites always render `SystemLegalNotice`; the only thing `requiredUiPlacement` needs to drive is a test/verification signal (SC-004: "independently verifiable as satisfied"), not conditional UI logic.

**Alternatives considered**: Only showing the notice where the specific license requires it (skip settings-view display for CC-BY systems that don't strictly require it) — rejected; FR-005 requires the settings-view location for every system regardless of its specific license, since "reachable afterward from a persistent location" is itself part of the compliance review's finding #2, independent of any one license's stricter rule.

## R4: How does pack loading reject/flag a manifest missing `legal`?

**Decision**: Extend the existing server-side manifest validator (same location as current `data_types`/`skills` structural checks, per `packs/systems/dnd5e/server/src/validators.rs`) with a check that `legal` is present and its required sub-fields (`licenseName`, `attributionText`) are non-empty. Follow the same validator failure convention already established there (structured validation error, not a panic) so pack loading fails closed and visibly rather than shipping a pack with silently-absent attribution.

**Rationale**: FR-007 requires this to be caught by the loader/validator specifically, matching Constitution Principle III's server-side-enforcement pattern and the project's existing validator test conventions (`validators.test.rs`).

**Alternatives considered**: A CI lint step over `packs/systems/*/system.json` instead of a runtime loader check — worth having in addition (cheap to add), but not a substitute: a runtime check is what actually prevents a non-compliant pack from being loaded and served if a lint step is ever bypassed or a pack is added outside the normal PR flow (e.g. a future third-party pack, per spec User Story 2's stated intent to hold up under that case).

## R5: Does ADR 027 need a full rewrite or an amendment?

**Decision**: Amend `docs/adrs/20260504-027-game_system_packaging_and_manifest_contract.md` in place with a new section documenting the `legal` field's shape and rationale (referencing this spec and `research/vtt_open_license_game_systems.md`), following the same Sync-Impact-Report convention used elsewhere in this repo's governance docs (e.g. the constitution's own header) rather than superseding the ADR with a new one.

**Rationale**: The existing ADR already governs the manifest contract wholesale; `legal` is an addition to that contract, not a replacement of the decision it recorded. A new ADR would fragment "what does system.json require" across two documents.

**Alternatives considered**: A brand-new ADR solely for the `legal` field — rejected as unnecessary fragmentation for what is an additive field on an existing, actively-governed contract.
