// Scene darkness, with light pools cut out of it — each light stopped by its
// own walls.
//
// Drawn as one quad covering the map, above the map art, grid and shapes and
// below tokens. Every fragment asks "how lit is this point?" and outputs
// darkness with the inverse alpha, so lit areas are transparent and unlit
// areas are dark. That is what lets an already-bright map be darkened.
//
// Occlusion is per light (playtest 2026-09-10 P9). Each light has a row in
// `shadow_map`: for each of SHADOW_BINS directions, how far the light reaches
// before a wall stops it (`thunderforge_canvas_core::vision::shadow_map_row`).
// A fragment further than that along its direction gets nothing from that
// light — and only from that light, so a room lit from both sides stays lit
// behind each wall. Shadows used to be quads painted over the whole layer,
// which darkened every light's pool alike.
//
// Sight (spec 076): when this client looks through a token, `sight` names
// where it stands and row SIGHT_ROW holds how far it sees in each direction.
// A fragment past that is unseen — drawn black, whatever lights fall on it —
// in a bright scene as in a dark one. Light says how far and how well; walls
// say where at all.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Must match MAX_LIGHTS and SHADOW_BINS in plugins/darkness.rs. Fixed-size
// because WebGL2 requires uniform arrays to have a compile-time length. The
// loop breaks at the live count, so a scene lighting three fragments costs
// three iterations regardless of this ceiling.
const MAX_LIGHTS: u32 = 128u;
const SHADOW_BINS: u32 = 512u;
// Must match SIGHT_ROW in plugins/darkness.rs: the row after the lights'.
const SIGHT_ROW: u32 = 128u;
// What the unseen is drawn as: black, opaque (owner decision 2026-10-06).
// Not the scene's own darkness tint — "not there" is a different thing from
// "dark", and reads as one. What exploration remembers draws above this.
const UNSEEN: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;

struct Darkness {
    // rgb = ambient tint, a = how dark an unlit fragment is (0 = no darkness).
    ambient: vec4<f32>,
    // x = number of active lights.
    params: vec4<f32>,
    // xy = the viewer's position, z = the sight row's reach, w = 1 while
    // looking through a token.
    sight: vec4<f32>,
    // xy = world position, z = bright radius, w = dim radius.
    lights: array<vec4<f32>, 128>,
    // rgb = light colour, a = intensity.
    light_colors: array<vec4<f32>, 128>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> darkness: Darkness;
// One row per light, one texel per direction: the reach, as a fraction of the
// light's dim radius, packed into 16 bits across red (high) and green (low).
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var shadow_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var shadow_sampler: sampler;

// How far light `i` reaches in the direction of `offset`, in world units.
fn reach(i: u32, offset: vec2<f32>, dim: f32) -> f32 {
    let angle = atan2(offset.y, offset.x);
    let bin = min(u32(floor((angle + PI) / TAU * f32(SHADOW_BINS))), SHADOW_BINS - 1u);
    let texel = textureLoad(shadow_map, vec2<i32>(i32(bin), i32(i)), 0);
    let packed = texel.r * 255.0 * 256.0 + texel.g * 255.0;
    return packed / 65535.0 * dim;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world = mesh.world_position.xy;

    // Out of the viewer's sight: unseen, before any light is asked.
    if (darkness.sight.w > 0.0) {
        let offset = world - darkness.sight.xy;
        if (length(offset) > reach(SIGHT_ROW, offset, darkness.sight.z) + 1.0) {
            return UNSEEN;
        }
    }

    // How much this fragment is lit, 0..1. Lights combine with `max`, never by
    // adding: two overlapping dim lights do not make bright light. This
    // mirrors `thunderforge_canvas_core::vision::illumination_at`, which is
    // the tested definition the rest of the engine uses.
    var lit = 0.0;
    var tint = vec3<f32>(1.0, 1.0, 1.0);

    let count = u32(darkness.params.x);
    for (var i = 0u; i < MAX_LIGHTS; i = i + 1u) {
        if (i >= count) {
            break;
        }

        let light = darkness.lights[i];
        let color = darkness.light_colors[i];
        if (color.a <= 0.0) {
            continue;
        }

        let offset = world - light.xy;
        let distance = length(offset);
        let bright = light.z;
        let dim = max(light.w, bright);
        if (distance > dim) {
            continue;
        }
        // Behind one of this light's walls: nothing from this light. The
        // small margin keeps the wall's own face lit rather than flickering.
        if (distance > reach(i, offset, dim) + 1.0) {
            continue;
        }

        // Full inside the bright core, then a smooth ramp out to the dim
        // edge. The ramp is what makes a torch look like a torch instead of a
        // hard-edged disc.
        var contribution = 1.0;
        if (distance > bright) {
            contribution = 1.0 - (distance - bright) / max(dim - bright, 0.0001);
        }

        contribution = contribution * clamp(color.a, 0.0, 1.0);

        if (contribution > lit) {
            lit = contribution;
            tint = color.rgb;
        }
    }

    // Unlit fragments get the full ambient darkness; fully lit ones get none.
    let alpha = darkness.ambient.a * (1.0 - clamp(lit, 0.0, 1.0));

    // The darkness itself carries the ambient tint (moonlight blue, say), and
    // warms toward a light's own colour where that light dominates.
    let color = mix(darkness.ambient.rgb, tint, clamp(lit, 0.0, 1.0) * 0.5);

    return vec4<f32>(color, alpha);
}
