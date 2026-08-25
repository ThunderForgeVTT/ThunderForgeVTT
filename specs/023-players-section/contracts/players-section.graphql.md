# Contract: Players Section GraphQL Surface

Extends the existing `worldMembers` query and reuses the existing `updateMemberRole`/`removeMember` mutations unchanged (behavior fix only, see research.md §3). No new types, no new mutations.

## Query changes

### `worldMembers(worldId: ID!): [WorldMembershipPayload!]!` — additive field

```graphql
type WorldMembershipPayload {
  # ...existing fields unchanged (id, userId, role, joinedAt)
  claimedActor: GraphQLWorldActor   # null when the member hasn't claimed a character
}
```

Authorization unchanged — any world member may call this (already the case today).

## Mutations — reused as-is, one internal fix

### `updateMemberRole(input: UpdateMemberRoleInput!): WorldMembershipPayload!`

No input/output change. Internal fix: caller-identity lookup now goes through `require_world_member` (Owner-fallback) instead of a raw row fetch — see research.md §3. Authorization semantics (`can_change_roles`/`can_manage`, valid `role` values) unchanged.

### `removeMember(worldId: ID!, userId: ID!): Boolean!`

No input/output change. Same internal fix as above. Self-removal and Owner-removal remain rejected exactly as they are today.

## Access control summary

| Action | Caller requirement |
|---|---|
| `worldMembers` (incl. new `claimedActor` field) | Any world member (unchanged) |
| `updateMemberRole` | GM/Owner with sufficient role-hierarchy standing over the target (unchanged authorization, fixed caller lookup) |
| `removeMember` | GM/Owner with sufficient role-hierarchy standing over the target; never self, never the Owner (unchanged authorization, fixed caller lookup) |

## Frontend consumption

`PlayersPage.tsx` calls `getWorldMembers(worldId)` (extended to include `claimedActor`) for User Story 1 (everyone). For User Story 2, GM/Owner callers additionally see role-`<select>` and Remove controls per row, wired to the new `updateMemberRole`/`removeMember` wrapper functions in `apps/web/src/api/worldMembers.ts` (promoted out of `CampaignSettingsPanel.tsx`'s inline-only calls, per data-model.md).
