# ADR-035: Player View Architecture

**Date:** 2026-05-05  
**Status:** ACCEPTED  
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Players in a VTT need a restricted view of the scene where:
- They see only tokens they own or GM reveals
- They cannot access GM tools
- Fog of War hides unrevealed areas
- Real-time sync respects their permissions
- Performance: Render 100+ visible tokens smoothly

---

## Decision

Implement a **Player View** with permission-aware GraphQL subscriptions and restricted React components.

### Architecture

```
┌─────────────────────────────────────┐
│ Player Router: /play/:worldId/:sceneId
├─────────────────────────────────────┤
│                                     │
│ • Verify world membership           │
│ • Load player permissions           │
│ • Restrict to player-visible tokens │
│ • Hide GM tools                     │
│                                     │
└────────┬────────────────────────────┘
         │
         ↓ GraphQL Query
┌─────────────────────────────────────┐
│ Backend Permission Filtering        │
├─────────────────────────────────────┤
│ tokens(sceneId: ID!) → Query:       │
│                                     │
│ SELECT * FROM world_tokens          │
│ WHERE scene_id = $1                 │
│ AND (                               │
│   created_by = $2    -- Own token   │
│   OR id IN (         -- Revealed by │
│     SELECT token_id FROM            │
│     player_token_visibility         │
│     WHERE player_id = $2            │
│   )                                 │
│ )                                   │
│                                     │
└────────┬────────────────────────────┘
         │
         ↓ RxDB Sync
┌─────────────────────────────────────┐
│ React Player View                   │
├─────────────────────────────────────┤
│ • PlayerCanvas (Bevy rendering)     │
│ • TokenInspector (read-only)        │
│ • No FogTools, no World settings    │
│ • Chat integration (future)         │
│                                     │
└─────────────────────────────────────┘
```

### Backend: Permission-Aware Queries

```rust
pub async fn get_scene_for_player(
    world_id: &str,
    scene_id: &str,
    player_id: &str,
    conn: &mut PgConnection,
) -> Result<Scene> {
    // Verify player has access to world
    let _world = worlds::table
        .filter(worlds::id.eq(world_id))
        .filter(worlds::id.eq_any(
            // Join to player_world_memberships
            player_world_memberships::table
                .filter(player_world_memberships::player_id.eq(player_id))
                .select(player_world_memberships::world_id)
        ))
        .first(conn)?;
    
    // Return scene
    scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .filter(scenes::world_id.eq(world_id))
        .first(conn)
}

pub async fn get_tokens_for_player(
    scene_id: &str,
    player_id: &str,
    conn: &mut PgConnection,
) -> Result<Vec<WorldToken>> {
    // Get all player-visible tokens:
    // 1. Tokens the player owns
    // 2. Tokens revealed by GM (via fog visibility)
    
    world_tokens::table
        .filter(world_tokens::scene_id.eq(scene_id))
        .filter(
            world_tokens::created_by.eq(player_id).or(
                world_tokens::id.eq_any(
                    player_token_visibility::table
                        .filter(player_token_visibility::player_id.eq(player_id))
                        .filter(player_token_visibility::scene_id.eq(scene_id))
                        .select(player_token_visibility::token_id)
                )
            )
        )
        .load(conn)
}

pub async fn get_fog_for_player(
    scene_id: &str,
    player_id: &str,
    conn: &mut PgConnection,
) -> Result<FogMask> {
    // Players see the REVEALED fog (inverse of GM fog)
    let gm_fog = fog_masks::table
        .filter(fog_masks::scene_id.eq(scene_id))
        .first(conn)?;
    
    // Return inverted mask (revealed areas are transparent)
    Ok(FogMask {
        bitmap_data: invert_fog_mask(&gm_fog.bitmap_data),
        ..gm_fog
    })
}
```

### GraphQL Subscription: Permission-Aware Events

```graphql
type Subscription {
  # Player only sees tokens they own or GM revealed
  playerTokensUpdated(sceneId: ID!): TokenEvent!
  
  # Player only sees fog changes (revealed/hidden regions)
  playerFogUpdated(sceneId: ID!): FogEvent!
}

# Resolver enforces ownership checks
async fn player_tokens_updated(
    ctx: &Context,
    scene_id: String,
) -> impl Stream<Item = TokenEvent> {
    let player_id = authenticated_user(ctx)?.user_id;
    let permissions = load_player_permissions(scene_id, player_id).await?;
    
    world_event_subscription
        .filter(move |event: &WorldEvent| {
            match event.token_event {
                Some(token_evt) => {
                    let token_id = &token_evt.token_id;
                    // Only send if player owns token or it's in revealed fog
                    permissions.can_view_token(token_id)
                }
                None => false,
            }
        })
}
```

