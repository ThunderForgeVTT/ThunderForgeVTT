# Contract: Collection Sharing

**Feature**: `026-content-collections` · **Date**: 2026-09-04

The GraphQL surface. Modelled on
`specs/025-world-abilities-compendium/contracts/ability-share.md`, with the
divergences called out where they exist.

---

## Queries

### `collection(id: UUID!): Collection`

An owner reading their own collection while building it. Requires authority over
the collection's world.

### `worldCollections(worldId: UUID!): [Collection!]!`

**The only listing query in this feature**, and it is scoped to one world the
caller has authority over. FR-020 forbids listing collections "by world, by
user, or globally" *beyond a user's own* — this is that exception and nothing
wider. There is no `collections` root, no search argument, no cursor over all
collections, and no resolver anywhere that accepts a share code and returns
anything but that one collection.

### `sharedCollection(shareCode: String!): SharedCollectionPreview`

**Unauthenticated.** This is the divergence from the three shipped share
queries, each of which calls `authenticated_user(ctx)?` before resolving — see
research §2. Resolving this one must **not** call it.

Returns for each resolvable, unrestricted, unmoderated member: its type, name,
and enough to preview it. Returns counts by type (US4 scenario 1) and a
`withheldCount` — a number, never a name (FR-022).

Refuses, with the same sentence in all four cases so the four are not
distinguishable by an outsider (FR-009d):

- the code does not exist
- the share is revoked
- the collection was deleted
- every member is withheld (FR-024) — *this* case may say "nothing is available"
  distinctly, because reaching it already required a valid code

Rate limited per caller IP (FR-009c) by a limiter of this feature's own, **not**
by `rate_limit_auth_requests`, which returns early for any path outside
`/authentication/`. This limiter must not honour
`THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT`: the e2e harness sets that on every run,
and a limiter switched off during its own test is untested.

**Reveals nothing about the source world** — not its id, its name, its other
content, nor its members (FR-009, FR-009d). The shipped
`shared_ability_preview_omits_source_world_identity` test is the precedent; note
that `shared_ability_impl` *reads* `worlds.game_system_id` to resolve a label
and returns the label, not the id. The same care applies here.

---

## Mutations

| Mutation | Requires | Enforces |
|---|---|---|
| `createCollection(worldId, name, description)` | DM of the world | FR-001 |
| `updateCollection(id, name, description)` | authority over the world | FR-005 |
| `deleteCollection(id)` | authority over the world | US2 scenario 4; deletes no artifacts (FR-004) |
| `addCollectionMember(collectionId, memberType, memberId)` | authority over the world | FR-001a (refusal names the reason), FR-003 (same world), FR-005a (≤100) |
| `removeCollectionMember(collectionId, memberId)` | authority over the world | FR-004: removing a member deletes nothing |
| `createCollectionShareLink(collectionId)` | **Owner** of the collection | FR-006, FR-008 |
| `revokeCollectionShareLink(shareId)` | the link's creator, or a DM of its world | FR-010, FR-011 |
| `copySharedCollectionToWorld(shareCode, destinationWorldId)` | **authenticated**, DM of the destination | FR-009b, FR-012–FR-019 |

`createCollectionShareLink` requiring Owner rather than DM follows
`create_ability_share_link_impl`, which refuses below
`ActorPermissionLevel::Owner` and then **re-checks visibility** with the comment
that sharing "is the one path that escapes the world, so it re-checks rather
than relying on that". The same re-check applies here, per member, at share
time as well as at add time.

---

## `copySharedCollectionToWorld` — the copy contract

One transaction, `conn.transaction::<_, CopyError, _>`, mirroring the shipped
copy paths. `CopyError` is a local newtype over `String` with
`From<diesel::result::Error>`; it exists to work around the orphan rule and is
re-declared per module in this codebase rather than shared.

**Inside the transaction, in order:**

1. Re-load the share and confirm it is active. The shipped paths do this and say
   why: the link may have been revoked between the preview and the confirm.
2. Re-assert the 100-member limit (FR-005a) — a concurrent add must not push a
   collection past it between preview and copy.
3. For each member: resolve it; skip and note it if it does not resolve, is
   moderated, or is restricted.
4. Refuse the whole copy if nothing remains (FR-024).
5. Insert copies, `created_by`/`updated_by` = the copier (FR-017a), no grant
   rows — the destination DM has implicit full control.
6. Re-point intra-collection references at the copies (FR-014), using an
   old-id → new-id map built as rows are inserted.
7. Record every out-of-collection reference as a fidelity note (FR-015).

**Returns** a `CopyReceipt`: what was created, by type, and the fidelity notes.
Not stored — FR-012 forbids a referential link back to the source, and a stored
receipt naming both sides is one.

**Any failure rolls back the whole transaction** (FR-013, SC-006). The
`?`-on-`CopyError` shape gives this by construction rather than by remembering
to clean up.

### Per-type copy rules

| Type | Copied | Notes |
|---|---|---|
| Ability | row + effects | Re-validate effect formulas as `copy_shared_ability_to_world_impl` does; preserve `gm_only` — fail closed, a copy arriving un-hidden exposes what the source hid |
| Item | row + effects | Preserve `gm_only`, same argument |
| Actor | row + inventory/known-ability links **within the collection** | Links to artifacts outside it become fidelity notes |
| Lore entry | row | Wiki links to entries outside the collection become fidelity notes |
| Scene | row + `walls` + `light_sources` + `shapes`; background via a **new asset row on the same `storage_path`** | **Not copied**: `tokens` (placed actors mid-play), `fog_masks` (per-session play state), `interactives` (wired to the source world's content). Each is a fidelity note, not a silent omission |

The scene background is the designed use of `storage/dedupe.rs`: each asset row
carries its own `asset_id`, `world_id`, `scene_id` and owner, and only
`storage_path` is shared; `canvas_assets_serve` authorises against the row it
looked up. Zero additional stored bytes (FR-019, SC-008), and the copy depends
on the object rather than on the source world (FR-018).

**Nothing in this feature deletes a stored object.** `dedupe.rs` states that
nothing in the product does, and that this is what makes a shared path safe.
Revocation flips a flag; deleting a collection removes rows. Reference counting
stays a prerequisite of any future deletion.

---

## Web routes

| Route | Auth | Purpose |
|---|---|---|
| `/world/:worldId/collections` | member | Owner's collections in one world |
| `/world/:worldId/collections/:id` | member | Build a collection, share, revoke |
| `/collection/:shareCode` | **none** | Preview and copy |

`/collection/:shareCode` must render for a signed-out visitor. Copying from it
prompts for sign-in and a destination world (FR-009b, FR-016) — the point at
which viewing and copying diverge.
