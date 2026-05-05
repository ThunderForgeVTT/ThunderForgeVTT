# ADR-032: Canvas Rendering Strategy (Bevy)

**Date:** 2026-05-05  
**Status:** ACCEPTED  
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

ThunderForgeVTT needs a high-performance canvas engine for rendering:
- Grid layers (square, hexagonal, or gridless)
- Tokens as 2D sprites
- Fog of war (bitmap mask)
- Annotations (tldraw strokes)
- Player restrictions (view culling, permission masking)

Requirements:
1. Render 1000+ tokens without frame drops
2. Support pan/zoom interactions
3. Integrate seamlessly with React UI (WebAssembly boundary)
4. Execute in browser via WebAssembly

---

## Decision

Use **Bevy Engine (WASM)** as the canvas renderer with the following architecture:

### Architecture Overview

```
┌─────────────────────────────────────┐
│   Bevy WASM App (Canvas Engine)    │
├─────────────────────────────────────┤
│ Plugin Layer:                       │
│  • ScenePlugin (grid + camera)      │
│  • TokenPlugin (spawn + sync)       │
│  • FogPlugin (bitmap blending)      │
│  • AnnotationPlugin (tldraw)        │
│                                     │
│ System Layer:                       │
│  • load_scene_system                │
│  • camera_pan_zoom_system           │
│  • token_render_system              │
│  • token_selection_system           │
│  • fog_composite_system             │
│  • apply_server_updates_system      │
│  • mutation_processor_task          │
│                                     │
│ ECS Layer:                          │
│  • GridMesh component               │
│  • Transform (position, rotation)   │
│  • WorldTokenComponent              │
│  • FogMask component                │
│  • CameraConfig                     │
└─────────────────────────────────────┘
         ↕ WebAssembly Boundary
         
┌─────────────────────────────────────┐
│   React Frontend (UI Shell)         │
├─────────────────────────────────────┤
│ • useCanvasEngine hook              │
│ • <WorldWhiteboard /> wrapper       │
│ • TokenPanel component              │
│ • FogTools component                │
└─────────────────────────────────────┘
```

### Rendering Pipeline

**Layer Stack (Z-order)**
1. Background layer (scene image)
2. Grid layer (procedural mesh)
3. Token layer (sprite batch)
4. Annotation layer (tldraw strokes)
5. Fog layer (bitmap composite)
6. UI layer (React overlay)

### Plugin Architecture

#### 1. ScenePlugin

Loads scene metadata and builds grid mesh.

```rust
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_scene_system)
           .add_systems(Update, camera_pan_zoom_system);
    }
}

pub fn load_scene_system(
    mut commands: Commands,
    scene_data: Res<SceneData>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let grid_mesh = match scene_data.grid_type {
        GridType::Square => create_square_grid_mesh(&scene_data),
        GridType::Hexagonal => create_hex_grid_mesh(&scene_data),
        GridType::Gridless => Mesh::new(PrimitiveTopology::TriangleList),
    };
    
    commands.spawn(PbrBundle {
        mesh: meshes.add(grid_mesh),
        material: materials.add(Color::rgb(0.1, 0.1, 0.1).into()),
        ..default()
    });
}
```

#### 2. TokenPlugin

Spawns and synchronizes tokens with server.

```rust
pub struct TokenPlugin;

impl Plugin for TokenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_tokens_from_graphql)
           .add_systems(Update, (
               token_render_system,
               token_selection_system,
               movement_system,
               apply_server_updates_system,
               rollback_system,
           ).chain());
    }
}
```

#### 3. FogPlugin

Renders fog mask and handles GM reveal/hide.

```rust
pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fog_composite_system);
    }
}

pub fn fog_composite_system(
    fog_mask: Res<FogMaskTexture>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut query: Query<&mut Handle<FogMaterial>>,
) {
    // Composite fog mask over scene
    // Only visible to GM (players see different mask)
}
```

#### 4. AnnotationPlugin

Renders tldraw annotations (if integrated).

```rust
pub struct AnnotationPlugin;

impl Plugin for AnnotationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, annotation_render_system);
    }
}
```

### Grid Mesh Generation

#### Square Grid

```rust
fn create_square_grid_mesh(scene: &SceneData) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList);
    let mut positions = vec![];
    let mut indices = vec![];
    
    let width = scene.width_squares as f32 * scene.grid_size as f32;
    let height = scene.height_squares as f32 * scene.grid_size as f32;
    
    // Vertical lines
    for x in 0..=scene.width_squares {
        let px = x as f32 * scene.grid_size as f32;
        positions.push([px, 0.0, 0.0]);
        positions.push([px, height, 0.0]);
        indices.push((x * 2) as u32);
        indices.push((x * 2 + 1) as u32);
    }
    
    // Horizontal lines
    for y in 0..=scene.height_squares {
        let py = y as f32 * scene.grid_size as f32;
        positions.push([0.0, py, 0.0]);
        positions.push([width, py, 0.0]);
        // ... add indices
    }
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.set_indices(Some(Indices::U32(indices)));
    mesh
}
```

