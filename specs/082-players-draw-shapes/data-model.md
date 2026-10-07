# Data Model: Players Draw Shapes

## Player defaults (code, not data)

| Constant               | Where                                                    | Value                  |
| ---------------------- | -------------------------------------------------------- | ---------------------- |
| `AUTHORING_TOOLS`      | `crates/thunderforge-server/src/auth/authoring_tools.rs` | unchanged, six tools   |
| `PLAYER_DEFAULT_TOOLS` | same file, new                                           | `["select", "shapes"]` |

Effective tools, in `AUTHORING_TOOLS` order:

| Caller                     | Effective tools                                 |
| -------------------------- | ----------------------------------------------- |
| DM of the world (or admin) | all six                                         |
| Non-DM member              | (`PLAYER_DEFAULT_TOOLS` − revocations) ∪ grants |
| Not a member               | none                                            |

## `world_authoring_tool_revocations` (new)

Migration `crates/thunderforge-server/migrations/2026-10-07-110000-0000_authoring_tool_revocations/`.

| Column            | Type          | Rule                                                        |
| ----------------- | ------------- | ----------------------------------------------------------- |
| `id`              | `uuid`        | primary key, `gen_random_uuid()`                            |
| `world_member_id` | `uuid`        | not null, references `world_members(id)` on delete cascade  |
| `tool`            | `varchar(32)` | not null, check `tool IN ('select', 'shapes')`              |
| `revoked_by`      | `uuid`        | references `users(id)` on delete set null                   |
| `revoked_at`      | `timestamp`   | not null, default `CURRENT_TIMESTAMP` (as the grants table) |

- Unique `(world_member_id, tool)`.
- `revoked_by` is nullable with `set null`, unlike the grants' `created_by`,
  so deleting the GM's account never fails on a revocation they wrote.

`up.sql` also deletes any `world_authoring_tool_grants` row whose `tool` is
`select` or `shapes` (R2), and adds a check to the grants table:
`tool NOT IN ('select', 'shapes')`. `down.sql` drops that check and the
table. Removed default-tool grants are not restored: with the defaults gone,
those players would hold nothing, as before this spec.

Diesel: `world_authoring_tool_revocations` in `src/schema.rs`, a
`joinable!` to `world_members`, and `NewWorldAuthoringToolRevocation` in
`src/models.rs`. No `Queryable` struct: like the grants table, the rows are
read as bare `tool` strings.

## `setAuthoringToolGrant` writes

| Tool           | `granted: true`            | `granted: false`             |
| -------------- | -------------------------- | ---------------------------- |
| select/shapes  | delete the revocation row  | upsert a revocation row      |
| the other four | upsert a grant row (today) | delete the grant row (today) |

## `shapes` (existing, unchanged)

`created_by` and `updated_by` are `NOT NULL REFERENCES users(id)`.
Ownership is `created_by`. Rules on write:

| Authority | create                   | update / delete            | `visible_to_players` |
| --------- | ------------------------ | -------------------------- | -------------------- |
| `Dm`      | yes                      | any shape on the scene     | as asked             |
| `Creator` | yes, holds `shapes`      | `created_by` = caller only | forced `true`        |
| `None`    | refused (existing error) | refused (existing refusal) | n/a                  |

## Account deletion (`delete_user_data_on`, `src/users/mod.rs`)

Inside its transaction, after the user's own worlds are gone and before the
user row is deleted:

| Rows                                                   | Action                                                                      |
| ------------------------------------------------------ | --------------------------------------------------------------------------- |
| `shapes` with `created_by` = user                      | delete; one `SHAPE_CHANGED` `deleted` event each, actor `worlds.created_by` |
| `shapes` with `updated_by` = user, `created_by` ≠ user | `updated_by = created_by`                                                   |

`UserDataDeleteSummary.shapes_deleted` counts the first.

## Canvas core `Shape` (`crates/thunderforge-canvas-core/src/shape.rs`)

| Field        | Type             | Note                                                      |
| ------------ | ---------------- | --------------------------------------------------------- |
| `created_by` | `Option<String>` | new, `#[serde(default)]`; `None` is editable by a GM only |

`WorldShapePayload` (`crates/thunderforge-engine/src/payloads.rs`) gains
`created_by: Option<String>` (`createdBy` on the wire), copied into `Shape`.
The web's `WorldShape` (`apps/web/src/engine/world/types.ts`) gains
`createdBy: string | null`, filled by `shapeRecordToWorldShape`.

## Engine resource

| Resource       | Type             | Set by                           |
| -------------- | ---------------- | -------------------------------- |
| `ViewerUserId` | `Option<String>` | `ExternalCommand::SetViewerUser` |

## Store command (web)

| Command        | Fields                                  | Bridge sends                      | Local effect |
| -------------- | --------------------------------------- | --------------------------------- | ------------ |
| `clear_shapes` | `sceneId: string; createdBy?: string[]` | `clearShapes(sceneId, createdBy)` | none         |

## Demo

The demo's saved world keeps its version. A stored shape with no
`createdBy` reads as the GM's (`DEMO_USER.id`).
