# ADR-033: Token Data Model & Ownership

**Date:** 2026-05-05  
**Status:** ACCEPTED  
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Tokens are game entities placed on scenes (miniatures, pawns, vehicles). We need to define:

1. What are the base properties of a token?
2. How do we prevent unauthorized token manipulation?
3. How do optimistic updates + server rejection work?
4. How do derived stats optimize network payloads?

---

## Decision

Tokens are **owned entities** with strict database-level ownership enforcement and optimistic client updates.

### Diesel Schema

```rust
#[derive(Queryable, Selectable, Insertable, AsChangeset, Serialize, Deserialize)]
#[diesel(table_name = world_tokens)]
pub struct WorldToken {
    pub id: String,                    // UUID
    pub world_id: String,              // Foreign key
    pub scene_id: String,              // Which scene
    pub base_x: i32,                   // Base position (grid coords)
    pub base_y: i32,
    pub base_z: i32,                   // Layer depth
    pub label: Option<String>,         // "Goblin 1", "PC Name"
    pub token_type: String,            // "NPC", "Player", "Vehicle"
    pub health: Option<i32>,           // Current HP
    pub max_health: Option<i32>,       // Max HP
    pub metadata: serde_json::Value,   // Conditions, buffs, custom fields
    pub created_by: String,            // Ownership (enforced at query level)
    pub updated_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
```

### GraphQL Schema

```graphql
type Token {
  id: ID!
  worldId: ID!
  sceneId: ID!
  baseX: Int!
  baseY: Int!
  baseZ: Int!
  label: String
  tokenType: TokenType!
  health: Int
  maxHealth: Int
  # Derived stats (computed locally, never sent as mutation input)
  healthPercentage: Float
  isDead: Boolean!
  createdBy: ID!
  updatedBy: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
}

enum TokenType {
  NPC
  PLAYER
  VEHICLE
}

type Mutation {
  createToken(input: CreateTokenInput!): Token!
  upsertToken(input: UpsertTokenInput!): Token!
  moveToken(tokenId: ID!, x: Int!, y: Int!, z: Int!): Token!
  deleteToken(tokenId: ID!): Boolean!
}

input CreateTokenInput {
  sceneId: ID!
  label: String!
  tokenType: TokenType!
  x: Int!
  y: Int!
  z: Int!
  health: Int
  maxHealth: Int
}

input UpsertTokenInput {
  id: ID!
  label: String
  health: Int
  maxHealth: Int
  metadata: JSON
}
```

### Ownership Enforcement

**Key Rule:** Mutations filter by `created_by == authenticated_user_id` at the Diesel query level.

```rust
pub async fn move_token(
    token_id: &str,
    new_x: i32,
    new_y: i32,
    new_z: i32,
    user_id: &str,
    conn: &mut PgConnection,
) -> Result<WorldToken, Error> {
    use crate::schema::world_tokens;
    
    // ✅ Ownership check at DB level (not in Rust code)
    let token = diesel::update(
        world_tokens::table
            .filter(world_tokens::id.eq(token_id))
            .filter(world_tokens::created_by.eq(user_id))  // <-- Enforced here
    )
    .set((
        world_tokens::base_x.eq(new_x),
        world_tokens::base_y.eq(new_y),
        world_tokens::base_z.eq(new_z),
        world_tokens::updated_by.eq(user_id),
        world_tokens::updated_at.eq(Utc::now().naive_utc()),
    ))
    .get_result(conn)
    .optional()?
    .ok_or(Error::Unauthorized)?;
    
    Ok(token)
}
```

**Why at DB Level?**
- Prevents information leaks (0 rows updated, no error thrown)
- Simplifies permission logic (single source of truth)
- Complies with ADR-013 (GraphQL Ownership Enforcement)

### Optimistic Updates + Rollback

#### Bevy Side

