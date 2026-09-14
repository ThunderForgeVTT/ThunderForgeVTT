# Implementation Plan: A Fight That Resolves

**Branch**: `046-a-fight-that-resolves` | **Date**: 2026-09-14 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/046-a-fight-that-resolves/spec.md`

## Summary

The combat tracker points at a combatant; nothing it points at can hurt
anyone. This plan joins a turn, a roll and the board, in the order the
playtest can prove it.

The server resolves an attack: it checks the turn, measures reach, range and
line of sight from footprint to footprint, rolls against the target's
declared defence, and sends the damage to whoever controls the target as an
**offer** to take or decline. A Game Master may resolve an absent player's
offer for them, and may switch on auto-apply for the NPCs they run. Hit
points become one record per creature: a **linked** token shares its actor's,
an **unlinked copy** holds its own, so two hundred goblins need no actors.
The pack declares what hit points, defence, sizes, a turn's budget and
legendary actions are; the platform enforces only turn order, and shows
everything else.

| Phase | What lands | Playtest FINDINGs cleared |
|---|---|---|
| 1 | Damage and healing as a server operation; bars move on every board; zero takes a combatant out | bar doesn't move (522), zero doesn't mark down (548) |
| 2 | Turn order refused for players, on every move path | a player moved out of turn (606) |
| 3 | Linked tokens and unlinked copies; unique NPCs; `tokens.health` retired | (new: copies hurt separately) |
| 4 | An attack with a target: defence, the roll shown to every seat and redacted per viewer, offers, auto-apply, offline attacks | roll not shown (263), no AC or target (416), longsword not rollable (439) |
| 5 | Size fills squares; reach, range and line of sight flagged | no size or reach (481) |
| 6 | The economy of a round | no turn economy (380) |
| 7 | Legendary actions and the lair | (new: legendary spent and refilled) |

Phases 1–5 are the spec's P1 stories; 6 is P2 and 7 is P3. Each phase ships
alone. Phase 4 needs 1 (the damage operation) and 3 (which record to write).
Phase 5's flags extend phase 4's attack record. Phase 6 extends 2 and 4, and
7 extends 6.

## Technical Context

**Language/Version**: Rust (server, native; engine, `wasm32-unknown-unknown`;
shared crates both), TypeScript 5 with React 19 (web).

**Primary Dependencies**: async-graphql 7, Diesel and Postgres (server);
`thunderforge-canvas-core` (grid, footprint, walls, vision), shared by engine
and server; `thunderforge-dice` (formula resolution with an injected RNG);
`pack_system_spec` (manifest schema); Bevy 0.19 (engine; no new system, only
the existing `set_token_grid` command); Playwright (e2e and playtest).

**Storage**: Postgres. New tables `world_attacks`, `world_offers`,
`world_combatant_budgets`. New columns on `tokens`, `world_actors`, `worlds`,
`world_combats`, `world_combatants`, `world_abilities` and `world_items`.
`tokens.health` and `tokens.max_health` are dropped.
See [data-model.md](./data-model.md).

**Testing**: `cargo test` for the server rules (contract C1–C10) and
canvas-core distance; Playwright e2e per phase; `pnpm playtest
--only=combat-5e` as the acceptance proof; `engine-status-limits.spec.ts`
extended with a 200-copy level. See [quickstart.md](./quickstart.md).

**Target Platform**: Chromium against a ThunderForge instance; the engine is
WASM in that browser.

**Project Type**: Existing multi-part repository: engine, server, web app,
shared crates and system packs.

**Performance Goals**: An attack, an offer's resolution and a hit-point change
reach every seat within one second (SC-001, SC-002). With 200 unlinked copies
on a scene, loading bars and footprints adds under 500 ms, and the frame rate
matches the 400-token status baseline (research R17).

**Constraints**:
- **Only turn order refuses a player.** Reach, range, line of sight and
  budgets are flags (clarification 1, decision 2).
- **No damage without the controller's say-so**, except auto-apply to NPCs the
  Game Master runs (decision 1, FR-005/006).
- **Nothing a redaction hides may appear in any payload** (FR-002a).
- **Shared code names no system's fields** (`check-system-registry.mjs`).

**Scale/Scope**: Seven phases. They touch:
- the server: attack resolution, offers, hit points, turn checks, budgets and
  per-viewer redaction;
- canvas-core: footprint distance;
- the web: an attack flow, an offer prompt, the tracker's budget and
  legendary columns, token-link controls, and footprint sync;
- two packs: 5e declares everything, and Genie moves its sizes.

No new service, and no new dependency.

## Constitution Check

*Gate before Phase 0; re-checked after Phase 1 design (below).*

| Principle | How this plan satisfies it |
|---|---|
| **I. ECS owns simulation** | The engine keeps footprint, snapping and hit-testing; the web only supplies each token's footprint through the existing `set_token_grid` command, as it supplies vision today. Distance, reach and line of sight for an attack are adjudication, not drawing, and live in canvas-core, called by the server. React renders the tracker, offers and attack log from server answers, and computes no rule. |
| **II. Plugin-modular engine** | No engine plugin changes shape. `TokenGridBehaviour` is already a component in the token grid resources; the only engine-side change is that something finally sends the command. |
| **III. Ownership at the data boundary** | The server rolls, judges, and writes hit points. An offer is resolvable only by the token's controllers or a Game Master; turn order is enforced in `moveOwnToken`, offline replay and `makeAttack`; redaction is decided server-side per viewer. New tables carry `created_by` / `updated_by`. |
| **IV. ADRs before divergent implementation** | Two ADRs land with their phases: **ADR-101** "An attack is resolved on the server, and its damage is an offer" (phase 4), and **ADR-102** "A token is its actor, or a copy of it" (phase 3). The `combat` manifest block amends the pack contract (ADR-027 lineage), documented in `packs/systems/README.md` in the same change. |
| **V. Verify before claiming done** | Per phase: `cargo check` (server), `make lint-wasm` when the engine changes, `pnpm -F @thunderforge/web exec tsc --noEmit`, `pnpm verify` (including the GraphQL contract steps), the phase's targeted e2e run through the harness with the log searched for `✘`, and the playtest. |

**Gate result (pre-research)**: PASS.

**Re-check after design**: PASS. The design adds one deliberate asymmetry,
recorded rather than waived. Spec 045 hides unseen tokens only in the engine,
while this plan withholds an unseen attacker's identity on the server (research
R8). It is a stricter rule for a narrower thing, and it does not contradict
Principle I: no canvas state moves out of the engine, and the server's
visibility test is the same canvas-core function the engine calls.

## Project Structure

### Documentation (this feature)

```text
specs/046-a-fight-that-resolves/
├── spec.md              # the specification, clarified 2026-09-14
├── plan.md              # this file
├── research.md          # Phase 0: R1–R17 and the loose ends found
├── data-model.md        # Phase 1: tables, columns, manifest blocks
├── contracts/
│   └── fight.md         # GraphQL, rules C1–C10, redaction, events, engine command, manifest
├── quickstart.md        # how each phase is proved
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/thunderforge-canvas-core/src/
├── grid.rs                         # phase 5: footprint_distance
crates/pack_system_spec/src/
├── lib.rs                          # phases 1,4,5,6,7: the typed combat block and budget
src/server/
├── migrations/                     # one directory per phase's schema change
├── src/combat/                     # new module: resolution, hit points, offers, budgets, redaction
│   ├── attack.rs                   # phase 4: makeAttack, previewAttack
│   ├── hit_points.rs               # phase 1: the damage operation
│   ├── offers.rs                   # phase 4
│   ├── turn.rs                     # phase 2: the turn check
│   ├── budget.rs                   # phase 6
│   ├── redaction.rs                # phase 4: per-viewer parties
│   └── manifest.rs                 # reads the pack's combat block
├── src/graphql/mutations_combat.rs # phases 1,6,7: downed_by, budgets, lair
├── src/graphql/mutations_tokens.rs # phases 2,3: turn check in moveOwnToken; placement defaults; setTokenLink
├── src/graphql/mutations_reconcile.rs # phases 2,4: turn check and offline attack intents
├── src/graphql/queries/token_status.rs # phase 3: read a copy's own data
├── src/graphql/queries/token_grid.rs   # phase 5: new
├── src/world_events.rs             # phase 4: codes 29, 30
apps/web/src/
├── engine/world/sync/tokenStatus.ts # phase 1: also re-read on event 26
├── engine/world/sync/tokenGrid.ts   # phase 5: new, shaped like tokenVision.ts
├── components/world/PlayDock/CombatPanel.tsx # phases 1,6,7: damage/heal, budget, legendary, lair
├── components/world/PlayDock/AttackFlow/     # phase 4: choose target, preview, roll
├── components/world/PlayDock/OfferPrompt/    # phase 4: take or decline
├── components/world/PlayDock/InPaneCharacterSheet.tsx # phase 4: attacks from the sheet
├── api/combat.ts, api/attacks.ts            # phases 1–7
packs/systems/dnd5e/system.json     # phases 1,4,5,6,7: hitPoints, defence, sizes, budget, legendary
packs/systems/genie/                # phase 5: sizeCategories → combat.sizes
packs/systems/README.md             # the combat block's contract
apps/web/playtest/combat-5e.playtest.ts # every phase: FINDINGs → hard checks
apps/web/e2e/combat-*.spec.ts, token-links.spec.ts # per phase (quickstart)
docs/adrs/                          # ADR-101 (phase 4), ADR-102 (phase 3)
```

**Structure Decision**: The repository's existing layout. The rules move into
one new server module, `src/server/src/combat/`, rather than growing
`mutations_combat.rs`, which is the file-length check's to police. Everything
else extends the file that already owns that concern.

## Complexity Tracking

No constitution violations to justify.

Risks named rather than tracked as complexity:

- **Dropping `tokens.health` is destructive.** Phase 3's migration copies any
  non-null value into a copy's `system_data` first. Its `down.sql` restores the
  columns from `system_data` where it can. Production data today has no screen
  that writes `health`, so little should be lost.
- **Per-viewer visibility on read** runs `visibility_of` per controlled token
  per attack. It is bounded (research R8), and measured in phase 4's e2e.
- **Controllers are movement's controllers** (research R7). Where claims and
  token owners disagree (`bringPartyToScene` assigns the Game Master), offers
  go to the wrong person until phase 3 sets owners from claims.
