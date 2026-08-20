# World Access Control

**Status**: ✅ Implemented (ownership + membership model)
**Last Updated**: 2026-08-20
**Related ADRs**: ADR-013 (`20260504-013-graphql_ownership_enforcement.md`), ADR-023 (`20260504-023-world_ownership_rules.md`), ADR-009 (`20260504-009-created_by_updated_by_enforcement.md`)

---

## History note

This document previously described a three-tier OWNER/EDITOR/VIEWER RBAC
system (`world_collaborators`/`permission_grants` tables, `RbacEngine`,
`CollaboratorMutation`, audit logging via `audit_logs`) as "✅ IMPLEMENTED."
That was inaccurate: `src/server/src/rbac.rs` and `src/server/src/audit.rs`
existed on disk but were never included in the module tree (no `mod rbac;`
/ `mod audit;` in `main.rs`), the tables they referenced (`world_collaborators`,
`audit_logs`) were never migrated, and their one real call site (world
creation's `RbacEngine::assign_creator_as_owner`) was commented out as
"disabled pending schema." Neither file compiled as part of the binary.
Both were deleted (2026-08-20) rather than left as dead, misleadingly
documented code. This document now describes the access control model
that is actually live.

---

## Overview

ThunderForgeVTT authorizes access to a world (and everything scoped under
it — scenes, tokens, walls, shapes, light sources, canvas assets) using
two mechanisms:

1. **Ownership** — `worlds.created_by`. The user who created a world can
   always read, write, and delete it and its contents.
2. **Membership** — the `world_members` table, populated when a user
   accepts an invite (`world_invites` → `InviteMutation::join_world` in
   `src/server/src/graphql/mutations_invites.rs`). A `world_members` row
   for `(world_id, user_id)` grants that user visibility into the world
   (role stored as a string, e.g. `"Owner"`, `"GM"`, `"Player"` — no
   fine-grained permission hierarchy beyond membership itself today).

There is no separate `EDITOR`/`VIEWER` permission tier; a member either
has a `world_members` row (full read access, per the callers below) or
doesn't. Write authorization for most mutations (walls, shapes, tokens,
map import) is scoped directly to `scenes::owner_id` rather than world
membership — see "Known gap" below.

## Known gap: world creation doesn't insert an owner `world_members` row

`create_world` (`src/server/src/graphql.rs`) inserts a `worlds` row with
`created_by` set, but does **not** insert a corresponding `world_members`
row for the creator. This is the same gap the deleted `RbacEngine::
assign_creator_as_owner` was meant to close and never did. Every read
path that checks membership therefore falls back to `worlds.created_by`
(see `require_world_member`/`require_visible_world` below) to avoid
locking owners out of their own worlds. Fixing this at the source
(inserting the `world_members` row inside `create_world`) is a tracked
follow-up, not yet done — the fallback is a deliberate compensating
control, not a fix.

## Core functions

### `require_world_member` (`src/server/src/auth/world_membership.rs`)

Added for spec 002 (canvas image assets), but is the general-purpose
membership guard: given a `PgConnection`, `user_id`, and `world_id`,
returns the caller's role string if an accepted `world_members` row
exists, falling back to `worlds.created_by == user_id` (returning
`"Owner"`). Returns `WorldMembershipError::NotAMember` otherwise. Used
by `uploadCanvasImage` and `canvasImageAssetsForScene`
(`graphql/mutations_assets.rs`).

### `load_visible_world_by_id` / `require_visible_world`
(`src/server/src/graphql/helpers.rs`)

The general read-path guard for world/scene/token/wall/shape/light
queries in `graphql/queries/scene.rs` and `graphql/queries/user.rs`.
`load_visible_world_by_id` returns `Ok(None)` for both "world doesn't
exist" and "world exists but you can't see it" (deliberately
indistinguishable, so a caller can't probe for valid world IDs);
`require_visible_world` wraps it for resolvers that return `Vec<T>`
rather than `Option<T>` and so need a hard `FORBIDDEN` error instead of
an empty-looking `None`.

**Until 2026-08-20 this function performed no check at all** — a
leftover `if !is_admin { /* TODO: rbac module not implemented */ }`
no-op meant every authenticated user could read any other user's world,
and (because every call site in `scene.rs` discarded the return value
with `let _ = load_visible_world_by_id(...).await?;`) simply making the
check real would not have been sufficient on its own — the call sites
had to start actually using the result. Both were fixed together; see
`graphql/helpers.rs`'s test module for regression coverage
(`load_visible_world_by_id_rejects_non_member_non_owner`, etc.) and
`git log` for the fix commit.

### Scene-level authorization for mutations

Wall/shape/token/light-source mutations (`mutations_walls.rs`,
`mutations_shapes.rs`, `mutations_tokens.rs`, `mutations_lighting.rs`)
authorize directly against `scenes::owner_id.eq(user_id)`, not
`world_members`. This means a non-owner accepted `world_members` player
can currently *view* a world's scenes (per the read-path fix above) but
cannot author walls/shapes/tokens on them — only the scene's owner can.
Whether player-authored content should be allowed is a product decision,
not addressed by this document.

## Testing

Regression tests for the access-control fix live in
`src/server/src/graphql/helpers.rs` (`mod tests`) and require a live
Postgres (`DATABASE_URL`, see `compose.yml`). Related coverage:
- `graphql/mutations_walls.rs`, `mutations_shapes.rs`,
  `mutations_tokens.rs`, `mutations_lighting.rs`: `*_scoped_to_scene_owner`
  tests for the scene-ownership write checks.
- `graphql/mutations_assets.rs`: membership grant/revoke tests for the
  spec-002 canvas-asset read/write paths.

## Related documentation

- ADR `20260504-013-graphql_ownership_enforcement.md`
- ADR `20260504-023-world_ownership_rules.md`
- ADR `20260504-009-created_by_updated_by_enforcement.md`
- `specs/002-canvas-authoring-asset-storage/research.md` §7 (the
  `world_members`-based guard this document's `require_world_member`
  section describes)
