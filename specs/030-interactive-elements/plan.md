# Implementation Plan: Interactive Elements — Props, Doors and Triggers

**Branch**: `030-interactive-elements` | **Date**: 2026-08-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/030-interactive-elements/spec.md`

## Summary

Give the Game Master things on a scene that respond: a prop that opens a lore
entry, a lever that toggles lights, a door that opens, closes and locks, a
region that fires when crossed, and a request the GM must approve.

The approach is a **contribution seam**, not a feature with six behaviours.
One plugin owns placing, triggering, permission and dispatch and owns no
effect at all. Every effect is contributed by the subsystem that performs it —
lighting contributes toggling, doors contribute open/close/lock/reveal, and
audio contributes a sound effect on the day it exists. The authorable
vocabulary is the union of what is compiled in, which is why an unbuilt
subsystem is not a problem to manage: it contributes nothing, so nothing dead
is ever offered.

Three surfaces have to agree on that vocabulary — the engine dispatches it,
the server validates and persists it, the web app offers it in the authoring
UI. The plan puts the declarations in `thunderforge-canvas-core`, which the
server already compiles and from which the web app's types are already
generated, so one definition serves all three.

## Technical Context

**Language/Version**: Rust (2024 edition) for engine, server and shared
crates; TypeScript 5.x + React 19 for the authoring UI.

**Primary Dependencies**: Bevy (wasm32) for the canvas and event dispatch;
Axum + async-graphql + Diesel/PostgreSQL for authoring, persistence and
authorization; ts-rs for the shared vocabulary's TypeScript types; existing
`worldEventsCreated` subscription for live fan-out.

**Storage**: PostgreSQL. One new table (`interactives`), one new transient
table (`interaction_requests`), two added columns on `walls`. Props reuse the
existing `tokens` table; no new placement pipeline.

**Testing**: `cargo test -p thunderforge_canvas_core` for the rules (the
engine crate's tests compile but never run — Constitution V); `cargo test -p
thunderforge` for authorization and persistence; Playwright for the table-level
behaviour, which is where "a player clicked a door and everybody saw it" is
actually provable.

**Target Platform**: Browser. Engine is wasm32-unknown-unknown; server is
native Linux.

**Project Type**: Web application with a WASM canvas engine — existing
structure, no new top-level layout.

**Performance Goals**: A scene with 50 interactives is as responsive as the
same scene without them, measured against the documented engine baseline
(SC-007). Activation reaches every viewer within one second (SC-003).

**Constraints**: The interaction plugin must compile and function with every
contributing subsystem removed (FR-039). Effect dispatch must not become a
per-frame cost — interactives are rare and event-driven, unlike status
displays which are per-token.

**Scale/Scope**: Tens of interactives per scene, not thousands. Contributors
in this feature: doors, lighting, navigation-request. Designed for more.

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design._

| Principle                                     | Assessment                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **I. ECS owns simulation, React owns chrome** | **Pass.** Interactives are canvas entities with position and hit-testing, so the engine owns them, dispatch and the door/light state changes. The GM's authoring UI — choosing an effect, configuring it, the approval queue — is chrome and belongs in React. The line to hold: React may _read_ the effect registry to build a form, and must never resolve an activation. |
| **II. Plugin-modular engine**                 | **Pass, and this is the feature.** `InteractionPlugin` is self-contained; contributors are separate plugins communicating through Bevy events. FR-039/FR-040 are the principle restated as testable requirements. The risk is doors — the effect most tempting to build into the core. US7 exists to catch that.                                                             |
| **III. Authorization at the data boundary**   | **Pass, with care.** Every authoring mutation is GM-only server-side. Activation is _also_ a mutation and must be authorized there — a locked door must be refused by the server, not merely by a client that chose not to offer the button. New tables carry `created_by`/`updated_by`.                                                                                     |
| **IV. ADRs before divergent implementation**  | **Action required.** The contribution seam is architecturally significant — a new dispatch mechanism spanning engine, server and web. An ADR must land in the same change set, not retroactively. Recorded as a task.                                                                                                                                                        |
| **V. Verify before claiming done**            | **Pass.** `cargo check --target wasm32-unknown-unknown` for the engine, native `cargo check` for the server, `tsc` for the web app, and a running dev instance for anything table-visible.                                                                                                                                                                                   |
| **DMCA / content guardrail**                  | **Not triggered, by design.** Link effects reference in-world content by id rather than carrying arbitrary URLs, so nothing here makes one world's content reachable from another. See research §5, which also settles the hostile-link edge case.                                                                                                                           |

**No violations to justify.** Complexity Tracking is therefore omitted.

### Post-design re-check

Re-evaluated after Phase 1. The design holds, with three things worth naming
because they are where it would slip:

- **Principle III is the one at risk.** "A player cannot open a locked door" is
  the kind of rule that gets implemented by not drawing the button. The
  contract states it as a server-side refusal and the quickstart's layer 4
  exists to prove it there, because a UI-only check passes every screen test
  and fails the moment somebody calls the mutation directly.
- **Principle II has a named failure mode.** Doors are the effect most likely
  to be built into the interaction core rather than contributed. FR-039 is
  written textually as well as behaviourally — the words "light", "door" and
  "sound" do not appear in that plugin's logic — so the violation is
  greppable rather than a matter of judgement.
- **Principle IV is outstanding.** The contribution seam is a new dispatch
  mechanism spanning engine, server and web, which is architecturally
  significant by the constitution's own definition. The ADR must land in the
  same change set as the implementation; it is listed in the project structure
  and belongs in the first task phase, not at the end.

One design choice deliberately trades against a principle and is worth stating
rather than burying: the engine dispatches optimistically for responsiveness
while the server remains authoritative. That is permitted by Principle III
("client and engine code MAY optimistically apply changes locally but MUST
treat the server as authoritative") and the engine-events contract says what
reconciliation means, but it does mean two paths produce the same visible
change. Any divergence between them is a bug in the client, never a
disagreement to resolve in the client's favour.

## Project Structure

### Documentation (this feature)

```text
specs/030-interactive-elements/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── graphql.md       # Authoring, activation and approval surface
│   ├── effect-registry.md  # What a contributor declares
│   └── engine-events.md    # The dispatch seam inside the engine
├── checklists/
│   └── requirements.md  # Spec quality checklist (passing)
└── tasks.md             # Phase 2 output (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/thunderforge-canvas-core/src/
├── interaction.rs        # NEW. Effect declarations, the registry, trigger and
│                         #      permission rules. Pure and tested — this is
│                         #      where the rules live, because engine tests
│                         #      never execute.
├── wall.rs               # EXTENDED. DoorState gains locked and secret, and
│                         #      the blocking rule that defines open/closed.
└── lighting.rs           # EXTENDED. Contributes its effect declarations.