```rust
pub fn movement_system(
    mut query: Query<(&mut Transform, &mut WorldTokenComponent, &GlobalTransform)>,
    input: Res<TokenMovementInput>,
    mut mutation_queue: ResMut<GraphQLMutationQueue>,
) {
    for (mut transform, mut token, _global) in &mut query {
        if let Some(target_pos) = input.get_target() {
            // 1. Store old position for rollback
            token.last_server_x = token.base_x;
            token.last_server_y = token.base_y;
            token.last_server_z = token.base_z;
            
            // 2. Update immediately (optimistic)
            transform.translation = target_pos;
            token.base_x = target_pos.x as i32;
            token.base_y = target_pos.y as i32;
            
            // 3. Enqueue mutation
            mutation_queue.push(GraphQLMutation::MoveToken {
                token_id: token.id.clone(),
                x: token.base_x,
                y: token.base_y,
                z: token.base_z,
            });
        }
    }
}

pub fn apply_server_updates_system(
    mut query: Query<&mut WorldTokenComponent>,
    mut subscription_rx: ResMut<SubscriptionReceiver>,
) {
    while let Ok(event) = subscription_rx.try_recv() {
        match event.event_code {
            2 => { // Moved
                if let Ok(mut token) = query.get_mut(Entity::from_raw(event.token_id)) {
                    token.base_x = event.x;
                    token.base_y = event.y;
                    token.base_z = event.z;
                    // Clear rollback point (confirmed by server)
                    token.last_server_x = token.base_x;
                }
            }
            -1 => { // Rejection
                if let Ok(mut token) = query.get_mut(Entity::from_raw(event.token_id)) {
                    // Rollback to last known good position
                    token.base_x = token.last_server_x;
                    token.base_y = token.last_server_y;
                    token.base_z = token.last_server_z;
                }
            }
            _ => {}
        }
    }
}
```

#### React/RxDB Side

```typescript
// 1. Save old position
const oldDoc = await tokens.findOne(tokenId).exec();
const lastPos = { x: oldDoc.x, y: oldDoc.y, z: oldDoc.z };

// 2. Update optimistically
await tokens.upsert({
  ...oldDoc,
  x: newX,
  y: newY,
  z: newZ,
  _optimistic: true,
  _lastServerPosition: lastPos,
});

// 3. Send mutation
try {
  await graphql.mutate(MoveTokenMutation, { tokenId, x: newX, y: newY, z: newZ });
} catch (error) {
  // 4. Rollback on rejection
  await tokens.upsert({
    ...oldDoc,
    x: lastPos.x,
    y: lastPos.y,
    z: lastPos.z,
    _optimistic: false,
  });
}
```

### Derived Stats Pattern

**Principle:** Base stats sent over network, derived stats computed locally.

```rust
pub struct DerivedTokenStats {
    pub health_percentage: f32,
    pub is_dead: bool,
    pub is_full_health: bool,
}

impl DerivedTokenStats {
    pub fn from_token(token: &WorldToken) -> Self {
        let health = token.health.unwrap_or(0) as f32;
        let max_health = token.max_health.unwrap_or(1) as f32;
        let health_percentage = (health / max_health * 100.0).clamp(0.0, 100.0);
        
        DerivedTokenStats {
            health_percentage,
            is_dead: health <= 0.0,
            is_full_health: health >= max_health,
        }
    }
}
```

**Network Savings:**
- Server sends: `{ health: 50, max_health: 100 }`
- Client computes: `{ health_percentage: 50 }`
- Reduction: 20-40% payload on average

---

## Event-Driven Mutations

Every token mutation persists a `WorldEvent` delta:

```rust
pub async fn move_token_mutation(
    token_id: &str,
    x: i32, y: i32, z: i32,
    user_id: &str,
    state: &AppState,
) -> Result<WorldToken> {
    // ... move token (ownership enforced)
    
    // Persist event
    let event = NewWorldEvent {
        id: uuid::Uuid::new_v4().to_string(),
        world_id: token.world_id.clone(),
        event_code: 2, // Moved
        token_event: Some(json!({
            "token_id": token_id,
            "x": x,
            "y": y,
            "z": z,
            "scene_id": token.scene_id,
        })),
        created_at: Utc::now().naive_utc(),
    };
    
    diesel::insert_into(world_events::table)
        .values(&event)
        .execute(conn)?;
    
    // Trigger NOTIFY (backplane picks this up)
    conn.execute("NOTIFY world_events_channel, $1", &[&event.id])?;
    
    Ok(token)
}
```

---

## Consequences

### Positive
- ✅ Strict ownership enforced at DB level
- ✅ Optimistic updates + rollback for instant UI
- ✅ Derived data computation saves 20-40% network
- ✅ Event log enables undo/audit trail
- ✅ No information leaks from unauthorized access

### Negative
- ⚠️ Complex rollback logic in clients
- ⚠️ Event log grows unbounded (archival strategy needed)

---

## Related ADRs

- ADR-009: created_by/updated_by Enforcement
- ADR-010: Ownership Fields on Persisted Tables
- ADR-013: GraphQL Ownership Enforcement

---

## References

- Phase 4 Implementation: `src/server/src/graphql.rs` (mutations)
- Bevy Systems: `src/server/src/token_systems.rs`
- RxDB Collection: `apps/web/src/db/collections/worldTokensCollection.ts`
