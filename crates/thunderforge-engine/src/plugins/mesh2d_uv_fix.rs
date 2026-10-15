//! Mesh2d texture coordinates on Adreno phones.
//!
//! Bevy 0.20's 2D mesh vertex shader copies the whole `MeshMetadata` uniform
//! into a local before reading `uv_channels_min_and_extents[0]` from it. On
//! WebGL2 through ANGLE on Adreno (seen on a Galaxy S23 Ultra, Chrome 155)
//! that copy loses the array member: every UV decodes to zero, every
//! fragment samples texel (0,0), and sprites show as one flat colour or
//! nothing at all. Positions, read from the same copy, survive. Stock Bevy's
//! sprite example is blank on the same phone.
//!
//! On the uniform-buffer path (WebGL2) this rewrites the embedded
//! `vertex_input.wesl` to read the range straight from the `metadata`
//! binding, which the phone draws correctly. The storage-buffer path is
//! left as Bevy wrote it. Removing this plugin brings the bug back and
//! changes nothing else.

use bevy::prelude::*;
use bevy::shader::Shader;

const SHADER_PATH_SUFFIX: &str = "mesh2d/vertex_input.wesl";

const UV_LINE: &str = "    let uv_min_and_extents = mesh_metadata.uv_channels_min_and_extents[0];";

const UV_LINE_PATCHED: &str =
    "    let uv_min_and_extents = metadata.uv_channels_min_and_extents[0];";

const IMPORT_LINE: &str = "import package::mesh2d::functions as mesh_functions;";

const IMPORT_ADDED: &str = "import package::mesh2d::bindings::metadata;";

pub struct Mesh2dUvFixPlugin;

impl Plugin for Mesh2dUvFixPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, patch_vertex_input);
    }
}

/// The patched source, or `None` when `source` is not the shader this fix
/// knows, or is already patched.
pub fn patched(source: &str) -> Option<String> {
    if source.contains(IMPORT_ADDED) || !source.contains(IMPORT_LINE) {
        return None;
    }
    let mut out = String::with_capacity(source.len() + 256);
    let mut swapped = false;
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if line == IMPORT_LINE {
            out.push_str(line);
            out.push('\n');
            out.push_str(IMPORT_ADDED);
            out.push('\n');
            continue;
        }
        // The line before is its `@if(VERTEX_UVS && VERTEX_UVS_COMPRESSED)`.
        if !swapped && lines.peek() == Some(&UV_LINE) {
            let cond = line
                .trim()
                .strip_prefix("@if(")
                .and_then(|c| c.strip_suffix(')'))?;
            lines.next();
            out.push_str(&format!(
                "    @if({cond} && METADATA_USE_UNIFORM_BUFFERS)\n"
            ));
            out.push_str(UV_LINE_PATCHED);
            out.push('\n');
            out.push_str(&format!(
                "    @if({cond} && !METADATA_USE_UNIFORM_BUFFERS)\n"
            ));
            out.push_str(UV_LINE);
            out.push('\n');
            swapped = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    swapped.then_some(out)
}

fn patch_vertex_input(
    mut events: MessageReader<AssetEvent<Shader>>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    let ids: Vec<_> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    for id in ids {
        let Some(shader) = shaders.get(id) else {
            continue;
        };
        if !shader.path.ends_with(SHADER_PATH_SUFFIX) {
            continue;
        }
        let Some(source) = patched(shader.source.as_str()) else {
            if !shader.source.as_str().contains(IMPORT_ADDED) {
                warn!("mesh2d_uv_fix: Bevy's {SHADER_PATH_SUFFIX} changed; UVs unpatched");
            }
            continue;
        };
        let mut next = Shader::from_wesl(source, shader.path.clone());
        next.shader_defs = shader.shader_defs.clone();
        next.file_dependencies = shader.file_dependencies.clone();
        next.validate_shader = shader.validate_shader.clone();
        let _ = shaders.insert(id, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEVY: &str = "import package::mesh2d::functions as mesh_functions;

fn decompress_vertex(vertex_in: Vertex, instance_index: u32) -> UncompressedVertex {
    let mesh_metadata = mesh_functions::get_metadata(instance_index);
    @if(VERTEX_UVS && VERTEX_UVS_COMPRESSED)
    let uv_min_and_extents = mesh_metadata.uv_channels_min_and_extents[0];
    @if(VERTEX_UVS && VERTEX_UVS_COMPRESSED)
    uncompressed_vertex.uv = decompress_vertex_uv(vertex_in.compressed_uv, uv_min_and_extents);
}
";

    #[test]
    fn reads_the_binding_on_the_uniform_path_only() {
        let out = patched(BEVY).unwrap();
        assert!(out.contains(IMPORT_ADDED));
        assert!(out.contains(
            "    @if(VERTEX_UVS && VERTEX_UVS_COMPRESSED && METADATA_USE_UNIFORM_BUFFERS)\n\
             \x20   let uv_min_and_extents = metadata.uv_channels_min_and_extents[0];\n\
             \x20   @if(VERTEX_UVS && VERTEX_UVS_COMPRESSED && !METADATA_USE_UNIFORM_BUFFERS)\n\
             \x20   let uv_min_and_extents = mesh_metadata.uv_channels_min_and_extents[0];\n"
        ));
        assert!(out.contains("uncompressed_vertex.uv = decompress_vertex_uv"));
    }

    #[test]
    fn patches_once() {
        let out = patched(BEVY).unwrap();
        assert_eq!(patched(&out), None);
    }

    #[test]
    fn leaves_an_unknown_shader_alone() {
        assert_eq!(patched("fn main() {}"), None);
        assert_eq!(patched(IMPORT_LINE), None);
    }
}
