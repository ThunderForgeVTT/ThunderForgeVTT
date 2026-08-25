# Phase 0 Research: Players Section

## 1. Route, nav, and page-shell pattern

**Decision**: Add `/world/:id/players` following the exact `ScenesRoutePage.tsx`/`ScenesPage.tsx` pattern: a route component fetches `getWorld(id)` + `useWorldRole(id, world)`, wraps the page component in `WorldSectionShell`. Add "Players" to `WorldSidebarNav.tsx`'s `categories` array (visible to every member, like Scenes/NPCs/Lore/Items/Abilities) — not `adminCategories`, since User Story 1 requires every member to see the roster; the GM-only controls are gated *inside* the page (`isGm` branch), matching how Compendium/Scenes already do it.

**Rationale**: This is the third world-section feature built on this exact shell/nav pattern (Compendium, Scenes, now Players) — reusing it outright is both the path of least resistance and keeps the sidebar's mental model consistent (one destination per world capability, GM extras revealed inside the page, not hidden behind a separate admin-only nav entry).

**Alternatives considered**: Putting "Players" only in `adminCategories` and giving non-GM members no nav entry at all — rejected outright, contradicts FR-003 (every member views the roster).

## 2. Member ↔ claimed-character data

**Decision**: Add a `claimedActor: GraphQLWorldActor` (nullable) field to the member type `worldMembers(worldId)` already returns, resolved server-side by looking up `world_actor_claims` on `world_member_id = member.id` (join to `world_actors` for the actor's name/label). One query, already-scoped by existing `worldMembers` authorization (any world member).

**Rationale**: No existing query returns "every member paired with their claimed actor" in one shot. `myActorClaim(worldId)` only returns the *caller's own* claim. The alternative — fetching `worldMembers(worldId)` and a full actors list client-side and joining on `claimedBy.id` — would work with zero backend changes, but means fetching every actor in the world (including NPCs, filtered client-side) just to read one field, and duplicating a join the server can do in one indexed lookup. A single additive field on the member payload is the smaller, more direct change, and mirrors how `GraphQLWorldActor` already exposes `claimedBy` (a `GraphQLWorldMember`) in the opposite direction — this is the same relationship, read from the other side.

**Alternatives considered**: A new standalone `worldActorClaims(worldId): [ActorClaim!]!` query — rejected as a parallel data shape the frontend would then have to re-join against `worldMembers` itself anyway; putting the field directly on the member payload avoids that client-side join entirely.

## 3. Role-change and remove-member — reuse existing mutations, with one bundled fix

**Decision**: Reuse `updateMemberRole`/`removeMember` (`src/server/src/graphql/mutations_invites.rs`) as-is for FR-007/FR-008/FR-009 — both already enforce a role-hierarchy check (`can_change_roles`/`can_manage`, from `thunderforge_core::models::invites::WorldMemberRole`) and `remove_member` already rejects self-removal outright. No new authorization logic needed.

One small, bundled fix: both resolvers currently look up the caller's own `world_members` row directly, **without** the `require_world_member` Owner-fallback used everywhere else in this codebase (`is_dm_of_world`, `world_members_impl`, etc.) to handle the known gap that `create_world` never inserts an owner `world_members` row (documented in `graphql.rs`'s `create_world_impl`). Concretely: a world's Owner who has never triggered a `world_members` backfill for themselves (e.g. never had another member's invite-accept touch their own row) would get rejected by `update_member_role`/`remove_member` today — a latent bug, not something this feature introduces, but one this feature would otherwise silently inherit and ship as "the GM's own role-management controls sometimes don't work for the GM." Since the Players section is about to become the *sole* place these actions happen (FR-011), fixing the caller-lookup to go through `require_world_member` (matching every other DM-gated mutation) is bundled into this feature rather than deferred.

**Rationale**: FR-010 requires "the same effect as elsewhere" — but "elsewhere" (the dashboard panel) is being removed by this same feature (FR-011), so there's no reason to preserve a bug alongside the fix; fixing the one caller-lookup path this feature actually depends on is in-scope, not scope creep, and low-risk (mirrors an established, already-reviewed pattern used by half a dozen other mutations).

**Alternatives considered**: Leave the bug as-is and document it as a known limitation — rejected; this feature is explicitly making these two mutations the sole entry point for role/removal management, so shipping them silently broken for some Owners is a worse outcome than a small, well-precedented fix.

## 4. `CampaignSettingsPanel`'s remaining scope after FR-011

**Decision**: `CampaignSettingsPanel.tsx` keeps invite generation/listing/copy and the "Allow players to create their own actors" toggle; the "Player Roster" list block and its embedded role-`<select>`/Remove-button controls are deleted outright (not just hidden), and the panel's header copy ("Manage invites and player roster") is updated to drop the roster-management framing.

**Rationale**: Directly implements FR-011 (supersede, not duplicate). Confirmed via research that these are the panel's only other responsibilities — nothing else depends on the roster block being present.

**Alternatives considered**: none — this is what the Clarifications session already decided; this section just confirms what "supersede" concretely means in this file.
