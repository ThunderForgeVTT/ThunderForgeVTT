# Phase 1 Data Model: Interactive Elements

Entities, the rules that constrain them, and the state they move through.
Decisions behind these shapes are in [research.md](./research.md).

---

## Effect declaration (code, not storage)

What a subsystem contributes. Lives in `thunderforge-canvas-core`, is compiled
in rather than stored, and is the authority on what a Game Master may author.

| Field           | Meaning                                                                                                                                                                                         |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`            | Stable identifier, namespaced by contributor — `door.set_state`, `light.toggle`. Namespacing is what makes FR-042's collision detection a check on a prefix rather than a coordination problem. |
| `label`         | What a GM is shown when choosing.                                                                                                                                                               |
| `description`   | One line explaining what it does at the table.                                                                                                                                                  |
| `subject_kinds` | Which subjects it may be attached to — a door effect on a door, not on a prop.                                                                                                                  |
| `config`        | What it needs configured, as typed fields the authoring form renders.                                                                                                                           |

**Rules**

- Two contributors MUST NOT declare the same `id` (FR-042). Detected when the
  registry is assembled, which is startup, not first use — a collision that
  surfaces only when a GM happens to author one of them is a collision found at
  the table.
- The registry is the union of what is compiled in. Nothing marks an effect
  unavailable; an absent contributor simply contributes nothing.

---

## Interactive

The authored thing. One row per interactive, scene-scoped.

| Field                                                     | Meaning                                                                                              |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `interactive_id`                                          | Identity.                                                                                            |
| `scene_id`                                                | Owning scene. Interactives do not travel between scenes.                                             |
| `subject_kind`                                            | `prop` \| `door` \| `region`.                                                                        |
| `subject_ref`                                             | The token for a prop, the wall for a door. Null for a region.                                        |
| `geometry`                                                | The bounded area, for a region only. Null otherwise.                                                 |
| `effect_id`                                               | The declared effect, or null — an interactive with no effect is legitimate scenery (US1 scenario 3). |
| `effect_config`                                           | Configuration for that effect, validated against its declaration.                                    |
| `trigger`                                                 | `click` \| `enter`.                                                                                  |
| `activation`                                              | `anyone` \| `gm_only` \| `requires_approval`.                                                        |
| `fire_mode`                                               | `always` \| `once`.                                                                                  |
| `fired_at`                                                | When a `once` interactive fired; null means it has not. Resettable.                                  |
| `created_by` / `updated_by` / `created_at` / `updated_at` | Provenance, per Principle III.                                                                       |

**Rules**

- Exactly one of `subject_ref` and `geometry` is populated, decided by
  `subject_kind`. A region with a `subject_ref`, or a door without one, is
  invalid rather than tolerated.
- `trigger = enter` is only valid for `subject_kind = region`. A book cannot be
  crossed.
- `effect_id` MUST exist in the registry **at authoring time**. It may cease to
  exist later, which is FR-041's case and is a display state, not a repair.
- `effect_config` MUST validate against the declaration's `config` at
  authoring time.
- Deleting the subject deletes the interactive. A door on a deleted wall is
  not a thing.

---

## Door

Not a new entity — two columns added to the existing wall.

| Field        | Meaning                                                        |
| ------------ | -------------------------------------------------------------- |
| `door_state` | Existing. `none` (not a door) \| `open` \| `closed`.           |
| `locked`     | New. Governs _who may change the state_, not the state itself. |
| `secret`     | New. Not presented to players until revealed.                  |

**The blocking rule** — the definition FR-008/FR-009 asked for:

| State    | Blocks vision                  | Blocks movement                  |
| -------- | ------------------------------ | -------------------------------- |
| `open`   | No                             | No                               |
| `closed` | The wall's own `blocks_vision` | The wall's own `blocks_movement` |
| `none`   | The wall's own `blocks_vision` | The wall's own `blocks_movement` |

A closed door is therefore indistinguishable from a plain wall in what it
blocks, which is correct: the difference is that it can be opened. A closed
window that blocks movement but not vision keeps being see-through.

**State transitions**

```text
closed ──(anyone, if unlocked)──> open
open   ──(anyone, if unlocked)──> closed
any    ──(GM only)──────────────> locked / unlocked
any    ──(GM only, or a reveal effect)──> secret / revealed
```

- A locked door refuses player state changes and accepts the GM's (FR-013).
- Revealing is one-way here. Re-hiding a door the table has seen is a fiction
  problem, not a state problem, and no scenario asks for it.
- `secret` affects presentation only. Per the spec's decision, secret geometry
  reaches clients that do not draw it.

---

## Prop

A row in `tokens` with kind `object` and no actor. No new storage.

**Rules**

- Takes no turn, appears in no initiative or party list, has no sheet.
- Anything consuming tokens must tolerate a null actor. Spec 029's
  `tokenStatus` already does, treating actorless tokens as markers rather than
  creatures.

---

## Region

Geometry carried on the interactive, not a shape.

**Rules**

- Invisible to players always. A region is not an annotation.
- Entry fires once per crossing, not continuously while inside (FR-030). The
  engine compares previous and current containment.
- With `fire_mode = once`, the first entry sets `fired_at` and later entries by
  anyone do nothing until a GM resets it.
- Movement performed while the scene is being prepared does not fire anything
  (FR-032).
- Overlapping regions both fire, in `interactive_id` order — arbitrary but
  stable, which is what the edge case needs. Undefined order would make a
  double-region trigger unreproducible.

---

## Approval request

A player's activation awaiting a decision. Session-scoped, pruned rather than
retained.

| Field                                      | Meaning                                  |
| ------------------------------------------ | ---------------------------------------- |
| `request_id`                               | Identity.                                |
| `interactive_id`                           | What was activated.                      |
| `requested_by`                             | Which player.                            |
| `scene_id`                                 | For routing to the right GM view.        |
| `state`                                    | `pending` \| `approved` \| `refused`.    |
| `created_at` / `decided_at` / `decided_by` | Provenance and audit within the session. |

**Rules**

- A request MUST NOT expire into approval (FR-027). Nothing may time it out
  into running.
- Only a GM of the world may decide it.
- Approving runs the effect **then**, with the permission it had at decision
  time — not the permission it had when asked. A GM who locks a door and then
  approves a queued request to open it has contradicted themselves, and the
  lock wins.
- A request whose requester has left is cancelled rather than left pending.
- A request whose interactive was deleted is cancelled.

---

## Entity relationships

```text
scene ──1:N── interactive ──0:1── token   (prop)
                          ──0:1── wall    (door)
                          ──0:1── geometry (region, inline)
                          ──0:N── approval request

wall ──has── door_state, locked, secret
```

---

## What changes in existing tables

| Table    | Change                                      | Why                                       |
| -------- | ------------------------------------------- | ----------------------------------------- |
| `walls`  | Add `locked BOOLEAN NOT NULL DEFAULT false` | FR-010                                    |
| `walls`  | Add `secret BOOLEAN NOT NULL DEFAULT false` | US4                                       |
| `tokens` | None                                        | Props reuse `object` kind and null actor  |
| `shapes` | None                                        | Regions are not annotations — research §4 |

Both wall columns default to false, so every existing wall stays exactly what
it is today.
