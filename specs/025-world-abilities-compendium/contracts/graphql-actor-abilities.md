# Contract: Actor known-abilities GraphQL surface

"This NPC knows Fireball" (FR-021..FR-023). Mirrors
`specs/013-items-inventory/contracts/graphql-inventory.md`, minus quantity.

## Types

```graphql
type GraphQLActorAbilityEntry {
  id: UUID!
  actorId: UUID!
  abilityId: UUID          # null when the source ability was deleted
  abilityName: String!     # server-side snapshot, always present
  classification: AbilityClassification   # null when the ability was deleted
}

input AttachAbilityToActorInput {
  actorId: UUID!
  abilityId: UUID!
}
```

`abilityId: null` + a retained `abilityName` is the tombstone state — the UI
renders *"Fireball (deleted ability)"* rather than dropping the row. Directly
mirrors `InventoryEntryRecord`'s `itemId: string | null` + `itemName`.

`classification` is denormalized onto the entry for display convenience and is
null for a tombstoned row (there is no ability left to read it from).

## Queries

| Query | Args | Returns | Authorization |
|---|---|---|---|
| `actorAbilities` | `actorId: UUID!` | `[GraphQLActorAbilityEntry!]!` | `require_actor_permission(actor_id, Viewer)`; **then filter out entries whose ability is `gm_only` unless the caller is a DM of that world (FR-023, FR-024b)** |

### GM-only entries are filtered silently

A non-DM's list simply omits GM-only abilities. It MUST NOT include a redacted
placeholder, a "N hidden" count, or a gap in ordering — a player must not be able
to infer that anything was withheld (FR-023, Clarification Session 2026-08-25).

### Tombstones are redacted for non-DMs (FR-023a)

An entry whose ability was **deleted** keeps its `ability_name_snapshot` and
stays listed, but a tombstone carries no `gm_only` flag to consult — so the
system cannot tell whether the deleted ability was secret.

It therefore **fails closed**: `actorAbilities` returns `ability_name:
"REDACTED"` for every tombstone to a non-DM caller, secret or not. A DM still
receives the real snapshot.

The cost is that a player also stops seeing the name of an ordinary deleted
ability. That is the accepted trade: a deleted ability's name is of little use
to a player, and the alternative leaks exactly the names the GM-only flag
exists to protect. Redaction happens **server-side** — a UI-only treatment
would still ship the name over the wire.

## Mutations

| Mutation | Args | Returns | Authorization |
|---|---|---|---|
| `attachAbilityToActor` | `input: AttachAbilityToActorInput!` | `GraphQLActorAbilityEntry!` | `require_actor_permission(input.actor_id, Editor)` |
| `detachAbilityFromActor` | `entryId: UUID!` | `Boolean!` | resolve entry → actor; `require_actor_permission(actor_id, Editor)` |

## The critical authorization rule (FR-022)

**Permission is checked against the ACTOR, never the ability.**

A user with Editor on an actor may attach any ability in that world to them,
even one they only have Viewer access to. Conversely, Owner-level access to an
ability grants no right to attach it to an actor the user cannot edit.

This is not an oversight — it is spec 013's explicit rule for inventory
(its FR-013), and it is what makes "the GM equips an NPC with a spell the
players can't read yet" work. The item precedent has a dedicated regression
test, `inventory_permission_follows_actor_not_item`; abilities need its
analogue, `actor_ability_permission_follows_actor_not_ability`.

## Cross-world guard

`attachAbilityToActor` MUST reject an ability whose `world_id` differs from the
actor's `world_id`. The `UNIQUE (actor_id, ability_id)` constraint does not
enforce this, and neither FK does — it needs an explicit check, returning a
clear error rather than creating a cross-world reference.

## De-duplication (FR-021)

Re-attaching an already-known ability is a **no-op returning the existing
entry**, not an error and not a duplicate row. Enforced by
`UNIQUE (actor_id, ability_id)` plus an `ON CONFLICT DO NOTHING` /
select-existing path.

Mirrors `adding_same_item_twice_merges_quantity`, except there is no quantity to
merge — the second attach simply returns what is already there.

⚠️ Postgres treats NULLs as distinct in a UNIQUE constraint, so multiple
tombstoned rows (`ability_id IS NULL`) per actor are permitted. This is correct:
two different deleted abilities must both remain listed.

## Deletion behavior (FR-023)

Deleting an ability MUST NOT be blocked by actors knowing it.
`world_actor_abilities.ability_id` is `ON DELETE SET NULL`, so:

- the delete succeeds
- the entry row survives with `ability_id = NULL`
- `ability_name_snapshot` keeps the row identifiable
- `actorAbilities` still returns it, marked as referencing a deleted ability

Detaching an ability from an actor deletes only the entry row — the ability
itself is untouched (FR-023, spec US3 scenario 6).

## Frontend

New `apps/web/src/pages/world/actor/ActorAbilitiesPanel.tsx`, mirroring
`ActorInventoryPanel.tsx`:

```ts
export interface ActorAbilitiesPanelProps {
  actorId: string;
  worldId: string;
  /** Editor/Owner on the ACTOR gates attach/detach — NOT the caller's
   *  permission on any given ability. */
  canManage: boolean;
}
```

Behavior copied from the inventory panel:

- loads `getActorAbilities(actorId)` on mount/actor change
- loads the world ability catalog **only when `canManage`** (a viewer never
  fetches the catalog)
- non-optimistic `refresh()` after every mutation
- the list is always visible; only the attach/detach controls are gated
- tombstoned rows render with an italic "(deleted ability)" marker

Wired into `ActorDetailPage.tsx` beside `ActorInventoryPanel`, receiving the
same `canManage={canEdit}` value (the actor's own `myPermissionLevel !==
"VIEWER"`), and — matching inventory — available from the view route, not only
`mode === "edit"`.

Classification labels in this panel go through `resolveAbilityLabel` (FR-012);
see `ability-facets.md`.

## Test expectations

- `actor_ability_permission_follows_actor_not_ability` — Editor on the actor +
  Viewer on the ability succeeds; Owner on the ability + Viewer on the actor is
  rejected.
- `attaching_same_ability_twice_is_a_noop` — one row, no error.
- `attaching_cross_world_ability_is_rejected`.
- `deleting_an_ability_tombstones_actor_entries_instead_of_blocking` — delete
  succeeds; entry survives with null `ability_id` and an intact name snapshot.
- `detaching_does_not_delete_the_ability`.
- `viewer_on_actor_can_read_but_not_modify_known_abilities`.
- `gm_only_abilities_are_omitted_from_a_non_dms_known_list` — a DM sees the
  entry, a Viewer-on-actor player does not, and nothing in the player's response
  hints that an entry was filtered.
- `tombstoned_ability_names_are_redacted_for_non_dms` — after deleting both a
  GM-only and an ordinary ability, a player sees two tombstones both reading
  `REDACTED`; the DM still sees both real names.
- `attach_catalog_excludes_gm_only_for_non_dm` — the catalog offered when
  attaching comes from `worldAbilities` and inherits its filter.