### Frontend: Player View Component

```tsx
function PlayerView({ worldId, sceneId }) {
  const { user } = useAuth();
  const scene = useQuery(GetSceneQuery, { sceneId });
  const tokens = useRxDB("world_tokens")
    .find({ _selector: { sceneId } })
    .sort("createdAt");
  const fog = useQuery(GetPlayerFogQuery, { sceneId });
  
  return (
    <div className="player-view">
      {/* Bevy canvas with restricted rendering */}
      <PlayerCanvas
        scene={scene}
        tokens={tokens}
        fog={fog}
        playerId={user.id}
      />
      
      {/* Visible tokens list (read-only) */}
      <TokenInspector tokens={tokens} readOnly={true} />
      
      {/* No GM tools shown */}
    </div>
  );
}

function PlayerCanvas({ scene, tokens, fog, playerId }) {
  // Render subset of scene:
  // - Only player-owned tokens
  // - Only tokens in revealed fog regions
  // - Restricted camera (no pan outside scene bounds)
  
  return (
    <div className="canvas">
      <BeamSceneCanvas
        scene={scene}
        tokens={tokens}
        fog={fog}
        mode="player"
        playerId={playerId}
      />
    </div>
  );
}
```

### Routing

```rust
// Axum router

Router::new()
  // GM routes
  .route("/worlds/:worldId", get(world_page))
  .route("/worlds/:worldId/scenes/:sceneId", get(scene_editor))
  
  // Player routes (permission-checked)
  .route("/play/:worldId/:sceneId", get(player_view_page))
    .layer(axum::middleware::from_fn(
      verify_player_world_access
    ))
```

### Permission Rules

| Action | GM | Player (Own Token) | Player (Other Token) | Player (Revealed) |
|--------|----|--------------------|---------------------|-------------------|
| Move token | ✅ | ✅ | ❌ | ❌ |
| Edit token | ✅ | ✅ | ❌ | ❌ |
| View token | ✅ | ✅ | ❌ | ✅ |
| Paint fog | ✅ | ❌ | ❌ | ❌ |
| See GM tools | ✅ | ❌ | ❌ | ❌ |

### Token Visibility Calculation

When GM reveals a token, insert into `player_token_visibility`:

```rust
pub async fn reveal_token_for_player(
    token_id: &str,
    player_id: &str,
    scene_id: &str,
    gm_id: &str,
    conn: &mut PgConnection,
) -> Result<()> {
    // Verify GM owns scene
    let _ = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .filter(scenes::created_by.eq(gm_id))
        .first::<Scene>(conn)?;
    
    // Insert visibility record
    diesel::insert_into(player_token_visibility::table)
        .values((
            player_token_visibility::token_id.eq(token_id),
            player_token_visibility::player_id.eq(player_id),
            player_token_visibility::scene_id.eq(scene_id),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    
    Ok(())
}
```

---

## Consequences

### Positive
- ✅ Clean separation of GM/Player views
- ✅ Permission checks at DB level (safe)
- ✅ Real-time sync respects permissions
- ✅ No information leaks via GraphQL
- ✅ Extensible for future role types (spectator, enemy, etc.)

### Negative
- ⚠️ Extra join overhead in token queries (needs indexing)
- ⚠️ Player token visibility table grows with users

---

## Performance Optimization

1. **Indexes:** Add composite index on (scene_id, player_id, token_id)
2. **Materialized View:** Cache player-visible tokens at scene level
3. **Caching:** Cache permissions for 5 seconds (invalidate on fog update)

```sql
CREATE INDEX idx_ptv_scene_player ON player_token_visibility(scene_id, player_id);
```

---

## Related ADRs

- ADR-031: Scene Domain Model
- ADR-033: Token Data Model & Ownership
- ADR-034: Fog of War Implementation

---

## References

- Comparable: Foundry VTT (player tokens view)
- Permission model: https://en.wikipedia.org/wiki/Attribute-based_access_control
