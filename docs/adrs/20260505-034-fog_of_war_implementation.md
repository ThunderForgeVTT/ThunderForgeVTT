# ADR-034: Fog of War Implementation

**Date:** 2026-05-05  
**Status:** ACCEPTED  
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Fog of War is critical for VTT gameplay:
- GMs must control what players see
- Fog state must persist per-scene
- Real-time sync across all clients
- Performance: 1000+ pixel bitmap masks

---

## Decision

Implement fog as a **bitmap mask** stored in PostgreSQL and rendered via WGSL shader.

### Diesel Schema

```rust
#[derive(Queryable, Selectable, Insertable, AsChangeset, Serialize, Deserialize)]
#[diesel(table_name = fog_masks)]
pub struct FogMask {
    pub id: String,                    // UUID
    pub scene_id: String,              // Foreign key
    pub bitmap_data: Vec<u8>,          // WebP compressed
    pub width_px: i32,
    pub height_px: i32,
    pub created_by: String,            // GM who last edited
    pub updated_at: NaiveDateTime,
}
```

### GraphQL API

```graphql
type Mutation {
  # Reveal fog in rectangular region
  revealFog(sceneId: ID!, x: Int!, y: Int!, width: Int!, height: Int!): Boolean!
  
  # Hide fog in rectangular region
  hideFog(sceneId: ID!, x: Int!, y: Int!, width: Int!, height: Int!): Boolean!
  
  # Clear all fog
  clearFog(sceneId: ID!): Boolean!
  
  # Paint fog with brush
  paintFog(sceneId: ID!, x: Int!, y: Int!, radius: Int!, opacity: Float!): Boolean!
}

type Subscription {
  fogUpdated(sceneId: ID!): FogEvent!
}

type FogEvent {
  sceneId: ID!
  action: String!  # "reveal", "hide", "paint", "clear"
  timestamp: DateTime!
}
```

### Rendering (Bevy + WGSL)

**WGSL Fragment Shader**
```wgsl
@group(1) @binding(0)
var fog_texture: texture_2d<f32>;

@group(1) @binding(1)
var fog_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv);
    let fog_alpha = textureSample(fog_texture, fog_sampler, in.uv).r;
    
    // Fog regions (alpha > 0.5) render as black
    let final_color = mix(scene_color.rgb, vec3<f32>(0.0), fog_alpha);
    return vec4<f32>(final_color, 1.0);
}
```

**Bevy System**
```rust
pub fn fog_composite_system(
    fog_texture: Res<FogTexture>,
    mut query: Query<&mut Handle<StandardMaterial>>,
) {
    // Update fog material binding with latest bitmap
    for material_handle in &mut query {
        // Composite fog over scene
    }
}
```

### GM Reveal/Hide Tools

React component for fog editing:
```tsx
function FogTools({ sceneId }) {
  const revealBrush = () => {
    // Draw revealed area
    graphql.mutate(RevealFogMutation, { sceneId, x, y, width, height });
  };
  
  const hideBrush = () => {
    graphql.mutate(HideFogMutation, { sceneId, x, y, width, height });
  };
  
  return (
    <div>
      <button onClick={revealBrush}>Reveal (R)</button>
      <button onClick={hideBrush}>Hide (H)</button>
    </div>
  );
}
```

### Player Visibility Filtering

**Backend Query**
```rust
pub async fn get_tokens_for_player(
    scene_id: &str,
    player_id: &str,
    fog_mask: &FogMask,
) -> Result<Vec<WorldToken>> {
    let tokens = world_tokens::table
        .filter(world_tokens::scene_id.eq(scene_id))
        .load::<WorldToken>(conn)?;
    
    // Filter tokens to only those in revealed fog regions
    let visible: Vec<_> = tokens
        .into_iter()
        .filter(|t| is_token_visible(t, fog_mask))
        .collect();
    
    Ok(visible)
}

fn is_token_visible(token: &WorldToken, fog_mask: &FogMask) -> bool {
    // Sample fog bitmap at token position
    // Return true if alpha < 0.5 (revealed)
    let px = (token.base_x * fog_mask.width_px / SCENE_WIDTH) as usize;
    let py = (token.base_y * fog_mask.height_px / SCENE_HEIGHT) as usize;
    let alpha = sample_fog_bitmap(&fog_mask.bitmap_data, px, py);
    alpha < 0.5
}
```

### Performance Optimization

1. **Compression:** Store fog bitmap as WebP (95% reduction)
2. **Lazy Decode:** Decompress on-demand, cache in Bevy
3. **Dirty Regions:** Only re-render affected areas
4. **Subscription Throttling:** Batch fog updates (max 1/100ms)

---

## Consequences

### Positive
- ✅ Bitmap approach scales to any resolution
- ✅ WGSL shader integrates with Bevy rendering
- ✅ WebP compression reduces storage/bandwidth
- ✅ Real-time sync via pg_notify backplane

### Negative
- ⚠️ Bitmap resolution limits precision (16px granularity recommended)
- ⚠️ High-frequency painting causes bandwidth spike

---

## Related ADRs

- ADR-032: Canvas Rendering Strategy (Bevy)
- ADR-035: Player View Architecture

---

## References

- WebP format: https://developers.google.com/speed/webp
- Bevy rendering: https://bevyengine.org/
