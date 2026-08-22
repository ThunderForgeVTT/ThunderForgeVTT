# Contract: World Session Notes (new field + mutation)

## Shape

```graphql
type GraphQLWorld {
  # ...unchanged existing fields...
  sessionNotes: String       # NEW — nullable, the DM's latest between-sessions recap
}

input UpdateWorldSessionNotesInput {
  worldId: ID!
  notes: String!             # empty string is a valid, explicit save (FR-013)
}

type WorldMutation {
  updateWorldSessionNotes(input: UpdateWorldSessionNotesInput!): GraphQLWorld!
}
```

`sessionNotes` is served on the existing `world(id: ID!): GraphQLWorld` query (`getWorld`) — no new query is introduced; Session Setup already fetches the world record for other purposes (name, etc.) and now also reads this field.

## Authorization

- **Read** (`sessionNotes` on `world`): same visibility rule as every other field on `GraphQLWorld` — any world member or the world's owner (existing `require_visible_world`-equivalent check already applied to the `world` query). No new read restriction.
- **Write** (`updateWorldSessionNotes`): DM/GM-only (FR-012) — enforced server-side via the same `is_dm_of_world`-style check spec 010 introduced for actor creation (research.md §2). A Player-role caller (or any non-member) MUST receive an error; the client additionally hides the Save control for non-DM/GM callers as a UX convenience only, never the actual gate (Principle III).

## Behavior

- **FR-013**: Saving an empty string (`notes: ""`) is a valid, successful mutation — it is not rejected as "no change" and does not silently no-op. The stored value becomes an empty string (or is normalized to `NULL` server-side; both render identically as "No notes yet" client-side per data-model.md's note on this).
- The mutation returns the updated `GraphQLWorld` so the caller's local state can be refreshed from the single round trip without a follow-up `getWorld` fetch.

## Tests (contract-level expectations, not exhaustive)

1. A DM/GM calling `updateWorldSessionNotes` with non-empty text succeeds and the new text is immediately visible via `world.sessionNotes`.
2. A DM/GM calling it with `notes: ""` succeeds (does not error, does not leave the prior value in place).
3. A Player-role world member calling it is rejected with an authorization error; the world's `sessionNotes` is unchanged.
4. A user with no relationship to the world calling it (or reading `world.sessionNotes`) is rejected, matching every other world-scoped operation's non-member rejection.
5. A brand-new world (never had notes set) returns `sessionNotes: null` from the `world` query — the client renders this as an empty state, not an error.
