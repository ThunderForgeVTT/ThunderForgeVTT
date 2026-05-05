# ADR-031: Scene Domain Model

**Date:** 2026-05-05  
**Status:** ACCEPTED  
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Scenes are the core spatial unit where tokens are placed and actions occur. We need to define:

1. What data does a scene persist?
2. How does a scene relate to worlds, grids, and rendering?
3. Who can create, edit, and delete scenes?
4. How do scenes scale (performance) when thousands exist?

---

## Decision

Scenes are **first-class entities** in ThunderForgeVTT with the following architecture:

### Data Model

```rust
pub struct Scene {
    pub scene_id: String,              // UUID
    pub world_id: String,              // Foreign key to worlds
    pub name: String,
    pub description: Option<String>,
    pub background_image_url: Option<String>,
    pub grid_type: GridType,           // Enum: Square, Hexagonal, Gridless
    pub grid_size: i32,                // Pixels per grid square
    pub width_squares: i32,            // Grid width
    pub height_squares: i32,           // Grid height
    pub metadata: serde_json::Value,   // Fog config, layers, etc.
    pub created_by: String,            // User who created (enforced at DB level)
    pub updated_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

pub enum GridType {
    Square,
    Hexagonal,
    Gridless,
}
```

### Ownership & Authorization

- **Scene Creator:** Automatically assigned via session middleware (never client input)
- **World Owner:** Inherits edit/delete permissions on all scenes in their world
- **Admin:** Can access/modify any scene
- **Players:** Read-only view of scenes they have permission to enter

Database query enforces ownership:
```sql
SELECT * FROM scenes WHERE scene_id = $1 AND (created_by = $2 OR $3 = 'admin')
UPDATE scenes SET ... WHERE scene_id = $1 AND (created_by = $2 OR $3 = 'admin')
```

### GraphQL API

```graphql
type Scene {
  sceneId: ID!
  worldId: ID!
  name: String!
  description: String
  backgroundImageUrl: String
  gridType: GridType!
  gridSize: Int!
  widthSquares: Int!
  heightSquares: Int!
  createdBy: ID!
  updatedBy: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
  tokens(limit: Int, offset: Int): [Token!]!
  fogMask: FogMask
}

type Query {
  scene(sceneId: ID!): Scene
  scenes(worldId: ID!, limit: Int, offset: Int): [Scene!]!
}

type Mutation {
  createScene(worldId: ID!, name: String!, gridType: GridType!, gridSize: Int!): Scene!
  updateScene(sceneId: ID!, patch: SceneUpdate!): Scene!
  deleteScene(sceneId: ID!): Boolean!
}

input SceneUpdate {
  name: String
  description: String
  backgroundImageUrl: String
  gridType: GridType
  gridSize: Int
  widthSquares: Int
  heightSquares: Int
}
```

### Persistence

**Diesel Schema**
```rust
table! {
    scenes (scene_id) {
        scene_id -> Text,
        world_id -> Text,
        name -> Text,
        description -> Nullable<Text>,
        background_image_url -> Nullable<Text>,
        grid_type -> Text,        // "Square" | "Hexagonal" | "Gridless"
        grid_size -> Int4,
        width_squares -> Int4,
        height_squares -> Int4,
        metadata -> Jsonb,
        created_by -> Text,
        updated_by -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
```

**Indexes**
```sql
CREATE INDEX idx_scenes_world_id ON scenes(world_id);
CREATE INDEX idx_scenes_created_by ON scenes(created_by);
CREATE INDEX idx_scenes_world_created_at ON scenes(world_id, created_at DESC);
```

### Bevy Rendering

Scenes are loaded into the Bevy scene via a **ScenePlugin**:

1. Query scene from GraphQL (fetch metadata, grid config, token list)
2. Spawn grid mesh based on `grid_type` and `grid_size`
3. Spawn camera centered on scene bounds
4. Load background image as sprite layer
5. Spawn tokens as entities (via token_systems.rs)
6. Subscribe to `worldEventCreated` for real-time token updates

```rust
pub fn load_scene_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    scene_query: Query<&CurrentScene>,
) {
    if let Ok(current_scene) = scene_query.get_single() {
        let mesh = match current_scene.grid_type {
            GridType::Square => create_square_grid_mesh(&current_scene),
            GridType::Hexagonal => create_hex_grid_mesh(&current_scene),
            GridType::Gridless => create_blank_mesh(),
        };
        commands.spawn(PbrBundle { mesh: meshes.add(mesh), ... });
    }
}
```

### React Frontend

Scene management is divided into two contexts:

1. **GM View** (`/worlds/:worldId/scenes/:sceneId`)
   - Scene editor (grid config, background image)
   - Token management
   - Fog of war editing
   - Linked to Bevy via WebAssembly boundary

2. **Player View** (`/play/:worldId/:sceneId`)
   - Read-only rendering
   - Fog-aware visibility
   - Token movement (limited to owned tokens)

**Scene List Component**
```tsx
function SceneList({ worldId }) {
  const { data } = useQuery(ListScenesQuery, { variables: { worldId } });
  return (
    <div>
      {data.scenes.map(scene => (
        <SceneCard key={scene.sceneId} scene={scene} />
      ))}
    </div>
  );
}
```

### Performance Considerations

- **Limit scene complexity:** Max 10,000 tokens per scene (raise after profiling)
- **Grid mesh baking:** Pre-generate and cache grid meshes
- **Token culling:** Only render tokens within viewport + margin
- **Fog bitmap:** Store as compressed WebP, decompress on demand

---

## Consequences

### Positive
- ✅ Clear separation of scenes (multiple scenes per world)
- ✅ Grid metadata supports diverse game types
- ✅ RxDB can cache scene list locally
- ✅ Bevy rendering pipeline clean and extensible

### Negative
- ⚠️ Requires UI for scene management (out of Phase 4 scope)
- ⚠️ Grid mesh generation adds startup latency
- ⚠️ Token culling needed for performance at scale

---

## Alternatives Considered

### Alternative 1: Single Scene per World
- **Con:** Limits game design flexibility; some VTTs have multiple maps
- **Rejected:** Too restrictive

### Alternative 2: Scene hierarchy (rooms inside scenes)
- **Con:** Adds complexity without MVP necessity
- **Deferred:** Future expansion after Phase 4

---

## Related ADRs

- ADR-010: Ownership Fields on Persisted Tables
- ADR-013: GraphQL Ownership Enforcement
- ADR-032: Canvas Rendering Strategy (Bevy)

---

## Implementation Checklist

- [ ] Diesel schema + migrations for `scenes` table
- [ ] GraphQL Scene type + resolvers
- [ ] Axum CRUD endpoints
- [ ] React Scene List + Create dialog
- [ ] Bevy ScenePlugin + load_scene_system
- [ ] Grid mesh generation (square + hex)
- [ ] Integration tests

---

## References

- Comparable systems: Foundry VTT (scenes), Roll20 (pages)
- Related docs: `src/server/src/models.rs` (Scene model)
