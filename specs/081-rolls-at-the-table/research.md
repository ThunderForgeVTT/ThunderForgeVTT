# Research: Rolls at the Table

## R1. Hiding a GM only roll from the stream

**Decision**: filter per subscriber. In `world_events_created`, an event
with code 36 whose payload says `gm_only` is passed only to a subscriber who
is the world's GM or an admin. The check runs when such an event arrives
(`is_dm_of_world`), not once at subscribe time, so a GM demoted mid-session
stops receiving them. `worldEventsSince` applies the same rule in SQL
(`NOT (event_code = 36 AND token_event->>'visibility' = 'gm_only')` unless
the caller is GM or admin).

**Why**: FR-005a allows either filtering or a "nothing to fetch" event, as
long as a player cannot tell a roll happened. A "nothing" event still says
something happened at that moment. Filtering says nothing.

**Gaps in event ids**: event ids come from one sequence shared by every
world, so a client already sees gaps and treats none as a signal. A dropped
event adds no new tell.

**Alternatives rejected**: a separate GM-only broadcast channel per world
(two channels to keep in order with each other, and catch-up still needs
the filter); encrypting the payload (no payload content exists to protect).

**Cost**: one membership query per `gm_only` roll per subscriber. GM only
rolls are rare and the query is indexed.

## R2. One rule, used everywhere

**Decision**: `crates/thunderforge-server/src/rolls/visibility.rs` holds
two pure functions:

- `may_roll(visibility, role) -> Result<(), Refusal>` (FR-007);
- `view_of(roll, viewer) -> RollView` where `RollView` is `Whole`,
  `Masked` or `Hidden` (FR-003).

`viewer` is `{ user_id, is_gm, is_admin }`. The fetch, the feed, the
stream filter and the catch-up all call `view_of` (the stream and catch-up
only need the `Hidden` case). A revealed roll is `Whole` for everyone.

**Why**: the rule is short; four copies of it would drift. Pure functions
are tested without a database.

## R3. When a board animates

**Decision**: the roll sync animates a roll when all hold:

1. the event arrived on the live subscription, not from catch-up;
2. the fetch answered `WorldRoll`, not `MaskedRoll` or nothing;
3. less than `REPLAY_WINDOW_MS = 4000` passed between the event's arrival
   and the fetch's answer.

For a reveal (code 37) the same holds and the roll animates once more.

**Why**: the window is measured on the client's own clock, from receipt to
answer, so server and client clocks never have to agree. 4 s is the dice's
settle time (1.2 s, `SETTLE_DURATION_SECS`) plus the slowest fetch worth
animating; a roll later than that would land after the table has moved on.

**The rolling tab**: it receives its own event like every other client and
animates from it. Components stop calling `triggerDiceRollAnimation`
(FR-013). The panel still shows the result from the mutation's answer,
timed by its existing `ANIMATION_REVEAL_MS`.

**A board in a tab without the engine** (the sheet page): the sync runs
only where the play view has mounted the engine. The sheet page shows its
result in place and lets the play tab animate.

## R4. Attacks and checks

**Decision**:

- `rollCheck` goes through `roll_dice_impl`, so it publishes code 36 with
  `everyone` and the check's name as its label at no extra cost.
- `combat/attack.rs::roll_and_record` records code 36 (`everyone`, label
  the attack's name) for each roll in the same transaction. The attack
  event (29) stays as it is, for the attack panel.
- `AttackFlow.tsx` stops animating locally.

**Why**: one path to the board for every roll. FR-001 allowed the attack
event to carry the roll id instead, but then the roll sync would need a
second fetch path through `attack(id)`.

## R5. The masked shape

**Decision**: a GraphQL union `WorldRollEntry = WorldRoll | MaskedRoll`.
`MaskedRoll` has `id`, `rollerName`, `createdAt`, `visibility` and nothing
else. It is built from `(id, roller name, created_at, visibility)` by a
constructor that never sees the roll row's content columns.

**Why**: FR-004. A union makes the client branch on `__typename`, and the
masked resolver cannot leak a field it does not have.

## R6. The demo's tabs

**Decision**: `apps/demo/src/backend/tabs.ts`.

- Every demo tab calls
  `navigator.locks.request("thunderforge-demo-world", hold)`. The tab
  granted the lock is the **holder** and keeps the lock until it closes.
- Other tabs are **guests**. A guest sends
  `{ kind: "request", id, viewer, query, variables }` on
  `BroadcastChannel("thunderforge-demo")` and awaits
  `{ kind: "answer", id, result }`.
- The holder runs every request through `execute.ts` as that request's
  viewer, saves, then posts each released event as
  `{ kind: "event", event }`. Every tab, holder included, delivers events
  to its own subscribers, filtered for its own viewer by the same rule as
  the server (`gm_only` roll events reach GM tabs only).
- **Takeover** (FR-016): when the holder closes, the lock passes to a
  waiting guest. It loads the saved world from `localStorage`, becomes the
  holder and posts `{ kind: "holder" }`. Guests resend any request still
  unanswered. The holder saves after every mutation, before it answers, so
  no answered change is lost.
- **Fallback**: without `navigator.locks` or `BroadcastChannel`, every tab
  is its own holder, as today (spec Edge Cases).

**Why not a SharedWorker**: Safari only gained it recently, and moving the
backend into a worker would move `execute.ts` and its imports with it. The
lock costs one module.

## R7. The viewer belongs to the tab

**Decision**: the demo's GM / player switch moves from the saved world
to `sessionStorage`, per tab. Every request carries it.

**Why**: two tabs, one as GM and one as player, is the demo's way to show
US3 and US4. A viewer saved in the shared world would flip every tab at
once.

## R8. The visibility choice per device

**Decision**: `localStorage["thunderforge.rollVisibility"]` holds the last
choice. A stored value the current role may not use reads as `everyone`.
Reading and writing are wrapped so a blocked storage means `everyone`.
