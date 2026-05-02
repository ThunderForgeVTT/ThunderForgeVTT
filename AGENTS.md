# ThunderForgeVTT Agent Guidelines

## Stack Overview

ThunderForgeVTT is a robust multiplayer virtual tabletop system combining:

- **Backend**: Rust/Axum (HTTP + WebSocket server)
- **Database**: PostgreSQL + Diesel ORM (durable persistence + pub/sub via NOTIFY/LISTEN)
- **Game Engine**: Bevy (WebAssembly, ECS-based simulation and rendering)
- **Frontend**: React + RxDB (UI, real-time sync, offline-first caching)
- **Graphics**: tldraw (collaborative drawing and canvas)

This architecture mirrors enterprise multiplayer systems like **Figma** and **PlayCanvas**, where a central server acts as the authority while thick clients handle presentation and optimistic updates.

---

## Core Architectural Principles

### 1. Circular Event-Driven Data Flow

**Never** treat frontend (React/tldraw) or engine (Bevy) as the final source of truth. Enforce strict circular flow:

```
┌─────────────────────────────────────────────────────────┐
│ 1. USER ACTION (tldraw stroke or Bevy token move)       │
└──────────────┬──────────────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────────────┐
│ 2. MUTATION (GraphQL mutation sent to Axum)             │
│    Example: upsertToken({ id, x, y, z, label })        │
└──────────────┬──────────────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────────────┐
│ 3. ADJUDICATION (Server validates + persists)           │
│    • Check permissions, bounds, turn order              │
│    • Persist to PostgreSQL via Diesel                   │
│    • Create WorldEvent delta record                     │
│    • NOTIFY world_events_channel with event_id          │
└──────────────┬──────────────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────────────┐
│ 4. BROADCAST (Server broadcasts via subscriptions)      │
│    • PostgreSQL NOTIFY fans out via Tokio broadcast     │
│    • WebSocket subscription: worldEventCreated          │
│    • Payload: { id, world_id, event_code, token_event} │
└──────────────┬──────────────────────────────────────────┘
               ↓
┌──────────────┴──────────────┬──────────────────────────┐
│                             │                          │
▼                             ▼                          ▼
BEVY SYNC              RXDB SYNC              (Other clients)
• Receive delta        • Receive delta
• Apply to ECS         • Update collections
• Render canvas        • React re-renders
```

**Key Rule**: Clients always render from server-approved state, never their local optimistic version as final truth.

---

### 2. Isolate tldraw and Bevy from Network

#### For React/tldraw

- **Do NOT**: Import GraphQL client directly in tldraw components
- **Do**: Let RxDB act as the single source of truth
  - RxDB collections mirror PostgreSQL tables
  - RxDB replication plugin listens to `worldEventCreated` subscription
  - tldraw components query RxDB directly
  - User drawings → RxDB mutation → GraphQL mutation → Server

```typescript
// ❌ Wrong
export const DrawingComponent = () => {
  const [drawing, setDrawing] = useState();
  const graphql = useGraphQL(); // Tightly coupled!
  const handleDraw = (shape) => {
    setDrawing(shape);
    await graphql.mutation(UpsertShape, { shape }); // Network logic in component
  };
};

// ✅ Right
export const DrawingComponent = () => {
  const drawing$ = useRxDB('drawings').find().sort('_id');
  const handleDraw = (shape) => {
    drawing$.insert({ ...shape, _id: uuid() }); // RxDB handles everything
    // RxDB replication → GraphQL mutation → Server → NOTIFY → other clients
  };
};
```

#### For Bevy

- **Do NOT**: Spawn WebSocket tasks inside game systems
- **Do**: Abstract network behind a GraphQL client wrapper
  - Engine emits "commands" to a mutation queue
  - Engine listens for "events" from subscription channel
  - ECS systems focus purely on simulation and rendering

```rust
// ❌ Wrong
pub fn handle_input_system(
    query: Query<&Transform, With<Token>>,
    mut websocket: ResMut<WebSocket>, // WebSocket logic in systems!
) {
    if let Some(input) = input_receiver.try_recv() {
        websocket.send_mutation(...).await; // Async in system
    }
}

// ✅ Right
pub fn handle_input_system(
    query: Query<&Transform, With<Token>>,
    mut mutation_queue: ResMut<GraphQLMutationQueue>, // Abstract queue
) {
    if let Some(input) = input_receiver.try_recv() {
        mutation_queue.push(UpsertTokenMutation { ... }); // Enqueue only
    }
}

pub async fn mutation_processor_task(mut mutation_queue: ResMut<GraphQLMutationQueue>) {
    loop {
        if let Some(mutation) = mutation_queue.pop() {
            graphql_client.mutate(mutation).await; // Network in dedicated task
        }
    }
}
```

