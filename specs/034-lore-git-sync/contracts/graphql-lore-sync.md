# Contract: Lore Synchronisation GraphQL Surface

**Feature**: `034-lore-git-sync` · **Date**: 2026-09-04

The first delivery is Stories 1 and 2. Nothing here writes to a world's lore —
that is the property that makes the delivery safe by construction, and it is
visible in this contract as the absence of any mutation touching a lore entry.

Every field is authorised server-side at this boundary (Constitution Principle
III). Authority is **re-checked per call and per synchronisation run**, never
captured at connection time (FR-003).

---

## Queries

### `loreRepositoryConnection(worldId: UUID!): LoreRepositoryConnection`

The world's connection, or null. Readable by any world member; the fields that
describe *state* are the point, and hiding a broken connection from players who
can see the lore it mirrors helps nobody.

```graphql
type LoreRepositoryConnection {
  id: UUID!
  worldId: UUID!
  repositoryRef: String!
  branch: String!
  directory: String!
  incomingEnabled: Boolean!
  state: LoreSyncState!          # WORKING | NEEDS_ATTENTION | NEVER_CONFIGURED | DEACTIVATED
  stateReason: String            # plain language, names the remedy (FR-029)
  noticeAcknowledgedAt: DateTime
  lastSyncedAt: DateTime
  # FR-040a. What was **observed** at the last run, not a guarantee —
  # visibility changes at the host without telling us, so every surface that
  # shows this must show when it was last seen. Null before the first run, and
  # a null must never be treated as "private": the notice for an unobserved
  # repository has to be as loud as the notice for a public one.
  repositoryIsPublic: Boolean
  visibilityCheckedAt: DateTime
  fidelityNotes: [LoreFidelityNote!]!
}
```

**`DEACTIVATED` is a fourth state and not a flavour of `NEEDS_ATTENTION`**
(FR-041c). It is the only state a Game Master cannot leave by fixing
something, and a client must not offer them a retry for it — telling someone to
check a connection they are not permitted to restore leaves them retrying
forever.

*Corrected 2026-09-04: this type originally omitted `DEACTIVATED`,
`repositoryIsPublic` and `visibilityCheckedAt`, all three of which the
requirements already demanded. Two independent implementations reached for
`data-model.md` instead, which is the signal that the contract was the thing
that was wrong.*

**`installationRef` and `hostKind` are deliberately absent from the API.** They
exist in the row and are read at the grant boundary only (FR-004c). A client
that could read them would be a client that could branch on the host, which is
the thing FR-004 forbids.

**No credential field exists at any depth.** FR-035 says credentials must never
be returned to any client; the cheapest way to guarantee that is for there to
be nothing to return.

### `loreSyncRuns(worldId: UUID!, limit: Int): [LoreSyncRun!]!`

Recent attempts, newest first. Owner-level authority (FR-002) — a run's failure
reason can name repository details a player has no business seeing.

### `instanceRepositoryIntegration: RepositoryIntegrationStatus!`

```graphql
type RepositoryIntegrationStatus {
  configured: Boolean!
  # When false, what the operator must do. Never a stack trace.
  operatorGuidance: String
}
```

FR-036b and FR-036c. An instance whose operator has registered no application
answers `configured: false`, and the world settings surface renders nothing
connectable. **A Game Master must never be shown a flow that cannot complete**,
so this is queried before the connection UI is offered, not after it fails.

This is also where R1's `git` binary check surfaces: a missing binary makes the
integration unusable for the same reason a missing registration does, and the
operator deserves to learn it at the same moment.

---

## Mutations

All require owner-level authority over the world (FR-002).

### `beginLoreRepositoryConnection(worldId: UUID!): ConnectionGrantHandoff!`

Starts the grant. Returns whatever the host adapter needs the user to be sent
to. **This is the one place a host-specific concept legitimately appears**
(FR-004b) — and its return type is opaque to the rest of the system.

The adapter behind it lives in `crates/thunderforge-repo-host`, not in this
feature's code. What crosses back into `lore_sync` is a credential and an
expiry; nothing that names a host does. FR-004c is enforced by the crate
boundary rather than by review.

### `completeLoreRepositoryConnection(input: CompleteConnectionInput!): LoreRepositoryConnection!`

```graphql
input CompleteConnectionInput {
  worldId: UUID!
  grantResponse: String!   # opaque; only the host adapter reads it
  branch: String           # default "main"
  directory: String        # default "lore"
}
```

Finishes the grant and creates the row. Fails, without creating anything, when:

- the world already has a connection (FR-001);
- the target repository and directory are already claimed by another world
  (FR-033);
- the grant covers more than the single repository being connected (FR-036a).

**`directory` defaults to a subdirectory rather than the repository root**, so
that an FR-032 collision with the user's own `README.md` is not the expected
first experience of the feature.

**`branch` and `directory` are validated, not sanitised.** Both reach `git`,
and a branch name beginning `-` is an argument rather than a ref — a value like
`--upload-pack=…` would otherwise be handed to `git` as an option. `directory`
refuses `.`, `..` and backslashes for the same reason `workspace::world_subtree`
does: a value trying to escape is a bug or an attack, and quietly rewriting it
into something safe hides both.

### `acknowledgeLoreSyncNotice(worldId: UUID!): LoreRepositoryConnection!`

Records FR-038's acknowledgement. **Synchronisation does not begin until this
succeeds** — a connection with a null `noticeAcknowledgedAt` is never picked up
by the background task.

The notice's content (FR-037) is a client concern, but the *gate* is here,
because a client-side-only gate is not a gate.

### `removeLoreRepositoryConnection(worldId: UUID!): Boolean!`

Deletes the connection and its working clone. Leaves the world's lore entirely
intact and the repository's contents untouched (FR-005). Never deletes anything
in the repository — a removal is the platform forgetting, not a retraction.

### `retryLoreSync(worldId: UUID!): LoreSyncRun!`

Requests a run now rather than at the next scheduled pass. Rate-limited; this
is a convenience for someone who has just fixed a credential, not a way to
drive synchronisation by hand.

### `resolveLoreSyncDivergence(input: ResolveDivergenceInput!): LoreSyncRun!`

FR-031. When the remote history no longer contains what the platform wrote, the
system stops and requires an explicit choice. The choice is between
**overwriting the divergent remote** and **abandoning the connection**; there is
no third option that silently reconciles, because reconciling would mean merging
prose, which FR-024 forbids everywhere in this spec.

---

## Not in this contract, and why

- **Anything that writes a lore entry.** Story 3 only. Its absence here is
  load-bearing.
- **Any query that lists connections across worlds.** FR-039 forbids
  aggregation, indexing, listing, searching or discovery. The only connection a
  caller can reach is one belonging to a world they already have authority over,
  reached *through* that world.
- **A repository browser.** The repository is the user's; the platform writes to
  it and does not present it.
