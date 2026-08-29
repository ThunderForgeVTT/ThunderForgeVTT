# Contract: Disclosure and Resolved Status (GraphQL)

The server side of the feature: who may set disclosure, and what each viewer
is sent. This is the boundary where entitlement is applied, so it is the
boundary that has to be right.

---

## Principle

**The server resolves; the client renders.** A client is never sent a figure
it may not display (FR-013). Not "sent and hidden" — not sent. A UI that
conceals a field the API still returns is a UI, not a permission.

---

## Query: `tokenStatus`

```graphql
enum DisclosureState {
  VISIBLE
  GREYED
  PERCENTAGE
  CHUNKED
}

type ResourceEntry {
  current: Int!
  max: Int
  label: String
}

type ResolvedResource {
  definitionId: String!
  label: String!
  kind: String! # "bar" | "counter"
  disclosure: DisclosureState!

  "Present only when disclosure is VISIBLE."
  entries: [ResourceEntry!]

  "Present only when disclosure is PERCENTAGE. No maximum is implied or sent."
  proportion: Float

  "Present only when disclosure is CHUNKED. 0-4."
  quarter: Int
}

type TokenStatus {
  tokenId: UUID!
  resources: [ResolvedResource!]!
}

extend type Query {
  tokenStatus(sceneId: UUID!): [TokenStatus!]!
}
```

**Resolution rules**

1. Start from the active system's `ResourceDefinition` set.
2. Load the actor's stored values.
3. Determine the viewer's entitlement — `runs_the_world()` sees `VISIBLE`
   regardless of stored disclosure; everyone else gets the token's stored
   state, or the world default when no row exists.
4. Emit **only** the field matching the state. `CHUNKED` emits `quarter` and
   nothing else; `PERCENTAGE` emits `proportion` and **no maximum**; `GREYED`
   emits neither.

Step 4 is the whole contract. The others are bookkeeping.

**Why `quarter` and not a rounded proportion**: sending a proportion for the
client to round means the proportion was on the client, and a proportion plus
one known damage figure recovers the maximum (FR-013c). The band is computed
server-side and the arithmetic is unit-tested in `thunderforge-canvas-core`
against its boundaries — exactly 25%, exactly zero, a spent top entry.

---

## Mutation: `setTokenDisclosure`

```graphql
input SetTokenDisclosureInput {
  tokenId: UUID!
  resourceId: String!
  state: DisclosureState!
}

extend type Mutation {
  setTokenDisclosure(input: SetTokenDisclosureInput!): TokenStatus!
}
```

**Authorization**: requires `runs_the_world()` via `thunderforge_authz`
(Owner, Game Master, or site admin). No new rule and no parallel check —
Principle III, and the lesson of `deleteWorld` having once accepted any world
member.

**Rules**

- Per **token**, not per actor (FR-013d): two tokens of one creature may
  differ, and the GM sets it on the one in front of the players.
- Setting the world default is idempotent and stores no row; the table is
  sparse.
- The change emits a world event so connected clients update live (FR-009)
  without a reload.
- Returns the mutating GM's own resolved view, which is always `VISIBLE` —
  the response must not be mistaken for what a player will now see.

---

## Event

Disclosure and value changes travel the existing `world_events` path. No new
transport (an assumption recorded in the spec).

A new event code is required for a disclosure change, because a value change
and a change in what may be _known_ about a value are different facts and a
client may need to react differently — a value change updates a bar, a
disclosure change may make one appear or vanish.

---

## Test obligations

- **SC-004 / SC-005a are wire-level assertions**, not screen-level. A test
  must inspect the payload reaching a non-GM client and confirm the exact
  figure is absent — for every state other than `VISIBLE`. Checking the
  rendered UI would pass against a client that received the value and chose
  not to draw it, which is exactly the bug class this guards.
- A GM and a player subscribed to the same scene must receive **different**
  payloads for the same token.
- Changing state mid-session must reach connected clients without a reload.