---

### 3. Axum and Tower Middleware Management

#### ServiceBuilder Pattern

Stack middleware cleanly using `tower::ServiceBuilder`:

```rust
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/graphql", post(graphql_handler))
    .route("/graphql/ws", get(graphql_ws_handler))
    .layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
            .layer(LoadSheddingMiddleware::new(MAX_QUEUE_DEPTH))
            .into_inner(),
    );
```

#### Load Shedding Middleware

Axum does not play nicely with backpressure. If GraphQL mutations pile up, requests will exhaust memory and crash:

```rust
pub struct LoadSheddingMiddleware {
    max_queue_depth: usize,
    current_depth: Arc<AtomicUsize>,
}

#[async_trait]
impl Middleware<BoxFuture<'static, Response>> for LoadSheddingMiddleware {
    async fn process(&self, req: Request) -> Result<Response, Error> {
        let depth = self.current_depth.load(Ordering::Acquire);
        if depth > self.max_queue_depth {
            // Drop request immediately instead of queuing
            return Err(StatusCode::SERVICE_UNAVAILABLE.into());
        }
        // Process request normally
        Ok(next.run(req).await)
    }
}
```

**Key Rule**: Drop overloaded requests fast, don't queue them. Better a client sees a 503 and retries than the entire server becomes unresponsive.

---

### 4. Optimistic Updates with Rollback

Since server round-trip latency is 50-200ms, users must see feedback immediately:

#### Bevy Optimistic Updates

```rust
pub fn handle_move_token(
    mut commands: Commands,
    mut query: Query<&mut Transform, With<Token>>,
    mut mutation_queue: ResMut<GraphQLMutationQueue>,
) {
    if let Ok(mut transform) = query.get_single_mut() {
        let old_pos = transform.translation;
        
        // 1. Update locally (optimistic)
        transform.translation = new_pos;
        
        // 2. Queue mutation to server
        mutation_queue.push_with_rollback(
            MoveTokenMutation { ... },
            Box::new(move |success: bool| {
                if !success {
                    // Rejection! Rollback to old position
                    transform.translation = old_pos;
                }
            }),
        );
    }
}
```

#### React/RxDB Optimistic Updates

```typescript
export const handleTokenMove = async (tokenId: string, newPos: { x, y }) => {
  const tokens = db.collections.world_tokens;
  const oldDoc = await tokens.findByIds([tokenId]);
  
  // 1. Update locally (optimistic)
  await tokens.upsert({
    ...oldDoc[0],
    x: newPos.x,
    y: newPos.y,
  });
  
  // 2. Send mutation to server
  try {
    await graphql.mutate(UpsertTokenMutation, { tokenId, ...newPos });
  } catch (error) {
    // 3. Rollback on rejection
    await tokens.upsert(oldDoc[0]);
    showErrorNotification(`Invalid move: ${error.message}`);
  }
};
```

**Key Rule**: Always cache the pre-mutation state. On rejection, restore it immediately.

---

### 5. Base Data vs. Derived Data

Keep GraphQL payloads small by never transmitting calculated values:

#### Example: Token with Buffs

```rust
// Core Model (transmitted over network)
#[derive(Serialize, Deserialize)]
pub struct WorldToken {
    // Base data (always sent)
    pub id: String,
    pub health: i32,
    pub max_health: i32,
    pub strength: i32,
    pub temporary_buff_strength: i32,
    
    // Derived data (calculated locally, never sent)
    #[serde(skip)]
    pub effective_strength: i32,
    #[serde(skip)]
    pub health_percentage: f32,
}

impl WorldToken {
    pub fn prepare_derived_data(&mut self) {
        self.effective_strength = self.strength + self.temporary_buff_strength;
        self.health_percentage = (self.health as f32 / self.max_health as f32) * 100.0;
    }
}
```

**In Bevy:**
```rust
pub fn calculate_stats_system(mut query: Query<&mut WorldToken>) {
    for mut token in query.iter_mut() {
        token.prepare_derived_data();
    }
}
```

**In React:**
```typescript
const token = useRxDB('world_tokens').findOne(id);
token.prepareDerivedData(); // Calculation on client side
return <TokenDisplay strength={token.effectiveStrength} />;
```

**Benefit**: 20-40% reduction in payload size. Consistency guaranteed because both Bevy and React run identical calculation logic.

---

### 6. PostgreSQL NOTIFY/LISTEN as Pub/Sub Backplane

Using PostgreSQL's native NOTIFY/LISTEN eliminates external dependencies and leverages your existing database connection pool.

#### Architecture Diagram

