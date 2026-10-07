# Implementation Plan: Rolls at the Table

**Branch**: `main` | **Date**: 2026-10-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/081-rolls-at-the-table/spec.md`

## Summary

A roll becomes something the table sees. Every roll the server records also
records a world event, in the same transaction, carrying only the roll's id
and its visibility. Each client fetches the roll, and the server decides on
that fetch what this caller may see: the whole roll, a masked roll, or
nothing. Boards animate from the event, so a sheet in one tab rolls onto the
board in another without the tabs talking to each other.

- **Server**: `world_roll_records` gains `visibility`, `label`,
  `revealed_at` and `revealed_by`. `rollDice` takes a visibility and a label;
  `revealRoll` is new. `worldRoll(id)` and `worldRolls` answer a union of
  `WorldRoll` and `MaskedRoll`. Two event codes, 36 (made) and 37
  (revealed). The subscription and `worldEventsSince` drop a `gm_only` roll
  event for a subscriber who is not the GM
  ([R1](./research.md#r1-hiding-a-gm-only-roll-from-the-stream)).
- **Web**: one roll sync (`engine/world/sync/rolls.ts`) fetches each roll
  event and, when it arrived live and may be seen whole, animates it on the
  board (R3). The three local `triggerDiceRollAnimation` calls go. The chat
  panel shows rolls between messages, masked ones as `****`, with a reveal
  button for the GM. A visibility picker sits on the dice roller and both
  sheets. The sheet page gains the in-pane sheet's roll buttons.
- **Demo**: the dice handler applies the same rules. One tab holds the
  world (Web Locks); other tabs send it their requests and receive its
  events over `BroadcastChannel`; the viewer becomes per tab (R6, R7).

What is deliberately not built: blind rolls hidden from the roller, hidden
attacks and checks, roll retention. All three are out of scope in the spec.

## Technical Context

**Language/Version**: Rust 2021 (server); TypeScript 5 (web, demo).

**Primary Dependencies**: `async-graphql` (union type, enum), Diesel
(migration, filtered catch-up query). Web: none new. Demo: platform
`navigator.locks` and `BroadcastChannel`, no library.

**Storage**: PostgreSQL. One migration on `world_roll_records`. The demo's
saved world keeps its version; a roll without `visibility` reads as
`everyone` (data-model.md).

**Testing**: `cargo test -p thunderforge-server` (visibility rules, fetch
per role, refusals, reveal, payload, stream filter); Vitest in
`apps/demo` (the same cases against `handlers/dice.ts`, the tab leader);
Vitest in `apps/web` (the roll sync's animate-or-not decision); Playwright
slice `rolls`; the demo's Playwright for two tabs.

**Target Platform**: Linux server; Chromium/Firefox/WebKit; the demo on the
static host.

**Project Type**: web service + web app + demo app.

**Performance Goals**: SC-001 and SC-004, 1 s on a local stack. One extra
fetch per roll per member, the same cost chat already pays.

**Constraints**: no hidden content in an event payload (FR-002); the masked
type has no content field (FR-004); a player's client cannot tell a
`gm_only` roll happened (FR-005a); no service worker (FR-018); no
client-side database (AGENTS.md).

**Scale/Scope**: one migration, two mutations changed or added, two
queries, two event codes, one stream filter; one sync module, one picker,
three components changed, one page gains rolls; one demo handler, one demo
tab module.

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- **I. ECS owns simulation, React owns chrome** — pass. The board is told
  to animate through the existing `triggerDiceRollAnimation` page function,
  now called from the roll sync instead of from components. No component
  gains network code: the feed is a hook with `refetch()`, as AGENTS.md
  allows for reads the store does not hold.
- **II. Plugin-modular engine** — pass. The engine is unchanged.
- **III. Ownership and authorization at the data boundary** — pass, and it
  is the point of US3 and US4. Visibility is decided in one Rust function
  (`rolls/visibility.rs`) used by the fetch, the feed, the stream filter and
  the catch-up. Server tests fetch every visibility as every role.
- **IV. ADRs and specs before divergence** — pass. The id-only payload is
  ADR-000's rule and chat's precedent.
- **V. Verify before claiming done** — `cargo clippy` on the host,
  `pnpm -F web typecheck`, `pnpm -F @thunderforge/demo typecheck`, the
  GraphQL contract check.
- **VI. Every feature is proven by its own slice** —
  - **Slice**: `rolls`, run as `pnpm e2e:rolls`.
  - **Own specs**: `rolls-*.spec.ts` (prefix) in `apps/web/e2e/`.
  - **Standalone half**: yes. `e2e:rolls:standalone` runs the demo's Vitest
    and its `rolls-across-tabs.spec.ts`; the demo needs no stack.
  - **Neighbours, by the seam each crosses**:
    - `dice-roll.spec.ts`: the panel no longer animates locally.
    - `combat-attack.spec.ts`: attacks now animate from the roll event.
    - `chat-panel.spec.ts`: the chat panel now interleaves rolls.
    - `roll-check.spec.ts`: checks now publish an event.
    - `genie-manifestation-roll.spec.ts`: it rolls through `rollDice`.
    - (Confirmed with `pnpm e2e:which --diff` once the code lands.)
  - **Cross-cutting?** The subscription filter touches every world event.
    It passes every event but one code untouched, and the full suite runs
    before the owner deploys.

No violations; Complexity Tracking is empty.

**Post-design re-check**: unchanged. No dependency, service or store added.

## Project Structure

### Documentation (this feature)

```text
specs/081-rolls-at-the-table/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R8
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── graphql-rolls.md
│   └── demo-tabs.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/thunderforge-server/
├── migrations/2026-10-07-100000-0000_roll_visibility/{up,down}.sql
├── src/schema.rs                         # four columns
├── src/models.rs                         # RollRecord, NewRollRecord
├── src/world_events.rs                   # codes 36, 37
├── src/rolls/{mod,visibility}.rs         # new: the one visibility rule
├── src/graphql/mutations_roll.rs         # visibility, label, event; revealRoll
├── src/graphql/queries/roll.rs           # worldRoll, worldRolls
├── src/graphql/types_rolls.rs            # new: WorldRoll, MaskedRoll, union
├── src/graphql/subscriptions.rs          # drop gm_only roll events per subscriber
├── src/graphql/queries/world_events_since.rs  # same filter on catch-up
└── src/combat/attack.rs                  # roll event for each attack roll
apps/thunderforge/schema.graphql          # regenerated
apps/web/src/
├── api/roll.ts                           # visibility, label, worldRoll(s), revealRoll
├── engine/world/sync/rolls.ts            # new: fetch, animate, notify the feed
├── engine/world/sync/playPanels.ts       # codes 36, 37
├── components/world/RollVisibility/      # new: picker + stored choice
├── components/world/DiceRollerPanel/DiceRollerPanel.tsx
├── components/world/PlayDock/InPaneCharacterSheet.tsx
├── components/world/PlayDock/AttackFlow/AttackFlow.tsx
├── components/world/PlayDock/ChatPanel.tsx    # roll entries, reveal
├── hooks/useWorldRolls.ts                # new: feed with refetch()
└── pages/world/actor/ActorDetailPage.tsx # roll buttons
apps/web/e2e/rolls-*.spec.ts
apps/demo/src/
├── backend/handlers/dice.ts              # visibility, masking, reveal
├── backend/events.ts                     # codes, per-viewer delivery
├── backend/tabs.ts                       # new: leader, proxy, fan-out
├── backend/state.ts, actors.ts           # viewer per tab
└── (apps/demo/e2e/rolls-across-tabs.spec.ts)
```

## Complexity Tracking

None.
