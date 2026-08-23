# Implementation Plan: System Pack Legal & Attribution Compliance

**Branch**: `016-system-pack-legal-compliance` | **Date**: 2026-08-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/016-system-pack-legal-compliance/spec.md`

## Summary

Formalize the `legal` object already prototyped in the six `research/system_*.json` digests into a required field of the `system.json` manifest contract, document it in ADR 027, validate its presence when a pack loads, and render it to the GM through the existing `SystemManifest`/`GameSystemContext` plumbing at two points: the system-selection step of world creation/reconfiguration, and a persistent per-world system-settings view. This is metadata-and-rendering work layered on top of the existing manifest-loading path (`GameSystemContext`, `useGameSystemManifest`, `SystemHooksProvider`) — no new content-loading architecture is introduced, and no game-mechanics behavior changes.

## Technical Context

**Language/Version**: TypeScript/React (`apps/web`) for manifest typing and the two new UI surfaces; Rust (server-side pack loader/validator, `src/server/src/systems.rs` / `system_hooks.rs`) for the load-time `legal` presence check

**Primary Dependencies**: Existing `SystemManifest` type (`apps/web/src/contexts/GameSystemContext.tsx`), existing manifest loading path (`useGameSystemManifest`, `SystemHooksProvider`), existing fantasy design-system UI primitives (`apps/web/src/components/ui/`) for the new banner/settings surfaces — no new runtime dependency

**Storage**: No new database table — `legal` lives inside each system pack's static `system.json`/manifest file, loaded the same way `skills`/`abilities`/`data_types` already are. No per-world persistence needed beyond the existing `gameSystemId` field already on the World entity.

**Testing**: `tsc`/existing web test conventions for manifest typing and the two new components; a small Rust unit test in the pack loader confirming a manifest missing `legal` fails validation, mirroring existing validator tests (`packs/systems/dnd5e/server/src/validators.test.rs`)

**Target Platform**: Web (React) + native server (pack loader/validator) — no engine/wasm surface

**Project Type**: Web application (existing `apps/web` + `src/server` + `packs/systems/*` split)

**Performance Goals**: Not performance-sensitive — `legal` is static per-pack metadata loaded once with the rest of the manifest, not a per-request or per-entity concern

**Constraints**: Must not require reshaping data already produced in `research/system_*.json` (FR-008) — the manifest contract's `legal` shape is derived from, not invented independently of, those digests. Must not weaken or duplicate the trademark-naming fix already applied to `dnd5e` (title strings) — this feature only adds the `legal` object, it does not touch `title`/`id` fields again.

**Scale/Scope**: One manifest contract field (`legal`) + one ADR update + one load-time validator check + two UI surfaces (selection-time notice, persistent settings view) + one populated example (`dnd5e`'s `legal` object, derived from `research/system_dnd5e.json`). Building out the five remaining system packs end-to-end is explicitly out of scope (spec Assumptions) — only their manifest contract compliance, when built, is in scope.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: N/A — no canvas/engine involvement; this is manifest metadata and React chrome.
- **Principle II (Plugin-modular engine)**: N/A — no engine crate changes.
- **Principle III (Ownership & authorization at the data boundary)**: N/A for this feature specifically — `legal` is public, non-sensitive, read-only metadata with no ownership dimension; no new mutation path is introduced.
- **Principle IV (Real ADRs and specs before divergent implementation)**: This spec exists; per FR-002 the ADR update (extending `docs/adrs/20260504-027-game_system_packaging_and_manifest_contract.md`) is itself one of this feature's required outputs, not an optional afterthought — tracked directly in Requirements, not deferred.
- **Principle V (Verify before claiming done)**: Web changes verified via `tsc`/build and manual exercise of both UI surfaces in a running dev instance (per repo convention for UI-affecting changes); server validator change verified via native `cargo check`/`cargo test`.

**Initial gate result**: PASS — no violations requiring Complexity Tracking.

**Post-design re-check** (after Phase 1 data-model/contracts): Still PASS. The `legal` shape (data-model.md) is additive to the existing manifest and requires no ownership/authorization changes; the ADR amendment (contracts/manifest-legal-schema.md references it) satisfies Principle IV directly rather than deferring it.

## Project Structure

### Documentation (this feature)

```text
specs/016-system-pack-legal-compliance/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/             # Phase 1 output
│   └── manifest-legal-schema.md
└── tasks.md              # Phase 2 output (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
packs/systems/
├── dnd5e/
│   └── system.json                        # gains a populated `legal` object (derived from research/system_dnd5e.json)
└── basic-game-system/
    └── system.json                        # out of scope per spec Assumptions (currently empty; untouched here)

src/server/src/
├── systems.rs                              # pack manifest loading
└── system_hooks.rs / validators.rs         # gains: manifest missing `legal` → load rejected/flagged (FR-007)

apps/web/src/
├── contexts/GameSystemContext.tsx          # SystemManifest type gains a typed `legal` field
├── components/game-systems/legal/
│   ├── SystemLegalNotice.tsx               # renders one pack's `legal` object; used at both surfaces below
│   └── systemLegalPlacement.ts             # small helper reading `legal.requiredUiPlacement` to decide selection-time vs settings-only display
├── pages/world/ (or wherever system selection currently lives)
│   └── <system selection step>             # gains SystemLegalNotice before/at confirmation (FR-004)
└── pages/world/settings/
    └── SystemSettingsPanel.tsx              # new persistent per-world location; gains SystemLegalNotice (FR-005)

docs/adrs/
└── 20260504-027-game_system_packaging_and_manifest_contract.md   # amended, not replaced (FR-002)
```

**Structure Decision**: No new project or service boundary. `legal` is added as a sibling field to the manifest's existing `skills`/`abilities`/`data_types` blocks, following the same load/validate/render path those already use, per Assumptions (formalize the existing digest shape, don't invent a new pipeline).

## Complexity Tracking

*No Constitution Check violations — table intentionally left empty.*
