# Contract: Abilities GraphQL surface

Core ability CRUD, effects, and permissions. Mirrors
`specs/013-items-inventory/contracts/graphql-items.md`.

**Argument-shape rule** (research.md §5): mutations that create/update a whole
entity take a single `input:` object; everything else takes flat scalar args.
Write the resolver first, then match the frontend query string to it.

## Types

```graphql
enum AbilityClassification { SPELL FEAT POWER TALENT }
enum AbilityEffectType { HEAL DAMAGE MODIFIER ATTACK_ROLL }
enum AbilityEffectTrigger { ON_USE PASSIVE }

type GraphQLAbilityEffect {
  id: UUID!
  abilityId: UUID!
  effectType: AbilityEffectType!
  formula: String!
  target: String!
  triggerKind: AbilityEffectTrigger
  sortOrder: Int!
}

type GraphQLAbility {
  id: UUID!
  worldId: UUID!
  name: String!
  description: String
  classification: AbilityClassification!
  gmOnly: Boolean!                           # visibility; only ever true in a DM's response
  effects: [GraphQLAbilityEffect!]!
  myPermissionLevel: ActorPermissionLevel!   # reused enum: VIEWER | EDITOR | OWNER — edit rights only
  moderated: Boolean!
  moderationCaseId: UUID
  createdAt: String!
  updatedAt: String!
  linkedFromLore: [GraphQLLoreEntry!]!       # ComplexObject field
}

type GraphQLAbilityPermission {
  abilityId: UUID!
  userId: UUID!
  level: ActorPermissionLevel!
  updatedAt: String!
}
```

`linkedFromLore` matches `GraphQLItem`'s naming (the newer of the two
conventions — actors use the older `loreLinkedFrom`).

`GraphQLAbility` needs both constructors the item type has: `from_row(row,
effects, my_permission_level)` and `moderated_placeholder(id, world_id,
my_permission_level, case_id)`.

## Inputs

```graphql
input CreateAbilityInput {
  worldId: UUID!
  name: String!
  description: String
  classification: AbilityClassification!
  gmOnly: Boolean                    # optional, defaults false (FR-024a)
}

input UpdateAbilityInput {
  abilityId: UUID!
  name: String
  description: String
  classification: AbilityClassification
  clearDescription: Boolean          # see note
  # NOTE: gmOnly is deliberately ABSENT here — see setAbilityGmOnly below
}

input AbilityEffectInput {
  effectType: AbilityEffectType!
  formula: String!
  target: String!
  triggerKind: AbilityEffectTrigger
  sortOrder: Int
}

input SetAbilityPermissionInput {
  abilityId: UUID!
  userId: UUID!
  level: ActorPermissionLevel!
}
```

**`clearDescription` note** (research.md §3, defect 1): the item version applies
`description.or(existing.description)`, making a null description
indistinguishable from "field omitted" — so a description can never be cleared
once set. The explicit boolean is the minimal fix that keeps the rest of the
partial-update semantics intact. Do not copy the item behavior.

## Queries

| Query | Args | Returns | Authorization |
|---|---|---|---|
| `worldAbilities` | `worldId: UUID!, search: String` | `[GraphQLAbility!]!` | `require_visible_world`; **`gm_only = false` unless caller is DM**; then `moderation::filter_visible(state, "world_ability", …)` |
| `ability` | `abilityId: UUID!` | `GraphQLAbility!` | load row → `require_visible_world(row.world_id)`; **reject if `gm_only` and caller is not DM (FR-025)**; if `moderation::effective_status` is `Some`, return `moderated_placeholder` |
| `suggestAbilityName` | `worldId: UUID!, name: String!` | `[GraphQLAbility!]!` | `require_visible_world`; **`gm_only = false` unless caller is DM**; `similarity(name, $1) > 0.4`, ordered desc, `LIMIT 5` |
| `abilityPermissions` | `abilityId: UUID!` | `[GraphQLAbilityPermission!]!` | `require_dm_of_abilitys_world` |

### GM-only filtering is a security boundary, not a UI convenience

Every non-DM read path above must filter `gm_only`. A miss is a content leak.
data-model.md carries the complete surface table — including the paths owned by
other contracts (`actorAbilities`, lore link candidates, lore link resolution).

The rejection for `ability` MUST NOT distinguish "GM-only" from "does not exist";
otherwise a non-DM can probe for the existence of hidden abilities by id.

`worldAbilities`' `search` is a server-side `ILIKE` on name/description with
`%`/`_` escaped, ordered `name ASC` — matching `worldItems`.

`suggestAbilityName` is advisory only and MUST NOT gate `createAbility` (FR-006,
FR-007).

