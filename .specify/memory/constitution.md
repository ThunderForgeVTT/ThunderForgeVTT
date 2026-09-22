<!--
Sync Impact Report
- Version change: 1.2.0 → 1.3.0
- Modified principles: n/a
- Added sections: Principle VI "Every Feature Is Proven by Its Own Slice".
  Every feature or feature set ships its own e2e entry point: a
  stack-free standalone slice where one is possible, and an integration
  slice that is the smallest set of specs crossing every seam the feature
  touches. The slice gates the feature; the full suite gates releases and
  cross-cutting changes. The model is spec 044's `pnpm e2e:hero-builder`.
- Removed sections: none
- Templates: plan/tasks templates are not edited by this command. The
  tasks template's proof task and the plan's Constitution Check will pick
  up Principle VI at runtime; see Next Actions in the amendment's summary.
- Deferred TODOs: none

Prior report (v1.2.0):
- Version change: 1.1.0 → 1.2.0
- Modified principles: n/a
- Added sections: the "DMCA / Content Moderation Guardrail" checkpoint gained
  an access-mode condition (spec 052-access-mode-and-legal-duty, ADR-103):
  the notice-and-takedown program is required of an instance that is **open**,
  and of any instance that publishes content outward by any path, rather than
  of every instance unconditionally. The checkpoint itself is unchanged for
  features that expose content beyond a world; what changed is that the
  guardrail now states which instances carry the program, and states plainly
  what the relaxation does not mean.
- Removed sections: none
- Deferred TODOs: none

Prior report (v1.1.0):
- Version change: 1.0.0 → 1.1.0
- Modified principles: n/a
- Added sections: Development Workflow gained a "DMCA / Content Moderation
  Guardrail" checkpoint (spec 015-dmca-notice-takedown, ADR-043) requiring
  the notice-and-takedown program to be operational, and an explicit
  "centralized public repository" determination on record, before any
  feature exposing one world's compendium content beyond that world may
  begin implementation.
- Removed sections: none
- Deferred TODOs: none

Prior report (v1.0.0):
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

### VI. Every Feature Is Proven by Its Own Slice
Every feature or feature set MUST be testable on its own. Proving a change
MUST NOT require the full end-to-end suite, which takes 30 minutes to an hour.
Each feature MUST ship an isolated e2e entry point as root scripts named
`e2e:<feature>`. The model is `pnpm e2e:hero-builder` (spec 044):

- **A standalone slice** (`e2e:<feature>:standalone`) wherever the feature
  can run without the stack: a package or harness app, served and tested
  with no database, bucket or server.
- **An integration slice** (`e2e:<feature>:integration`) that runs
  `scripts/e2e-parallel.mjs --only=...` on one shard, with a stack of its
  own. It MUST NOT depend on state left behind by specs outside the slice.
- **The combined script** (`e2e:<feature>`) runs both, standalone first.

An integration slice MUST be the *smallest* set of specs that crosses every
seam the feature touches. That is the feature's own specs, plus the
neighbouring specs of each existing surface it changes. For spec 044 that
meant adding `actor-art` for the actor imagery the builder writes. The
slice SHOULD select specs by a shared file prefix (`hero-builder-*`), so a
new spec joins without editing the script. It SHOULD finish in about ten
minutes on one shard. A slice that cannot is a sign the feature set is too
wide and SHOULD be split. When a feature starts touching a new surface, the
neighbour specs for that surface MUST be added to its slice in the same
change.

A feature's proof task in `tasks.md` MUST name its slice and record that
slice's result. A green slice is what "done" means for the feature. The
full suite remains the check before a release and for changes that cut
across the whole app, such as the schema, auth, the harness itself, or
shared UI primitives. It MUST NOT be the gate for an individual change.

**Rationale**: A gate that takes an hour gets skipped, run less often, or
run while other edits land in the tree. Any of those turns "tested" into a
guess. A slice that crosses exactly the feature's seams gives the same
confidence about that feature in minutes. It can run beside other work,
and one agent or contributor can own it end to end.

## Technology & Architecture Constraints

- Canvas rendering and interaction: Bevy (Rust, compiled to WASM). No
  additional third-party canvas/whiteboard libraries may be introduced for
  simulation surfaces; tldraw is being removed as part of this transition
  and MUST NOT be reintroduced for token/wall/lighting/annotation authoring.
- Backend: Rust (Axum + async-graphql + Diesel/PostgreSQL), with NOTIFY/LISTEN
  for real-time fan-out.
- Offline-capable sync is the client world cache (`thunderforge-cache-core`,
  `thunderforge-cache-browser`, `thunderforge-opfs`) plus the engine/GraphQL
  world-store bridge, together with the offline queue and its reconciliation on
  reconnect. RxDB was removed when it moved to a paid licence and MUST NOT be
  reintroduced; there is no client replication library in this stack.
- Supported browsers: **Chromium only**, for now. This is a real constraint
  rather than an omission — the world cache depends on OPFS, WebCrypto and
  IndexedDB, and `thunderforge-cache-browser` degrades rather than crashing
  where they are absent. A browser outside this set MUST be told plainly that
  its content cannot be kept on device, never shown an empty cache that reads
  as "nothing happened yet". The end-to-end suite runs Chromium alone, so any
  claim about another browser is currently untested by construction.
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
- **DMCA / Content Moderation Guardrail** (spec `015-dmca-notice-takedown`,
  ADR-043, FR-011/FR-012): before any feature is proposed that would make
  one world's compendium content (actors, items, lore entries, or any
  future content type) visible, copyable, searchable, or otherwise
  accessible outside that world — a public marketplace, a shared community
  compendium, cross-world content browsing, etc. — design/launch review
  MUST confirm both of the following before implementation begins: (a) the
  notice-and-takedown program (spec 015's User Stories 1-3: intake,
  disable, counter-notice/restoration, repeat-infringer tracking) is fully
  operational, and (b) an explicit, on-record determination of whether the
  proposed feature constitutes "a centralized public repository" for
  user-shared, potentially-copyrighted content under that spec's policy —
  if so, the feature must be redesigned or the risk explicitly accepted by
  an accountable owner before build work starts. This checkpoint exists
  because the platform's own legal research identifies exactly this
  feature category as its single highest-liability move.
- **Which instances carry that program** (spec `052-access-mode-and-legal-duty`,
  ADR-103, amended 2026-09-15). The guardrail above was written as though
  every instance shares. It does not. An instance's access mode — open,
  invite-only or closed (spec 035) — decides which legal capabilities it MUST
  carry:
  - An **open** instance MUST carry the notice-and-takedown program of spec
    015 in full, and MUST hold a published contact on which a copyright
    notice can be served without an account, **before** it becomes open. A
    change to open that would leave that contact unset MUST NOT take effect.
  - An **invite-only** or **closed** instance MUST NOT be required to supply
    that contact in order to be set up or operated, because it admits nobody
    and publishes nothing outside itself.
  - **In every mode**, the refusal to publish content beyond a world while
    that contact is unset stays in force. The relaxation is about when the
    question is asked, never about what an instance that has not answered it
    may do.

  This relaxation is narrow, and three things are NOT claimed by it:
  copyright does not stop applying to material in a private world; a rights
  holder loses no remedy, only a channel here, and an operator running
  without a safe harbour carries that exposure themselves; and "closed" is
  this product's word for an admission policy, true as a statement about
  reach only while no other path on the instance publishes outward. Any
  feature that adds such a path re-engages the checkpoint above regardless
  of access mode.

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

**Version**: 1.3.0 | **Ratified**: 2026-08-20 | **Last Amended**: 2026-09-22