src/engine/src/plugins/
├── interaction.rs        # NEW. InteractionPlugin: placement, hit-testing,
│                         #      trigger detection, permission, dispatch.
│                         #      Names no effect.
├── wall.rs               # EXTENDED. Contributes door effects; handles them.
└── lighting.rs           # EXTENDED. Contributes light effects; handles them.

src/server/src/
├── interaction.rs        # NEW. Registry validation, activation authorization,
│                         #      approval lifecycle. The rules it enforces come
│                         #      from canvas-core.
├── graphql/queries/interactives.rs    # NEW. Read side + effect registry.
├── graphql/mutations_interactives.rs  # NEW. Authoring, activation, approval.
└── migrations/           # NEW. interactives, interaction_requests, wall cols.

apps/web/src/
├── api/interactives.ts             # NEW. Client for the above.
├── components/InteractionAuthor/   # NEW. GM authoring panel, driven by the
│                                   #      registry rather than a hard-coded list.
├── components/ApprovalQueue/       # NEW. Pending requests for the GM.
└── engine/world/sync/interactives.ts  # NEW. Live updates into the world store.

apps/web/e2e/
├── interactive-prop.spec.ts        # US1
├── interactive-doors.spec.ts       # US2
├── interactive-lighting.spec.ts    # US3
├── interactive-secrets.spec.ts     # US4
├── interactive-regions.spec.ts     # US5
├── interactive-approval.spec.ts    # US6
└── interactive-contribution.spec.ts # US7 — the seam itself

docs/adrs/
└── ADR-0NN-interaction-effect-contribution.md  # NEW. Principle IV.
```

**Structure Decision**: No new top-level layout. The feature follows the shape
spec 029 established and validated — rules in `thunderforge-canvas-core` where
tests execute, a self-contained Bevy plugin for canvas behaviour, server-side
authorization at the GraphQL boundary, React for chrome, and Playwright for the
claims that only hold end to end. Props reuse the `tokens` table and doors
extend the `walls` table rather than either introducing parallel geometry.