## Mutations

| Mutation | Args | Returns | Authorization |
|---|---|---|---|
| `createAbility` | `input: CreateAbilityInput!` | `GraphQLAbility!` | `is_dm_of_world(input.world_id)`; else "Only the DM (Owner or GM) may create abilities" |
| `updateAbility` | `input: UpdateAbilityInput!` | `GraphQLAbility!` | `require_ability_permission(ability_id, Editor)` |
| `deleteAbility` | `abilityId: UUID!` | `Boolean!` | `require_ability_permission(ability_id, Owner)` |
| `addAbilityEffect` | `abilityId: UUID!, effect: AbilityEffectInput!` | `GraphQLAbilityEffect!` | `require_ability_permission(Editor)`, then `validate_formula` + `validate_target` |
| `updateAbilityEffect` | `effectId: UUID!, effect: AbilityEffectInput!` | `GraphQLAbilityEffect!` | validate; resolve parent ability; `require_ability_permission(Editor)` |
| `removeAbilityEffect` | `effectId: UUID!` | `Boolean!` | resolve parent ability; `require_ability_permission(Editor)` |
| `setAbilityPermission` | `input: SetAbilityPermissionInput!` | `GraphQLAbilityPermission!` | `require_dm_of_abilitys_world`; UPSERT on `(ability_id, user_id)` |
| `removeAbilityPermission` | `abilityId: UUID!, userId: UUID!` | `Boolean!` | `require_dm_of_abilitys_world`; idempotent DELETE (resets to implicit Viewer) |
| `setAbilityGmOnly` | `abilityId: UUID!, gmOnly: Boolean!` | `GraphQLAbility!` | `require_dm_of_abilitys_world` — **DM-only, Owner-level on the ability is not sufficient (FR-024c)** |

### Why `gmOnly` gets its own mutation

It is deliberately **not** a field on `UpdateAbilityInput`. `updateAbility`
requires only `Editor`, so folding visibility into it would let any Editor
un-hide a GM's secret ability. A separate DM-gated mutation keeps the two
authority levels from being conflated, and follows the existing
`updateSceneHidden` precedent (`graphql.rs`, guarded by
`update_scene_hidden_requires_dm_role`).

## Authorization module

New `src/server/src/auth/ability_permissions.rs`, a near-verbatim copy of
`auth/item_permissions.rs` with the table names swapped, registered in
`auth/mod.rs`:

```rust
pub async fn effective_ability_permission(
    state: &AppState, user_id: Uuid, is_admin: bool, ability_id: Uuid,
) -> GraphQLResult<ActorPermissionLevel>

pub async fn require_ability_permission(
    state: &AppState, user_id: Uuid, is_admin: bool, ability_id: Uuid,
    minimum: ActorPermissionLevel,
) -> GraphQLResult<()>
```

Resolution order (unchanged from the item version):

1. Look up `world_abilities.world_id`; `None` → error "Ability not found".
2. `is_dm_of_world(...)` → early-return `Owner` (FR-024's DM-always-full-control;
   no permission row can downgrade a DM).
3. Otherwise read the caller's `world_ability_permissions.level`.
4. `.unwrap_or(Viewer)` — the default-Viewer rule, also covering an unparseable
   DB string.

`require_ability_permission` compares `rank()` and rejects with
`FORBIDDEN`-coded "You do not have sufficient permission on this ability".

**Permission levels reuse the existing `ActorPermissionLevel`** — do not define a
fourth copy of Viewer/Editor/Owner.

## Validation

`validate_formula` / `validate_target` are copied from `mutations_items.rs`
(structural only — non-empty, ≥1 alphanumeric; never ruleset-aware, FR-019):

- called in `add_ability_effect_impl` and `update_ability_effect_impl`
- **also called on the share-copy path**, unlike the item version
  (research.md §3, defect 4)

## Registration checklist

Every hook point, mirroring items exactly:

1. `schema.rs` — 4 `table!` blocks, `joinable!` lines, `allow_tables_to_appear_in_same_query!` entries
2. `models.rs` — row + `New…` structs (field order must match `schema.rs`)
3. `auth/mod.rs` — `pub mod ability_permissions;`
4. `graphql/queries/mod.rs` — `pub mod ability;` + re-export
5. `graphql.rs` — module declarations + re-exports
6. `graphql.rs` — `QueryRoot` merged object
7. `graphql.rs` — `MutationRoot` merged object
8. `graphql/types.rs` — the types above + `ModerationEntityType::WorldAbility ↔ "world_ability"`
9. `graphql/mutations_moderation.rs` — owner/world lookup match arm