```
┌────────────────────────────────────┐
│  Axum Server Instance #1           │
├────────────────────────────────────┤
│ Tokio LISTEN Task                  │
│ └─ Single PG connection            │
│    LISTEN world_events_channel     │
└────┬─────────────────────────────┬─┘
     │ notification                │
     ↓                             │
┌─────────────────────────┐        │
│ Tokio broadcast::channel│◄───────┘
│ (memory-cheap)          │
└──────┬──────────────────┘
       │ subscribe
       ├──→ WebSocket #1 → Bevy Instance #1
       ├──→ WebSocket #2 → React Browser
       └──→ WebSocket #3 → Bevy Instance #2
```

#### Key Implementation Guidelines

##### 1. Multiplex Single LISTEN Connection

```rust
// src/server/src/pubsub/mod.rs

pub struct PubSubBackplane {
    broadcast_tx: broadcast::Sender<WorldEvent>,
}

impl PubSubBackplane {
    pub async fn spawn_listener(
        pool: DbPool,
    ) -> Result<broadcast::Receiver<WorldEvent>> {
        let (tx, _rx) = broadcast::channel(1000);
        let tx_clone = tx.clone();
        
        // Single Tokio task, single PG connection
        tokio::spawn(async move {
            let mut conn = pool.get().await?;
            conn.execute("LISTEN world_events_channel")?;
            
            // Loop receives notifications
            while let Some(notification) = conn.recv_notification().await {
                if let Ok(event_id) = notification.payload.parse::<i64>() {
                    // Query DB for full payload (indexed!)
                    let event = query_world_event(pool.clone(), event_id).await?;
                    let _ = tx_clone.send(event); // Fire and forget
                }
            }
        });
        
        Ok(tx.subscribe())
    }
}
```

**Why single connection?** PG connections are expensive. Multiplex via Tokio broadcast instead.

##### 2. Respect 8KB Payload Limit

PostgreSQL restricts NOTIFY payloads to 8,000 bytes. For large events, send only the ID:

```rust
// ❌ Wrong: Sends entire JSON
pub async fn create_world_event(
    pool: &DbPool,
    event: &WorldEvent,
) -> Result<()> {
    let mut conn = pool.get().await?;
    
    // Insert event
    let event_id = diesel::insert_into(world_events::table)
        .values(event)
        .returning(world_events::id)
        .get_result::<i64>(&mut conn)?;
    
    // Notify with full JSON (might exceed 8KB!)
    let payload = serde_json::to_string(event)?;
    conn.execute(&format!("NOTIFY world_events_channel, '{}'", payload))?;
    Ok(())
}

// ✅ Right: Sends only event_id
pub async fn create_world_event(
    pool: &DbPool,
    event: &WorldEvent,
) -> Result<()> {
    let mut conn = pool.get().await?;
    
    // Insert event
    let event_id = diesel::insert_into(world_events::table)
        .values(event)
        .returning(world_events::id)
        .get_result::<i64>(&mut conn)?;
    
    // Notify with only the ID (always <8KB)
    conn.execute(&format!("NOTIFY world_events_channel, '{}'", event_id))?;
    
    // Listener will SELECT event by ID when it receives the notification
    Ok(())
}
```

##### 3. Handle Dropped Connections

pg_notify is "fire and forget". If a server loses its PG connection, clients miss events:

```rust
// On new WebSocket subscription, fetch missed events
pub async fn setup_websocket_subscription(
    last_event_id: i64,
    pool: &DbPool,
    mut pubsub_rx: broadcast::Receiver<WorldEvent>,
) -> impl Stream<Item = WorldEvent> {
    // 1. Fetch all events since last_event_id
    let backlog = query_world_events_since(pool, last_event_id)
        .await
        .unwrap_or_default();
    
    // 2. Yield backlog first
    let backlog_stream = futures::stream::iter(backlog);
    
    // 3. Then subscribe to live notifications
    let live_stream = tokio_stream::wrappers::BroadcastStream::new(pubsub_rx);
    
    // 4. Combine: backlog then live
    backlog_stream.chain(live_stream)
}
```

**Client-side** (Bevy):
```rust
pub async fn subscribe_world_events(
    graphql_client: &GraphQLClient,
    mut last_event_id: i64,
) -> impl Stream<Item = WorldEvent> {
    graphql_client.subscribe(
        WORLD_EVENTS_SUBSCRIPTION,
        json!({ "last_event_id": last_event_id }),
    ).await
}
```

##### 4. Trigger NOTIFY at Database Level (Optional)

Instead of relying on Rust code to NOTIFY, use a PostgreSQL trigger to guarantee consistency:

