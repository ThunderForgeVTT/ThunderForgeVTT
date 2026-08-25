# Data Model: Players Section

No migrations — every table this feature reads/writes already exists. This is additive query surface plus a caller-lookup fix in two existing mutations.

## World Member (existing `world_members`, existing `worldMembers` query — additive field only)

| Field | Type | Notes |
|---|---|---|
| `id`, `userId`, `role`, `joinedAt` | — | existing, unchanged |
| **`claimedActor`** | `WorldActor`, nullable | **NEW.** Resolved by joining `world_actor_claims` on `world_member_id = member.id`, then `world_actors` for the claimed actor's `id`/`label` (the existing "name" field on `world_actors`, see `claimedBy`'s reverse resolver for the exact shape to mirror). `null` when the member hasn't claimed a character (FR-005). |

**Validation rules**: none new — `claimedActor` is read-only, derived from data this feature doesn't write to.

**Lifecycle**: Set/cleared entirely by the existing Actor Selection flow (`claimActor`/`createAndClaimActor`/GM un-claim mutations, spec 017) — this feature only reads the current state, per the Clarifications-adjacent Assumption that character-claiming rules are out of scope here.

## Role-change / removal (existing `updateMemberRole`/`removeMember` mutations — behavior fix only)

No new fields. The one change: both mutations' caller-identity lookup switches from a raw `world_members` row fetch to `require_world_member` (the same Owner-fallback helper `is_dm_of_world` and `world_members_impl` already use), so a world's Owner without a backfilled `world_members` row is correctly recognized as the Owner rather than rejected. The role-hierarchy authorization check itself (`can_change_roles`/`can_manage`, self-removal rejection) is unchanged.

**Validation rules** (unchanged, restated for completeness):
- `updateMemberRole`: caller must satisfy `can_change_roles()` and `can_manage(target_role)`; `role` must be `"Owner" | "GM" | "Player"`.
- `removeMember`: caller must be `Owner` or `GM`, must satisfy `can_manage(target_role)`, and cannot remove themselves (existing explicit check) — the role hierarchy itself means an `Owner` is never `can_manage`-able by anyone, so Owner removal is already rejected without a separate hard-coded check (FR-009).

## Frontend types (additive)

- `WorldMemberRecord`/`WorldMemberDoc` (`apps/web/src/api/worldMembers.ts`, `apps/web/src/hooks/useWorldMembers.ts`) gain `claimedActor: { id: string; label: string } | null` (or reuse whatever minimal shape `GraphQLWorldActor`'s existing consumers already use — match, don't invent a second actor summary shape).
- New `apps/web/src/api/worldMembers.ts` wrappers: `updateMemberRole(worldId, userId, role)` and `removeMember(worldId, userId)` — currently only inlined ad hoc inside `CampaignSettingsPanel.tsx`'s own `postGraphQL` calls; promoted to real wrapper functions so both `CampaignSettingsPanel` (if anything there still needs them — it shouldn't after FR-011) and the new `PlayersPage.tsx` can share one implementation instead of two independent inline copies.
