# Contract: Ability share links and Copy-to-World

User Story 6 (FR-032..FR-037). Mirrors
`specs/013-items-inventory/contracts/item-share.md`.

> ## Guardrail prerequisite — T001 must land first
>
> Constitution v1.1.0's DMCA guardrail requires, **before implementation
> begins**, both:
>
> - **(a)** the notice-and-takedown program is operational — **SATISFIED**
>   (spec 015 complete, 41/41).
> - **(b)** an on-record "centralized public repository" determination —
>   drafted as
>   `docs/adrs/20260825-049-share_link_dmca_repository_determination.md`,
>   pending acceptance by an accountable owner. That is **task T001**, the first
>   task in this feature.
>
> ADR-049 finds that share links are **not** a centralized public repository,
> and covers actor (spec 010) and item (spec 013) shares retroactively — no
> determination had ever been recorded for any of them.
>
> **The finding is conditional on six invariants.** Everything in this contract
> is designed to satisfy them; violating any one re-opens the determination:
>
> 1. **No enumeration** — no query returns shares by world, by user, or in
>    aggregate (FR-037).
> 2. **No discovery surface** — no search, index, directory, or browse view over
>    shared content.
> 3. **Unguessable codes** — v4-derived, never v7.
> 4. **Owner-controlled and revocable** — Owner-level to create; creator or DM
>    can revoke; revoked resolves to a distinct unavailable state.
> 5. **Takedown-effective** — a moderated ability's share must stop resolving.
> 6. **One-time deep copy** — independent record, empty ownership block, no
>    referential link back.
>
> User Stories 1-5 have no dependency on this contract.

## Types

```graphql
type GraphQLAbilityShareLink {
  id: UUID!
  abilityId: UUID!
  shareCode: String!
  revoked: Boolean!
  createdAt: String!
}

type SharedAbilityPreview {
  name: String!
  description: String
  classification: AbilityClassification!
  effects: [GraphQLAbilityEffect!]!
}

input CopySharedAbilityInput {
  shareCode: String!
  destinationWorldId: UUID!
}
```

`SharedAbilityPreview` deliberately carries **no** `id`, `worldId`, `createdBy`,
or ownership block — a viewer must not be able to identify the source world or
its members (FR-033). Mirrors `SharedItemPreview`.

## Queries

| Query | Args | Returns | Authorization |
|---|---|---|---|
| `sharedAbility` | `shareCode: String!` | `SharedAbilityPreview!` | **Authenticated only — no world-membership check, by design.** Then blocked entirely if `moderation::effective_status(state, "world_ability", id)` is `Some` |

## Mutations

| Mutation | Args | Returns | Authorization |
|---|---|---|---|
| `createAbilityShareLink` | `abilityId: UUID!` | `GraphQLAbilityShareLink!` | `effective_ability_permission(...)`, then explicit `level.rank() < Owner.rank()` reject |
| `revokeAbilityShareLink` | `shareId: UUID!` | `Boolean!` | link's `created_by == user_id` **OR** `is_dm_of_world` of the ability's world |
| `copySharedAbilityToWorld` | `input: CopySharedAbilityInput!` | `GraphQLAbility!` | `is_dm_of_world(destination_world_id)` **first**, then re-validate the share link inside the transaction |

## Share-code generation

Reuse `generate_share_code()`: a **v4** UUID, dashes stripped, first 20 chars,
uppercased. Stored in `VARCHAR(32) NOT NULL UNIQUE`.

⚠️ **The v4 derivation is load-bearing — do not "optimize" it to v7.** Spec 005
found and fixed a real collision bug where invite codes derived from a **v7**
UUID's leading hex characters collided for anything generated within the same
millisecond, because v7 front-loads a timestamp. The row's own `id` stays v7 for
index locality; only the human-facing code must be v4-derived.

## Revocation

`revoked` is a soft boolean flag, never a row delete — FR-036 requires a revoked
link to render a distinct "no longer available" state, which a deleted row could
not distinguish from a code that never existed. `load_active_share` filters
`revoked = false` and errors with "This share link is no longer available".

## GM-only interaction

- A **non-DM cannot share a GM-only ability**, and this needs no extra guard: a
  GM-only ability's detail data is already denied to non-DMs (FR-025), so they
  can never reach a share control for it. `createAbilityShareLink` should still
  reject defensively rather than rely on that.
- A **DM may share a GM-only ability.** Hiding is about players in *this* world;
  sharing is an explicit, deliberate act by the person who set the flag.
- **`gm_only` is preserved on copy**, unlike the ownership block (which resets to
  empty). Fail closed: a copy arriving un-hidden would silently expose content
  that was hidden at the source, and the destination DM can always clear the flag
  themselves. This is the one field where copy semantics deliberately diverge
  from "reset to defaults".

## Copy semantics (FR-035)

A deep, one-time copy producing a fully independent record:

- new `id`, `world_id` = destination
- same `name`, `description`, `classification`, **and `gm_only`** (see above)
- **all effects cloned** and re-parented to the copy
- `created_by`/`updated_by` = the copying user
- **empty ownership block** (destination DM has implicit full control)
- **no** live or referential link back to the source

Subsequent edits to either side must not affect the other. Runs inside a single
`conn.transaction`, using the `CopyError` newtype wrapper pattern
(`From<diesel::result::Error>` + `From<String>`) that `mutations_item_shares.rs`
uses to work around the orphan rule.

**Re-validate effect formulas on copy** — unlike the item version, which clones
effects without re-running `validate_formula` (research.md §3, defect 4).

## FR-037 — no discoverability

**Structurally guaranteed by omission**: there is no world-scoped or global
"list share links" query, and none is added. A share is reachable only by
possessing its code. This is what keeps the feature on the non-repository side
of the guardrail determination — it must stay true.

Do not add an "all my shares" or "shares for this world" query without
re-opening the guardrail question.

## Moderation integration — mandatory

`sharedAbility` MUST block on a moderated ability. Without this, share links are
a moderation bypass for exactly the content type the DMCA guardrail concerns.
Spec 013 guards this with
`shared_item_is_unavailable_once_moderation_disabled`; abilities need the
analogue.

## Frontend

- `apps/web/src/pages/ability-share/SharedAbilityPage.tsx` — mirrors
  `SharedItemPage.tsx`: requires login (redirects to
  `/login?returnTo=/shared/ability/{code}`) but not world membership; loads
  `Promise.all([getSharedAbility(code), getMyDmWorlds()])`; step machine
  `idle → confirming → copying → done`; shows a "you don't have DM-level access
  to any world yet" message when `dmWorlds` is empty.
- Route: `/shared/ability/:code`, registered in `AppRoutes.tsx` + a
  `pageLoaders.ts` entry.
- Share/revoke controls on the ability detail page, gated on
  `myPermissionLevel === "OWNER"`.
- Classification renders through `resolveAbilityLabel` — but note the shared
  page has no world context, so it uses **default labels** (facets belong to a
  system, and the preview deliberately hides the source world).

## Test expectations

- `create_ability_share_link_requires_owner_level`.
- `copy_produces_independent_ability_with_cloned_effects` — non-DM at the
  destination rejected; the DM's copy has a new id, the destination `world_id`,
  cloned effects re-parented, and an empty ownership block.
- `revoked_share_link_is_unavailable`.
- `shared_ability_is_unavailable_once_moderation_disabled`.
- `shared_ability_preview_omits_source_world_identity`.
- `copy_preserves_gm_only_flag` — a GM-only source produces a GM-only copy.
- `non_dm_cannot_create_share_link_for_gm_only_ability`.