```sql
-- Create function to fire notification
CREATE OR REPLACE FUNCTION notify_world_event()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'world_events_channel',
    NEW.id::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Attach trigger to world_events table
CREATE TRIGGER world_events_notify_trigger
AFTER INSERT ON world_events
FOR EACH ROW
EXECUTE FUNCTION notify_world_event();
```

**Benefit**: Even manual database updates or migrations trigger the backplane automatically.

---

## Common Patterns & Anti-Patterns

### ✅ Do's

- ✅ **Circular Flow**: Every state change flows through server
- ✅ **Optimistic UI**: Update immediately, validate async
- ✅ **Abstract Network**: Engine/UI never see raw WebSockets
- ✅ **Audit Trail**: Store every mutation as WorldEvent
- ✅ **Load Shed**: Drop requests under overload, don't queue
- ✅ **Derived Data**: Calculate stats locally, send only base values
- ✅ **Backpressure**: Single PG LISTEN, multiplex via Tokio broadcast

### ❌ Don'ts

- ❌ **Direct Writes**: Don't let frontend/engine write local state as "truth"
- ❌ **Missing Rollback**: Optimistic updates without rejection handling
- ❌ **Network in Systems**: Don't spawn WebSocket tasks in Bevy systems
- ❌ **Large Payloads**: Don't send derived/cached data over network
- ❌ **Queue Overload**: Don't buffer requests when system is saturated
- ❌ **Multiple LISTEN Connections**: Don't open one PG connection per client
- ❌ **Ignoring 8KB Limit**: Don't send full JSON in NOTIFY without chunking

---

## Implementation Checklist

- [ ] Shared core models (no Diesel/network deps) in `src/core/src/models/`
- [ ] Adapter layer (Diesel ↔ Core conversion) in `src/server/src/adapters.rs`
- [ ] PostgreSQL pub/sub backplane in `src/server/src/pubsub/`
- [ ] Axum load shedding middleware in `src/server/src/middleware/`
- [ ] GraphQL mutations for all game actions
- [ ] GraphQL subscriptions for real-time sync
- [ ] Bevy GraphQL client abstraction
- [ ] Bevy optimistic update + rollback system
- [ ] RxDB collections and replication setup
- [ ] React optimistic update + rollback hooks
- [ ] `prepareDerivedData()` in all data models
- [ ] Database trigger for NOTIFY on world_events insert
- [ ] Catch-up logic for dropped connections
- [ ] End-to-end test: Action → Mutation → Persist → NOTIFY → Sync

---

## Troubleshooting

### WebSocket Connection Dropped

**Symptom**: Clients see old state after reconnect

**Solution**: Implement catch-up query in subscription setup:
```rust
// On reconnect, query events since last_event_id
let backlog = db.query_world_events_since(last_event_id).await?;
send_backlog_to_client(backlog).await?;
```

### Server Feels Sluggish / Mutation Latency High

**Symptom**: Tokens move slowly, 2+ second delays

**Solution**: 
1. Check load shedding middleware is active
2. Verify single LISTEN connection (not one per client)
3. Profile Diesel queries for missing indexes
4. Add query tracing to identify bottlenecks

### State Conflicts Between Bevy and RxDB

**Symptom**: Engine shows token at (10, 20), frontend shows (15, 25)

**Solution**:
1. Verify both are listening to same `worldEventCreated` subscription
2. Check optimistic rollback logic fires on mutation rejection
3. Ensure schema versions match between engine and frontend
4. Debug: log all mutations and events to trace divergence

### NOTIFY Messages Lost Under Heavy Load

**Symptom**: Occasional state divergence during rapid moves

**Solution**:
1. Increase `broadcast::channel` buffer size: `broadcast::channel(10000)` instead of 1000
2. Reduce GraphQL subscription overhead (check derived data calculations)
3. Profile PG LISTEN loop: ensure no blocking operations
4. Add monitoring: log NOTIFY events vs received events

---

## Related Documentation

- [ADR-000: Durable Objects via GraphQL Event-Driven Sync](docs/adrs/20260501-000-durable_objects_with_graphql_event_driven_sync.md)
- [Implementation Guide](docs/IMPLEMENTATION_GUIDE.md)
- [Core Models](src/core/src/models/)
- [Server Adapters](src/server/src/adapters.rs)

---

## References

- [Figma's Multiplayer Technology](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/)
- [Bevy ECS Book](https://bevyengine.org/learn/book/introduction/)
- [PostgreSQL NOTIFY/LISTEN](https://www.postgresql.org/docs/current/sql-notify.html)
- [Axum Web Framework](https://github.com/tokio-rs/axum)
- [RxDB Replication](https://rxdb.info/replication.html)
- [Event Sourcing Pattern](https://martinfowler.com/eaaDev/EventSourcing.html)
