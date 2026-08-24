# Research: Player Onboarding — Invite-to-Actor Selection

## 1. Where does the claim relationship live?

**Decision**: A new table `world_actor_claims` (`id`, `actor_id` UNIQUE, `world_member_id` UNIQUE, `claimed_at`), rather than a column on either `world_actors` or `world_members`.

**Rationale**: A claim is a symmetric 1:1 relationship between two existing entities that both already have rich schemas (`world_actors`, `world_members`). Adding `claimed_by_member_id` directly to `world_actors` would work for "who claims this actor" but makes "does this member already have a claim" a reverse lookup with no natural place to also enforce "one claim per actor" and "one claim per member" as two independent unique constraints in one place. A dedicated join table lets both invariants (FR-006's "one active claim per Actor", FR-014's "one active claim per (world, member)") be expressed as two `UNIQUE` constraints on the same table, and makes un-claim (FR-013) a single `DELETE`.

**Alternatives considered**:
- Column on `world_actors` (`claimed_by_member_id UUID UNIQUE`): simpler schema, but "one claim per member" then requires a `UNIQUE` index on that column anyway (which is exactly what a join table's second unique constraint gives you) — no real complexity savings, and it conflates "ownership" (`owned_by`, already on `world_actors`, meaning "who administratively controls this row today, usually the GM") with "who is playing this character," which spec.md's Assumptions section explicitly says must stay distinct from the ownership-block model. Rejected to avoid the same conflation FR-016 explicitly warns against.
- Column on `world_members` (`claimed_actor_id UUID UNIQUE`): same shape, same rejection rationale, plus `world_members` is reused across every feature that touches membership and is a less natural place to add actor-specific state.

## 2. How is "available for claiming" represented?

**Decision**: `world_actors.available_for_claim BOOLEAN NOT NULL DEFAULT false`, a flat column alongside the existing `is_npc`/`is_public` booleans on the same table.

**Rationale**: Matches the existing `world_actors` schema's own pattern (`is_public`, `is_npc` are both flat booleans on the actor row) and is a property of the Actor itself, not a relationship — an actor is either currently offered up or not, independent of whether anyone has claimed it (spec.md Key Entities: "independent of whether it is currently claimed"). A GM-only-mutable, Owner-permission-gated column read alongside the rest of the actor row needs no join.

**Alternatives considered**: A separate `world_actor_availability` table mirroring the claims table — rejected as unnecessary indirection for a single boolean with no additional fields (unlike the claim, which has a genuine 1:1 relationship and a `claimed_at` timestamp worth recording; availability is just a flag with no analogous "since when" requirement in the spec).

## 3. Where does the "allow players to create their own actors" setting live?

**Decision**: `worlds.allow_player_created_actors BOOLEAN NOT NULL DEFAULT false`, a flat column on `worlds` — following the exact precedent of `worlds.session_notes` (added ad hoc for spec 011) and `worlds.game_system_id`/`interface_pack_id`. Default `false` per the spec's Clarifications (Q1).

**Rationale**: `worlds` already carries several per-world settings as flat nullable/boolean columns rather than a separate `world_settings` table; there is no existing settings-table precedent in this codebase to break from. A single boolean does not warrant introducing one.

## 4. How is atomic claiming (FR-006, SC-003) enforced?

**Decision**: The `claimActor` mutation performs a single `INSERT INTO world_actor_claims (...) SELECT ... WHERE world_actors.available_for_claim AND world_actors.id NOT IN (SELECT actor_id FROM world_actor_claims)` guarded by the table's own `UNIQUE(actor_id)` constraint as the final backstop — if two requests race past the `NOT IN` check simultaneously, the database's unique-constraint violation on the second `INSERT` is caught and mapped to the "this character was just claimed" GraphQL error (Edge Cases). `createAndClaimActor` inserts the new `world_actors` row and its claim in the same transaction, so no race is possible there (the actor doesn't exist for anyone else to contend over until the transaction commits).

**Rationale**: Postgres unique constraints are the standard, already-used-elsewhere-in-this-codebase (`world_invites.invite_code`, `world_lore_entries` slug uniqueness) mechanism for "exactly one wins" under concurrency — no new locking primitive, row lock, or advisory lock needed. `SELECT ... FOR UPDATE` was considered but is unnecessary: the unique constraint alone gives the correctness guarantee (SC-003's "zero instances of two members simultaneously recognized as the same character"), and a `FOR UPDATE` row lock would only matter if the mutation needed to read-then-conditionally-write across multiple statements, which it doesn't.

**Alternatives considered**: Application-level check-then-write with no DB constraint — rejected outright, this is exactly the TOCTOU race the requirement calls out as unacceptable ("never a silent double-claim").

## 5. How does the client know to route to Actor Selection instead of the world dashboard?

**Decision**: A `myActorClaim(worldId: ID!): GraphQLActorClaim` query (nullable — `null` means "no claim yet"). The route that currently lands a joining member on `/world/:id` (both `JoinWorldPage.tsx`'s post-join `navigate()` and any direct visit to `/world/:id`) checks this query (skipped entirely for the GM/Owner role, per FR-003) and redirects to `/world/:id/actor-select` when no claim exists and the caller is a non-GM member.

**Rationale**: Mirrors the existing `useWorldRole` hook's pattern of a per-world, per-user server-derived value the frontend gates rendering on, rather than introducing new client-side state. Server remains authoritative (Principle III) — the redirect is a UX convenience, not a security boundary; every mutation the Actor Selection screen offers independently re-verifies claim status server-side.

**Alternatives considered**: Redirecting at `joinWorld` mutation response time only — rejected because FR-001 requires the gate to apply "the first time they access that world after joining," which must also cover a member who joined before this feature existed, or who navigated away before selecting a character and is now revisiting — a one-time post-join redirect wouldn't cover those.

## 6. Does un-claiming need a new permission concept?

**Decision**: No. `unclaimActor` requires the same Owner-level Actor permission already checked by `auth::actor_permissions::require_actor_permission` (spec 010) — a GM has this by default on every actor in their world (spec 010's DM-always-full-control rule, referenced directly in the Clarifications). No new role or grant is introduced.

**Rationale**: Explicitly settled in spec.md's Clarifications (Q3) — reusing the existing authority model was the resolved answer, not an open question.
