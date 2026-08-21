<!--
Sync Impact Report
- Version change: (none) → 1.0.0
- Modified principles: n/a (initial ratification)
- Added sections: Core Principles (I-V), Technology & Architecture Constraints,
  Development Workflow, Governance
- Removed sections: none
- Deferred TODOs: RATIFICATION_DATE set to the date this constitution was first
  authored (2026-08-20) since no earlier project-wide governance doc existed;
  amend if an earlier date is discovered.
-->

# ThunderForgeVTT Constitution

## Core Principles

### I. ECS Owns Simulation, React Owns Chrome
The Bevy (WASM) engine is the single source of truth for canvas simulation
state: scene geometry, tokens, walls, lighting, fog, selection, and any other
entity that must be drawn, dragged, or spatially queried on the game canvas.
React components MAY observe engine/world-store state for presentation
(panels, minimaps, toolbars) but MUST NOT become a second source of truth for
canvas state, and MUST NOT re-implement simulation or adjudication logic in
presentation components. All canvas authoring tools (drawing, annotation,
walls, lighting) are built as Bevy plugins/systems, not as wrapped
third-party editors — this is a hard boundary, not a preference.

**Rationale**: Prior architecture wrapped a third-party canvas library
(tldraw) for annotation while Bevy owned tokens/grid/fog. That split forced
two competing stores and a sync bridge. Consolidating all canvas ownership in
the ECS layer removes that class of bug permanently and keeps performance
(1000+ tokens, real-time sync) inside the engine that was built for it.

### II. Plugin-Modular Engine Architecture
Every new engine capability (drawing tools, walls, lighting, fog, selection,
etc.) MUST ship as a self-contained Bevy `Plugin` with its own module under
`src/engine/src/plugins/`, its own `systems/*` and `resources/*` where state
is non-trivial, and a narrow public surface re-exported through
`systems/mod.rs` / `resources/mod.rs`. Plugins MUST be independently
addable/removable from the `App` builder in `lib.rs` without editing each
other's internals. Cross-plugin communication happens through Bevy events or
shared resources, never through direct calls into another plugin's private
systems.

**Rationale**: The engine has already suffered from tightly-coupled systems
(e.g. token sync wired directly into `TokenPlugin`, requiring a full rewrite
to extract selection). Enforcing plugin boundaries up front keeps each
canvas tool (walls, lighting, drawing) independently testable and
replaceable.

### III. Ownership & Authorization at the Data Boundary
Every mutation that creates, updates, or deletes persisted, per-scene, or
per-world data MUST enforce ownership/authorization server-side at the
GraphQL/database boundary (see ADR-009, ADR-013, ADR-023, ADR-028). Client
and engine code MAY optimistically apply changes locally but MUST treat the
server as authoritative. New tables MUST carry `created_by`/`updated_by`
provenance consistent with existing ownership-field conventions.

**Rationale**: This is already established and enforced across the
codebase (worlds, game systems, invites, walls). New features must not
regress it.

### IV. Real ADRs and Specs Before Divergent Implementation
Architecturally significant decisions (new subsystem, replacing an
established dependency, changing an ownership boundary) MUST be captured as
an ADR under `docs/adrs/` and, for net-new features, a Spec Kit
specification under `specs/` before implementation diverges across multiple
files. Specs describe WHAT and WHY for stakeholders; ADRs record the
technical decision and its rationale. Implementation MAY proceed in parallel
with drafting once the shape of the decision is clear, but the documents
MUST land in the same change set as the feature, not as a retroactive
afterthought.

**Rationale**: The project has 35+ ADRs and this history is what lets a new
contributor (or a fresh agent session) reconstruct "where were we" quickly.
Skipping documentation degrades that asset.

### V. Verify Before Claiming Done
Before reporting a task complete, the relevant crate/package MUST be checked
against its actual target (`cargo check --target wasm32-unknown-unknown` for
the engine crate, native `cargo check` for the server, `tsc`/build for the
web app) and, for UI-affecting changes, exercised in a running dev instance.
Compilation warnings introduced by new code MUST be resolved or explicitly
justified; pre-existing warnings are not blocking.

**Rationale**: The engine crate only compiles under wasm32 — a native
`cargo check` will always fail and is not a signal. Knowing the right check
per crate prevents false "it's broken" or false "it's fine" conclusions.

## Technology & Architecture Constraints

- Canvas rendering and interaction: Bevy (Rust, compiled to WASM). No
  additional third-party canvas/whiteboard libraries may be introduced for
  simulation surfaces; tldraw is being removed as part of this transition
  and MUST NOT be reintroduced for token/wall/lighting/annotation authoring.
- Backend: Rust (Axum + async-graphql + Diesel/PostgreSQL), with NOTIFY/LISTEN
  for real-time fan-out and RxDB-based client replication for offline-capable
  sync where already established.
- Frontend shell: React + the existing fantasy design system
  (`apps/web/src/components/ui/`, `apps/web/src/styles/`) built on Radix
  primitives. New UI chrome around the Bevy canvas (toolbars, tool panels,
  property inspectors) belongs in this layer, not inside the engine crate.
- Migrations: Diesel migrations under `src/server/migrations/`, one directory
  per change, with paired `up.sql`/`down.sql`.

## Development Workflow

- Features with meaningful scope go through: ADR (if architecturally
  significant) → Spec Kit spec (`/speckit-specify`) → plan
  (`/speckit-plan`) → tasks (`/speckit-tasks`) → implementation.
- Small, well-understood changes (bug fixes, isolated refactors) may skip
  the Spec Kit flow but still respect Principles I-III.
- Commits are scoped to a coherent unit of work and reference the phase or
  feature they belong to, consistent with existing `Phase N.M: ...` commit
  message conventions.

## Governance

This constitution supersedes ad-hoc practice where the two conflict. ADRs
under `docs/adrs/` remain the authoritative record of individual technical
decisions; this document governs the process and non-negotiable boundaries
those decisions must respect.

Amendments require: a description of the change, a version bump per the
rules below, and an update to this file's Sync Impact Report. Amendments
that remove or redefine a principle are MAJOR; new/expanded principles or
sections are MINOR; clarifications and wording fixes are PATCH.

Compliance is reviewed at PR/change-review time. Any deviation from
Principle I (ECS owns simulation) or Principle III (ownership enforcement)
requires explicit justification recorded in the associated ADR or spec.

**Version**: 1.0.0 | **Ratified**: 2026-08-20 | **Last Amended**: 2026-08-20
