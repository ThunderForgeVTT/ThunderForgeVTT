# Phase 1 Data Model: In-Engine Status Displays

Entities, their fields, and the rules that constrain them. Derived from
[spec.md](./spec.md); decisions behind the placements are in
[research.md](./research.md).

---

## ResourceDefinition

What a game system declares. Ships in the system package's manifest; the
engine never contains one.

| Field           | Type               | Notes                                                     |
| --------------- | ------------------ | --------------------------------------------------------- |
| `id`            | string             | Stable identifier, e.g. `health`. Unique within a system. |
| `label`         | string             | What a person reads, e.g. "Hit Points".                   |
| `kind`          | `bar` \| `counter` | A bar has a maximum; a counter does not.                  |
| `order`         | integer            | Display order. The engine imposes none (FR-003).          |
| `allowStacking` | boolean            | Whether more than one entry is permitted (FR-002b).       |

**Rules**

- `id` MUST be unique within a system.
- A `counter` MUST NOT declare `allowStacking: true` — stacking describes
  layered pools with maxima, and a counter has none.
- A definition removed from a system MUST cause its values to stop being
  displayed even where stored data remains (FR-005).

---

## ResourceEntry

One layer of a resource. The unit that makes overflow unrepresentable.

| Field     | Type    | Notes                               |
| --------- | ------- | ----------------------------------- |
| `current` | integer |                                     |
| `max`     | integer | Absent for a `counter`.             |
| `label`   | string? | Optional, e.g. "Shield", "Stage 2". |

**Rules**

- `0 <= current <= max` for a bar. A value outside that is a **defect to
  report, not a state to render** (FR-002d) — there is no clamping rule
  because there is no case to clamp.
- Entries are **ordered**; index 0 is the base pool and later entries stack
  above it.
- Depletion consumes the **highest** index first (FR-002c).
- An entry at zero **remains in the list**. A boss on its last stage must
  still read as being on its last stage, which requires the spent ones to be
  visible.
- A resource with `allowStacking: false` MUST have exactly one entry; a second
  is rejected rather than merged (FR-002b), because merging loses which pool
  was temporary.

---

## DisclosureState

What a viewer other than the Game Master is permitted to learn. Set per token
per resource; the GM always sees the true value regardless.

| Value        | What reaches the client                             |
| ------------ | --------------------------------------------------- |
| `visible`    | Full entry list, exact.                             |
| `greyed`     | Presence only. No values, no maxima, no proportion. |
| `percentage` | A proportion. **No maximum.**                       |
| `chunked`    | A quarter index, 0–4. No proportion, no totals.     |

**Rules**

- Coarsening happens **server-side** (FR-013). The client is never sent a
  figure it may not display.
- `chunked` MUST arrive as an index, not a percentage the client rounds
  (FR-013b) — rounding on the client means the exact figure was on the client.
- `greyed` MUST be distinguishable from "at zero" (FR-008). Absence of
  knowledge and knowledge of absence are different facts.
- `percentage` leaks more than it appears to (FR-013c): a viewer who knows the
  damage they dealt can divide it by the change, recover the maximum, and read
  exact values from then on. It is offered, not withdrawn — but the four
  states are not equally safe and the type should not imply they are.

---

## TokenResourceDisclosure _(persisted, new)_

Table: `token_resource_disclosure`

| Column                      | Type      | Notes                                  |
| --------------------------- | --------- | -------------------------------------- |
| `id`                        | uuid      | PK                                     |
| `token_id`                  | uuid      | FK → `tokens.token_id`, cascade delete |
| `resource_id`               | varchar   | The `ResourceDefinition.id`            |
| `state`                     | varchar   | One of the four above                  |
| `created_by`                | uuid      | Provenance, per Principle III          |
| `updated_by`                | uuid      |                                        |
| `created_at` / `updated_at` | timestamp |                                        |

**Rules**

- Unique on `(token_id, resource_id)`.
- **Sparse**: absence means the world default. Most tokens store no row.
- Writing requires `runs_the_world()` via `thunderforge_authz` — no new
  authorization rule (Principle III).
- Deleting a token deletes its rows; deleting a _resource definition_ does
  not, because the definition may return.

---

## TokenStatus _(resolved, not persisted)_

What the engine actually draws for one token — the product of the system's
declaration, the stored values, and the viewer's entitlement.

| Field       | Type                 | Notes                 |
| ----------- | -------------------- | --------------------- |
| `tokenId`   | string               |                       |
| `resources` | `ResolvedResource[]` | In declaration order. |

### ResolvedResource

| Field          | Type               | Notes                                     |
| -------------- | ------------------ | ----------------------------------------- |
| `definitionId` | string             |                                           |
| `label`        | string             |                                           |
| `kind`         | `bar` \| `counter` |                                           |
| `disclosure`   | DisclosureState    | How this was resolved.                    |
| `entries`      | `ResourceEntry[]`? | Present only when `visible`.              |
| `proportion`   | number?            | Present only when `percentage` (0.0–1.0). |
| `quarter`      | integer?           | Present only when `chunked` (0–4).        |

**Rules**

- Exactly one of `entries` / `proportion` / `quarter` is present, matching
  `disclosure`; `greyed` carries none of them. The shape makes an
  over-disclosing payload unrepresentable rather than merely forbidden.
- A token whose actor has no displayable resources yields an **empty**
  `resources` list, and the engine draws no furniture at all (FR-007) — not an
  empty container.

---

## PanelPlacement _(per viewer)_

| Field    | Type                                                         | Notes |
| -------- | ------------------------------------------------------------ | ----- |
| `corner` | `top-left` \| `top-right` \| `bottom-left` \| `bottom-right` |       |

**Rules**

- Persists across reloads (FR-011). Per viewer, per device; it does not need
  to follow someone to another machine.
- With no selection the panel MUST NOT show a previous token's values
  (FR-012).

---

## DisplayAppearance _(application-supplied)_

Colours, sizes and spacing, passed in by the application rather than compiled
into the engine (FR-022), so a later theming feature has something to
configure.

**Rules**

- A documented default set exists in exactly **one** place (FR-023).
- Any default palette meets the separation standard already applied to token
  kinds: distinguishable in perceived lightness, not hue alone (FR-024) —
  a red bar and a green bar are the same bar to a viewer with a red-green
  deficiency.

---

## Relationships

```text
GameSystem ──declares──> ResourceDefinition
                              │
Actor ──has values for────────┘
  │
  └──bound to──> Token ──has──> TokenResourceDisclosure (sparse, per resource)
                   │
                   └── resolved per viewer ──> TokenStatus ──> drawn by engine
                                                          └──> read by React panel
```

The resolution step is where entitlement is applied, and it happens on the
server. Everything downstream of it — engine and React alike — receives a
payload that already contains only what the viewer may see.

---

## Where the rules live

| Rule                            | Home                              | Why                               |
| ------------------------------- | --------------------------------- | --------------------------------- |
| Entry ordering, depletion order | `thunderforge-canvas-core`        | Pure; its tests execute           |
| Quarter banding arithmetic      | `thunderforge-canvas-core`        | Boundary cases need real tests    |
| Disclosure application          | Server, using canvas-core helpers | Must not be client-side           |
| Drawing bars on tokens          | Engine plugin                     | Spatial (Principle I)             |
| The corner panel                | React                             | Screen-space chrome (Principle I) |

The engine crate is deliberately not the home for any _rule_: its tests
compile and never run, so logic placed there is untested by construction.
