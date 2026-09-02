# Contract: appearance over the wire

Three additions and one correction. All of them mirror something that already
exists, deliberately — a world's interface pack is the same kind of fact as a
world's game system, and the two should not be reached for differently.

## Query — the packs available

```
GET /interface-packs
GET /interface-packs/{id}/manifest.json
```

REST, not GraphQL, mirroring `src/server/src/systems.rs` exactly. A manifest is
a static document read from disk and served whole; routing it through the graph
would gain nothing and would put a JSON blob in a typed schema.

`GET /interface-packs` returns id, title, version, and description for every
pack in `packs/interface/`, sorted by title, with **no special position for
Forge** (FR-007, US1 scenario 6). The manifest route validates before serving,
the way `get_system_manifest` does for `system.json`: a pack that has drifted
out of compliance fails closed rather than reaching a browser.

## Query — what this world uses

`GraphQLWorld.interfacePackId` already exists and is already returned. No
change. It has been carried end-to-end since the phase-3 world-metadata
migration and read by nothing; this feature is its first consumer.

## Mutation — setting it

```graphql
input UpdateWorldInterfacePackInput {
  worldId: UUID!
  interfacePackId: String   # null clears the binding, returning the world to Forge
}

type Mutation {
  updateWorldInterfacePack(input: UpdateWorldInterfacePackInput!): World!
}
```

Authorization: `is_dm_of_world`, refusing with *"Only the DM (Owner or GM) may
change a world's interface pack"* — the wording and the check both mirroring
`update_world_game_system_impl` (FR-010). A player attempting it is refused by
the same rule that refuses them a system change, which is the point: this is a
world setting, and world settings have one authority.

Validation on write: the id must name a pack that exists and validates.
Accepting an id for a pack that is not installed would create FR-019's degraded
state on purpose, from the one place that knows better.

Unlike `updateWorldGameSystem`, `interfacePackId` is **nullable** on the way in.
Clearing it is a real thing a Game Master may want — it means "the default",
and after this feature the default has a name to show for it (FR-023).

## Event — telling everyone else

```
EVENT_CODE_WORLD_APPEARANCE_CHANGED = 23

payload: { "action": "changed", "interfacePackId": "<id>" | null }
```

Recorded by the mutation through `record_world_event`, exactly as
`EVENT_CODE_WALL_CHANGED` and its siblings are. Every client in the world
re-resolves its appearance on receipt and applies it without a reload (SC-001).
A client that was offline when it changed picks it up through the spec 028
catch-up, at no additional cost.

## Correction — one wording for the unset state

Not an addition, but part of this contract because it is what the field means
to a reader (FR-022, FR-023, SC-008).

| Surface | Today | After |
|---|---|---|
| `WorldCard.tsx` | `"Unbound placeholder"` | The active pack's title — `"Forge"` when unset |
| `WorldDashboardPage.tsx` | `"Not yet assigned"` | The same |

There is no unset state to have wording *for*. A world with no
`interface_pack_id` is drawn in Forge, so a screen saying "Not yet assigned" is
describing a state the product does not have. SC-008 asks for zero distinct
strings for the unset state, down from two; the way to reach zero is to stop
having one, not to agree on which lie to tell.

The same correction is **not** made to the `gameSystemId` labels on those two
screens. There the unset state is real — a world with no system is a world that
genuinely has not been bound — and it belongs to User Story 2's half of the
spec, which this increment does not build.
