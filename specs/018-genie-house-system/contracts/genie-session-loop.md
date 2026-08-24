# Contract: Genie Session Loop (Session Wish Pool, Clocks, Session Resources)

All four mutations below broadcast a `world_events` row with `event_code = 15` (data-model.md) on success, consumed by the existing `worldEventsCreated(worldId)` subscription (spec 005) — no new subscription is defined.

## Shape

```graphql
enum GenieSessionStatus { ACTIVE, WON, LOST }

type GenieSession {
  id: ID!
  worldId: ID!
  wishesRemaining: Int!
  doomClockCurrent: Int!
  doomClockMax: Int!
  status: GenieSessionStatus!
  puzzleClocks: [GeniePuzzleClock!]!
}

type GeniePuzzleClock {
  id: ID!
  sessionId: ID!
  label: String!
  segmentsCurrent: Int!
  segmentsMax: Int!
  resolvedAt: String
}

type GenieResourceHolding {
  actorId: ID!
  resourceType: String!
  quantity: Int!
}

type Mutation {
  # GM-only (research.md R8). Rejects if wishesRemaining is already 0.
  # `narrativeEffect` is free text describing the GM-adjudicated Wish Effect (FR-014) — not a dice roll.
  spendWish(sessionId: ID!, narrativeEffect: String!): GenieSession!

  # GM-only. `delta` is typically 1 but left open for GM discretion (a severe complication
  # might warrant advancing more than one segment at once).
  advanceDoomClock(sessionId: ID!, delta: Int!): GenieSession!

  # GM-only. Also GM-only to create a new Puzzle Clock at session start / mid-session.
  createPuzzleClock(sessionId: ID!, label: String!, segmentsMax: Int!): GeniePuzzleClock!
  advancePuzzleClock(clockId: ID!, delta: Int!): GeniePuzzleClock!

  # Two-party consent (research.md R8) — a propose/accept pair, not a single call.
  # `proposeResourceTrade` is callable by either party; `acceptResourceTrade` must be
  # called by the OTHER party named in the proposal, or it is rejected.
  proposeResourceTrade(
    sessionId: ID!,
    fromActorId: ID!, fromResourceType: String!, fromQuantity: Int!,
    toActorId: ID!, toResourceType: String!, toQuantity: Int!
  ): GenieTradeProposal!
  acceptResourceTrade(proposalId: ID!): [GenieResourceHolding!]!   # returns both parties' updated holdings

  # Player-callable — spending one's own pooled/individual holdings against a specific
  # Puzzle Clock is not a trade (single actor, no counterpart), so no consent step needed
  # beyond having sufficient quantity (data-model.md validation rules).
  spendResourceOnPuzzleClock(clockId: ID!, actorId: ID!, resourceType: String!, quantity: Int!): GeniePuzzleClock!
}

type GenieTradeProposal {
  id: ID!
  sessionId: ID!
  fromActorId: ID!
  toActorId: ID!
  # ... offer details, expires if not accepted within a session-configurable window
}

type Query {
  genieSession(worldId: ID!): GenieSession   # the world's active session, if any
  genieResourceHoldings(sessionId: ID!, actorId: ID): [GenieResourceHolding!]!
}
```

## Authorization contract (research.md R8, Constitution Principle III)

- `spendWish`, `advanceDoomClock`, `createPuzzleClock`, `advancePuzzleClock` — **GM-only**, enforced server-side the same way existing "DM-only" mutations are enforced in specs 011/013. A non-GM caller is rejected, not silently ignored.
- `proposeResourceTrade` — callable by either the `fromActorId` or `toActorId` player (their own actor). `acceptResourceTrade` — callable **only** by the actor named as the counterpart in the proposal; the proposer cannot accept their own proposal.
- `spendResourceOnPuzzleClock` — callable by the actor spending their own holdings; cannot spend another actor's holdings on their behalf.
- Every mutation is scoped to a `session_id`/`world_id` the caller is a member of, per the platform's existing world-membership check (same pattern spec 005's research.md flags as required for `worldEventsCreated`).

## Win/loss evaluation (FR-016)

Evaluated server-side, inside `advancePuzzleClock` (checking for win) and `advanceDoomClock` (checking for loss), in that precedence order per spec.md's Edge Cases (a Puzzle Clock resolving in the same action that would also fill the Doom Clock resolves as a win, not a loss):

1. On `advancePuzzleClock`: if this clock just reached `segmentsMax` AND every other `world_genie_puzzle_clocks` row for the session now has non-null `resolvedAt`, set `world_genie_sessions.status = 'won'`.
2. On `advanceDoomClock`: if `doomClockCurrent` just reached `doomClockMax` AND the session's `status` is still `'active'` (i.e. the win check above didn't already fire), set `status = 'lost'`.
