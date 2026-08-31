# Contract: GraphQL Surface

Authoring, activation and approval. Field names are indicative; the rules are
the contract.

## Queries

### `effectRegistry: [EffectDeclaration!]!`

What this build can perform. Drives the authoring form, so a Game Master is
offered exactly what exists (FR-038).

Returns `id`, `label`, `description`, `subjectKinds`, and `config` as typed
field descriptors.

**Authorization**: any world member. Knowing what effects exist is not a
secret.

### `interactives(sceneId: UUID!): [Interactive!]!`

Every interactive in a scene.

- A **Game Master** receives the authoring view: subject, effect,
  configuration, activation mode, fire state, and whether the effect is
  currently `available`.
- A **player** receives what is needed to interact and nothing more: which
  subjects are interactive and whether they may activate them. Not the effect,
  not its configuration, not what it targets.

That split is not a security boundary — per the spec's decision, secrets are a
table concern — it is an interface one. A player has no use for an effect's
configuration, and sending it would invite a client to render it.

## Mutations

### Authoring — Game Master only (FR-005)

- `createInteractive(input)` — subject, trigger, activation, optional effect
  and config.
- `updateInteractive(interactiveId, input)` — partial.
- `deleteInteractive(interactiveId)`.
- `resetInteractive(interactiveId)` — clears `firedAt` for a `once`
  interactive (FR-031).

**Rules**

- Rejected unless the caller runs the world.
- `effectId` must exist in the registry; `effectConfig` must validate against
  its declaration.
- `trigger: ENTER` is rejected for anything but a region.
- Subject and geometry must agree with `subjectKind`.

### `activateInteractive(interactiveId): ActivationResult!`

The one mutation a player calls.

`ActivationResult` is a tagged outcome, not a boolean:

| Outcome                   | Meaning                                                              |
| ------------------------- | -------------------------------------------------------------------- |
| `Performed`               | The effect ran.                                                      |
| `Requested { requestId }` | A request was raised for the GM.                                     |
| `Refused { reason }`      | Not permitted — locked, GM-only, or already fired.                   |
| `Unavailable`             | The effect's subsystem is absent (FR-041).                           |
| `NoEffect`                | The interactive carries no effect. Legitimate scenery, not an error. |

**Rules — enforced server-side, per Principle III**

- A locked door refuses a player's activation **here**, not merely in a client
  that declined to draw the button. This is the requirement most likely to be
  implemented only in the UI, and the one where that is least acceptable.
- `gm_only` refuses non-GMs.
- `requires_approval` returns `Requested` and performs nothing.
- A `once` interactive that has fired returns `Refused`.
- Concurrent activation of the same interactive resolves to one outcome
  (SC-005); the second sees the state the first produced.

### Approval — Game Master only

- `approveRequest(requestId)` — runs the effect **now**, re-checking permission
  at decision time. A GM who locked the door after the request was raised has
  contradicted themselves, and the lock wins.
- `refuseRequest(requestId)` — the requester is told (FR-028).

Neither may be called by the requester, including when the requester is the GM
— a GM's own activation does not queue.

### Doors — Game Master only

- `setDoorDesignation(wallId, isDoor)` — designate or undesignate (FR-007).
- `setDoorLock(wallId, locked)` (FR-013).
- `setDoorSecret(wallId, secret)`.

Door _state_ changes go through `activateInteractive` for players, and through
a GM override on the same mutation for a GM — so there is one authorization
path, not two.

## Subscriptions

Interactive and door changes ride the existing `worldEventsCreated`
subscription with their own event codes, as walls, lights, shapes and token
status already do (FR-020). No new transport.

Approval requests reach the GM the same way. A GM on a second device sees the
same queue.

## What is deliberately absent

- No mutation to run an arbitrary effect. Effects run because an interactive
  was activated or a request approved — never because a client asked for one
  directly. Otherwise every server-side permission rule becomes advisory.
- No bulk authoring. Fifty interactives is the scale (SC-007), and a bulk API
  would exist only to be misused.
