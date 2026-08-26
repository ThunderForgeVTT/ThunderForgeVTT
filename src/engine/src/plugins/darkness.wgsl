// Scene darkness, with light pools cut out of it.
//
// Drawn as one quad covering the map, above the background art and below
// tokens. Every fragment asks "how lit is this point?" and outputs darkness
// with the inverse alpha, so lit areas are transparent and unlit areas are
// opaque black. That is what lets an already-bright map be darkened.
//
// Occlusion is deliberately NOT computed here. Walls are drawn as separate
// solid-black shadow quads on top of this layer (see `plugins/darkness.rs`),
// which is far cheaper than marching every wall per fragment and composes
// correctly under ordinary alpha blending: black over a light pool is dark
// again.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Must match MAX_LIGHTS in plugins/darkness.rs. Fixed-size because WebGL2
// requires uniform arrays to have a compile-time length. The loop breaks at
// the live count, so a scene lighting three fragments costs three iterations
// regardless of this ceiling — and the CPU culls to what is on screen before
// anything is uploaded.
const MAX_LIGHTS: u32 = 128u;

struct Darkness {
    // rgb = ambient tint, a = how dark an unlit fragment is (0 = no darkness).
    ambient: vec4<f32>,
    // x = number of active lights.
    params: vec4<f32>,
    // xy = world position, z = bright radius, w = dim radius.
    lights: array<vec4<f32>, 128>,
    // rgb = light colour, a = intensity.
    light_colors: array<vec4<f32>, 128>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> darkness: Darkness;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world = mesh.world_position.xy;

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

        let distance = length(world - light.xy);
        let bright = light.z;
        let dim = max(light.w, bright);

        // Full inside the bright core, then a smooth ramp out to the dim
        // edge. The ramp is what makes a torch look like a torch instead of a
        // hard-edged disc.
        var contribution = 0.0;
        if (distance <= bright) {
            contribution = 1.0;
        } else if (distance <= dim) {
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
