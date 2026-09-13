# Contract: GraphQL surface

**Spec**: [../spec.md](../spec.md) · **Data model**: [../data-model.md](../data-model.md)

Every new root field is classified in `graphql/admin_surface_tests.rs`
(operator fields in `ADMIN_ONLY`) and in `graphql/play_pause_surface_tests.rs`
(see [live-play-lock.md](live-play-lock.md)).

## Error code

Every refusal because a world is paused is a GraphQL error with

```json
{ "message": "Play in this world has been paused by an operator.",
  "extensions": { "code": "WORLD_PLAY_PAUSED", "worldId": "<uuid>", "pausedAt": "<iso8601>" } }
```

The message and extensions never carry grounds, a trigger, or who paused it.

## Member-facing

### `worldPlayState(worldId: UUID!): WorldPlayState!` (query)

Guard: `require_world_member`, not the pause gate. Answers while paused (FR-024).
Admins who are not members are refused, as with any member query.

```graphql
type WorldPlayState {
  paused: Boolean!
  pausedAt: DateTime        # null when not paused
  history: [PlayPauseSpan!]! # newest first, for "that and when" (FR-050)
}
type PlayPauseSpan { pausedAt: DateTime!  liftedAt: DateTime }
```

No field for grounds, trigger, operator or request exists on these types, so none
can be added to a query by a client.

## Operator-facing (all guarded by `admin_user`)

### Queries

```graphql
playPauseRequests(state: PauseRequestState = PENDING, first: Int, after: String): PauseRequestConnection!
playPauses(active: Boolean, worldId: UUID, first: Int, after: String): PlayPauseConnection!
playPauseCandidates(search: String!, first: Int = 20): [PauseCandidateWorld!]!

type PauseRequest {
  id: UUID!  worldId: UUID!  worldName: String!  worldExists: Boolean!
  raisedAt: DateTime!  state: PauseRequestState!
  playedNow: Boolean!              # world_live_play at read time (FR-031)
  triggers: [PauseTrigger!]!
  decidedBy: OperatorName  decidedAt: DateTime  decisionNote: String
}
type PlayPause {
  id: UUID!  worldId: UUID!  worldName: String!  worldExists: Boolean!
  pausedBy: OperatorName!  pausedAt: DateTime!  grounds: String!
  requestId: UUID  triggers: [PauseTrigger!]!
  liftedBy: OperatorName  liftedAt: DateTime  liftGrounds: String
  playedNow: Boolean!
}
type PauseTrigger {
  kind: PauseTriggerKind!   # TAKEDOWN | OPERATOR | ABUSE_REPORT
  moderationActionId: UUID  entityType: String  entityId: UUID  note: String  recordedAt: DateTime!
}
type OperatorName { id: UUID!  name: String! }
type PauseCandidateWorld { id: UUID!  name: String!  ownerName: String!  playedNow: Boolean!  paused: Boolean! }
```

### Mutations

```graphql
pauseWorldPlay(worldId: UUID!, grounds: String!): PauseOutcome!
decidePlayPauseRequest(requestId: UUID!, decision: PauseDecision!, note: String!): PauseDecisionOutcome!
liftWorldPlayPause(pauseId: UUID!, grounds: String!): PlayPause!

enum PauseDecision { APPROVE DECLINE }

type PauseOutcome {
  pause: PlayPause!
  alreadyPaused: Boolean!   # true when an active pause existed; grounds were added to it as an OPERATOR trigger (FR-036)
}
type PauseDecisionOutcome {
  request: PauseRequest!
  decidedHere: Boolean!     # false when another operator decided first (FR-035); request carries their decision
  pause: PlayPause          # set when approved, here or by the other operator
}
```

| Field | Refuses with | When |
|---|---|---|
| `pauseWorldPlay` | `GROUNDS_REQUIRED` | blank grounds (FR-004) |
| | `WORLD_NOT_FOUND` | no such world |
| `decidePlayPauseRequest` | `GROUNDS_REQUIRED` | blank note |
| | `REQUEST_NOT_FOUND` | no such request |
| `liftWorldPlayPause` | `GROUNDS_REQUIRED` | blank grounds |
| | `PAUSE_ALREADY_LIFTED` | lifted already; extensions carry `liftedBy` and `liftedAt` |
| all three | the existing admin refusal | caller is not an operator, including a world's Owner (FR-006, FR-040) |

**Effects of a pause** (`pauseWorldPlay`, or approval), in one transaction:
insert `world_play_pauses`, attach an `OPERATOR` trigger (or, on approval, set
`request_id`, leaving the request's triggers on the request), record `EVENT_CODE_WORLD_PLAY_PAUSED`. After commit, nothing
else is needed: streams end on their next tick.

**Effects of a lift**: set the lift columns. No content, membership or moderation
state is touched (FR-041, FR-042).

## Not a GraphQL field: raising a request from a takedown

`play_pause::raise_for_takedown(conn, world_id, moderation_action)` is called
from `submit_takedown_notice_impl` after the takedown's work, for the target's
world and for every world `reach::fan_out_disable` reached, when that world is in
live play. It is non-fatal and idempotent per `(moderation_action_id, world)`.
Restoring content (`resolve_moderation_case_impl`, `restore_case_sync`, the lazy
restore in `effective_status_sync`) does not call into `play_pause` at all
(FR-042), and a test asserts a restore leaves an active pause active.
