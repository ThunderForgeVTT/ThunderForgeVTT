# Contract: The Effect Contribution Seam

What a subsystem must provide to become triggerable, and what it may assume.
This is the contract FR-036 through FR-043 describe, and the one US7 tests.

## What a contributor declares

A subsystem contributes a set of effect declarations. Each carries:

- **`id`** — stable, namespaced by contributor (`door.set_state`,
  `light.toggle`, `nav.request_scene`). The namespace is what makes collision
  detection (FR-042) a prefix check rather than a coordination problem.
- **`label`** — what a Game Master sees when choosing.
- **`description`** — one line, in a GM's language, not an engineer's.
- **`subject_kinds`** — which subjects it may attach to (`prop`, `door`,
  `region`).
- **`config`** — typed fields the authoring form renders and the server
  validates.

The declaration is data. It carries no behaviour, because the three consumers
need it in three places: the engine dispatches against it, the server validates
against it, and the web app builds a form from it.

## What a contributor implements

One system, in its own plugin, that reads activation events and acts on the
ones bearing its identifiers. Nothing more.

## What the interaction feature guarantees

- An effect is only dispatched after permission has been resolved. A
  contributor never has to ask whether the actor was allowed.
- An effect is only dispatched with configuration that validated against its
  own declaration.
- A `requires_approval` interactive dispatches only after a GM approved it.
- A `once` interactive dispatches at most once until reset.

## What the interaction feature must never do

- Reference a specific effect, subject type or subsystem in its own logic
  (FR-039). The test for this is textual as well as behavioural: the words
  "light", "door" and "sound" do not appear in it.
- Call into a contributor, or be called by one (FR-040).
- Fail, or degrade, when a contributor is absent.

## Collision and absence

- **Collision**: two contributors declaring the same `id` is detected when the
  registry is assembled — at startup, not at first use. A collision found when
  a Game Master happens to author one of them is a collision found at the
  table.
- **Absence**: an authored interactive whose `effect_id` is not in the current
  registry is _unavailable_. It is shown as such to the GM, is not dispatched,
  is not deleted, and is not surfaced to players as an error (FR-041).
  Detection happens by comparing against the registry, never by observing that
  dispatch did nothing — an event is fire-and-forget and cannot report that
  nobody listened.

## Effects contributed by this feature

| Contributor | `id`                | Subjects           | Config                                                |
| ----------- | ------------------- | ------------------ | ----------------------------------------------------- |
| Doors       | `door.set_state`    | prop, door, region | target wall, desired state (`open`/`closed`/`toggle`) |
| Doors       | `door.set_lock`     | prop, door         | target wall, locked or not                            |
| Doors       | `door.reveal`       | prop, door, region | target wall                                           |
| Lighting    | `light.toggle`      | prop, door, region | one or more lights                                    |
| Lore        | `lore.open`         | prop, door, region | a lore entry in this world                            |
| Navigation  | `nav.request_scene` | prop, region       | destination scene                                     |

`nav.request_scene` raises a request and, on approval, does nothing further
until multi-scene navigation exists — which is honest rather than dead: the
request and the decision are the parts this feature owns, and they work.

## Effects deliberately not contributed

**Sound.** No audio subsystem exists, so nothing declares a sound effect and
none is offered. When audio is built it contributes `audio.play` and this
document gains a row. Nothing else changes, which is the point of the seam.
