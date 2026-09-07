# Contract: The play-field claim

> **Revised during implementation, 2026-09-07.** This contract specified a
> `claimPlayField` mutation beside a `playFieldClaimChanged` subscription, and
> that shape cannot satisfy FR-030. The registry hands back a guard whose drop
> releases the claim, and **a mutation has nowhere to put one** — it would have
> to be stored, which needs a heartbeat and a reaper to decide when the storing
> client is gone, which is the precise failure this design exists to avoid.
>
> What was built instead: **subscribing is claiming**, exactly as `peerSignals`
> registration is itself the grant of reachability. `playField(worldId,
> clientId)` takes the table on open, carries every subsequent change including
> `null` for nobody, and releases on drop with no timeout and nothing to reap.
> `playFieldClaim(clientId)` is a query for a companion asking who holds it.
>
> There is no `releasePlayField`, and the SDL guard asserts there is not:
> closing the stream is the release, and a mutation able to release a claim
> would be a way to push a person off their own table from another window.
>
> The rules below still hold; the surface they are expressed through changed.


One claim per account. The client asks for it as it mounts the engine and
releases it as it tears the engine down; a companion surface never calls
anything here.

```graphql
type PlayFieldClaim {
  "The page-load id currently holding the table for this account."
  clientId: String!
  worldId: UUID!
  claimedAt: String!
  "True when the caller is the holder."
  isMine: Boolean!
}

extend type Mutation {
  """
  Take the play field for this account. Always succeeds for an authorized
  caller: the newest claim wins and any previous holder is demoted.
  """
  claimPlayField(worldId: UUID!, clientId: String!): PlayFieldClaim!

  "Release a claim this client holds. A no-op if it does not hold one."
  releasePlayField(clientId: String!): Boolean!
}

extend type Subscription {
  """
  Fires for the calling account whenever its claim moves. A demoted client
  learns here that it has become a companion.
  """
  playFieldClaimChanged: PlayFieldClaim!
}
```

## Rules

1. `claimPlayField` requires membership of the world; it is an authorization
   decision at the data boundary like any other (Principle III).
2. The claim is **not stored**. It lives only as long as the stream that owns
   it, so a crashed or closed client releases it without a timeout (FR-030).
3. Concurrent claims serialise; exactly one holder exists afterwards (SC-009).
4. A demoted client MUST stop applying world events to a canvas and say so —
   it is a companion until it claims again (FR-029).
5. `peerSignals` registration requires a held claim. A companion is refused,
   and therefore cannot be addressed by a peer at all (FR-038).

## Failure shapes

| Situation | Result |
|---|---|
| Caller is not a member of the world | Refused, as any world mutation would be |
| Caller already holds the claim | Succeeds, unchanged; claiming is idempotent |
| Two clients claim at the same instant | Both return; exactly one reports `isMine: true` on the following notification |
| Holder's connection drops | Claim released; the next claimant simply succeeds |