#### Hexagonal Grid

```rust
fn create_hex_grid_mesh(scene: &SceneData) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList);
    let hex_size = scene.grid_size as f32;
    
    for row in 0..scene.height_squares {
        for col in 0..scene.width_squares {
            let hex = create_hex_shape(col, row, hex_size);
            // Append hex outline to mesh
        }
    }
    
    mesh
}
```

### Camera System

```rust
pub struct CameraConfig {
    pub pan_speed: f32,
    pub zoom_speed: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
}

pub fn camera_pan_zoom_system(
    mut query: Query<&mut Transform, With<Camera>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mouse_wheel: EventReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    // Pan with middle-mouse drag or space+drag
    // Zoom with mouse wheel
    // Constrain to scene bounds
}
```

### Token Rendering

**Sprite Batching**
- Use a single SpriteBundle per token
- Batch sprites via material instancing for performance
- Cull tokens outside viewport + margin (e.g., 500px)

```rust
pub fn token_render_system(
    mut query: Query<(&WorldTokenComponent, &mut Transform, &mut Visibility)>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = camera.single();
    let viewport = camera.physical_viewport_rect();
    
    for (token, mut transform, mut visibility) in &mut query {
        let world_pos = Vec3::new(token.x as f32, token.y as f32, 10.0);
        
        // Check if in viewport + margin
        if is_in_viewport(world_pos, viewport, 500.0) {
            transform.translation = world_pos;
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden; // Culled
        }
    }
}
```

### Fog Rendering (WGSL Shader)

```wgsl
@group(1) @binding(0)
var fog_texture: texture_2d<f32>;

@group(1) @binding(1)
var fog_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv);
    let fog_alpha = textureSample(fog_texture, fog_sampler, in.uv).r;
    
    // Lerp between scene color and black based on fog
    let final_color = mix(scene_color.rgb, vec3<f32>(0.0), fog_alpha);
    return vec4<f32>(final_color, 1.0);
}
```

### Performance Optimizations

1. **Token Culling:** Only render visible tokens
2. **Mesh Instancing:** Batch identical grids via material instances
3. **Fog Bitmap:** Store as WebP, decompress once at load
4. **Lazy Loading:** Load background images on-demand
5. **Double Buffering:** Separate simulation and render frames

---

## Consequences

### Positive
- ✅ High-performance WebAssembly rendering
- ✅ Native browser APIs (WebGL/WebGPU via Bevy)
- ✅ Clean ECS architecture (systems composable)
- ✅ Integrates well with React (useCanvasEngine hook)
- ✅ Supports multiple grid types (square, hex, gridless)

### Negative
- ⚠️ Bevy WASM bundle size (~3MB gzipped)
- ⚠️ Learning curve for game engine patterns
- ⚠️ Limited IDE support for WASM debugging

---

## Alternatives Considered

### Alternative 1: PixiJS (2D graphics library)
- **Rejected:** Original design, but Bevy ECS model better for real-time sync
- **Trade-off:** Bevy heavier but more extensible for Phase 4+

### Alternative 2: Three.js (3D graphics)
- **Rejected:** Overkill for 2D grid-based VTT
- **Trade-off:** Three.js more flexible, but slower for this use case

### Alternative 3: Canvas 2D API
- **Rejected:** No ECS, harder to manage state
- **Trade-off:** Lighter bundle, but less scalable

---

## Related ADRs

- ADR-000: Fantasy UI Shell with Radix & Wrapped tldraw
- ADR-031: Scene Domain Model

---

## Implementation Checklist

- [ ] Bevy app initialization + WebAssembly setup
- [ ] ScenePlugin with grid mesh generation
- [ ] Camera pan/zoom system
- [ ] TokenPlugin integration
- [ ] Token culling + viewport management
- [ ] Fog composite system
- [ ] WGSL fog shader
- [ ] React useCanvasEngine hook
- [ ] Performance profiling + optimization
- [ ] Integration tests

---

## References

- Bevy docs: https://bevyengine.org/
- WASM in Bevy: https://bevyengine.org/learn/book/getting-started/setup/
- Comparable: Foundry VTT (EaselJS canvas), Roll20 (WebGL)
