# Phase 1 Data Model: GM Staging Page and Full-Screen Play Canvas

No database schema changes. This feature reuses three existing tables as-is and adds one new GraphQL read path over an existing table.

## Existing entities reused unchanged

### Scene (`scenes` table)
Already fully modeled and queryable via `scenes(worldId)`. Reused as-is for both the staging page's scene selector and the full-screen sidebar's scene section (both render the existing `SceneSwitcher` component).

### World Member (`world_members` table)
Already fully modeled and queryable via `worldMembers(worldId)` (server) / `useWorldMembers.ts` (client, RxDB-replicated). Fields relevant to this feature: `user_id`, `role` (`"Owner" | "GM" | "Player"`). Reused as-is for the staging page's player roster and for GM-role gating (see research.md §3).

### World (`worlds` table)
Already fully modeled. Only `createdBy` is newly load-bearing for this feature's role-gating fallback (research.md §3) — already present, no change.

## New read path: World Actor (`world_actors` table, existing)

The table already exists with every field this feature needs:

| Column | Type | Used for |
|---|---|---|
| `id` | Uuid | React key / future actor-detail linking |
| `world_id` | Uuid | Query filter — this feature's new `worldActors(worldId)` |
| `scene_id` | Uuid | Not filtered on by this feature's query (world-scoped, research.md §2), but returned for future scene-grouping in the roster UI |
| `actor_type` | String | Display (e.g. "npc", "character", "hazard") |
| `label` | String | Display name in the roster |
| `is_npc` | Bool | Distinguishes NPCs from player characters — the staging page's "NPC roster" filters/displays where `is_npc = true` |
| `is_public` | Bool | Not used by this feature (no visibility filtering beyond world membership is in scope) |
| `owned_by` | Uuid | Not used by this feature (player-character assignment is explicitly out of scope, per spec FR-015) |
| `created_at` / `updated_at` | Timestamp | Display only, if useful (e.g. sort order) |

**What's new**: a GraphQL query resolver (`ActorQuery::world_actors`) and its payload type (`GraphQLWorldActor`, a straightforward field-for-field mirror of the existing `WorldActor` Rust struct — see `contracts/world-actors-query.md`). No new table, no new columns, no migration.

## New client-only concept: staging/playing UI state

Not persisted anywhere (not a database entity, not synced across users — per spec FR-014). A single client-side state value on `WorldPage.tsx`:

```
type PlayViewState = "staging" | "playing";
```

- Initialized to `"staging"` on every fresh visit to `/world/:id/play`.
- Set to `"playing"` when the user clicks "Play" on the staging page.
- Set back to `"staging"` when the user clicks the on-screen "back to setup" control in full-screen mode.
- Never read from or written to the server, never included in any GraphQL request, never broadcast via the existing `world_event_sender`/pubsub channel used for real, synchronized world state (walls, tokens, etc.) — this is deliberately outside that system, matching FR-014's "not synchronized across users" requirement.

## Relationships (unchanged)

```
World 1──* Scene
World 1──* WorldMember
World 1──* WorldActor (this feature's new read path)
Scene 1──* WorldActor (existing FK, not queried by this feature)
```
