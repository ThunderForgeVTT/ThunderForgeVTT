//! Playtest 2026-09-10 P8: vector art in, WebP out.
//!
//! Heroes are drawn as SVG (`packages/heroes`), because art composed from
//! parts is easy to write, change and generate — but the engine draws
//! textures, and Bevy loads raster images only. So an uploaded SVG is drawn to
//! pixels here, on its way into the same WebP pipeline every other upload
//! takes; nothing downstream learns it was ever a vector.
//!
//! An SVG is a document, not pixels, and a document can point at things.
//! Nothing it names is followed: `usvg` treats an `<image href>` as a path on
//! this server's disk by default, and loads a nested document from a data URL
//! — both resolvers are replaced with ones that load nothing. `resvg` is built
//! without `text`, so `<text>` draws nothing rather than reading the host's
//! font directory. And the declared size is ignored: a file claiming to be
//! 100000px square is drawn at `SVG_RASTER_EDGE` like any other.

use image::{DynamicImage, RgbaImage};
use resvg::{tiny_skia, usvg};

/// Longest edge, in pixels, an SVG is drawn at.
///
/// Vector art has no size of its own worth keeping, so every SVG is drawn at
/// this one: sharp for a token or a portrait at any zoom the play field
/// offers, and a bounded 4MB of pixels however large the file says it is.
pub const SVG_RASTER_EDGE: u32 = 1024;

/// How far into a file to look for `<svg`: past an XML declaration, a
/// doctype and a comment or two, which is where an editor puts it.
const SNIFF_BYTES: usize = 4096;

/// Whether `bytes` is an SVG document. Sniffed, because `image::guess_format`
/// knows only raster formats; every raster format starts with binary magic,
/// never with `<`.
pub fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(SNIFF_BYTES)]);
    let text = head.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<') && text.contains("<svg")
}

/// Draws an SVG document to straight-alpha RGBA, `SVG_RASTER_EDGE` on its
/// longest side. A document that does not parse is an error, never a blank
/// image.
pub fn rasterise_svg(bytes: &[u8]) -> Result<DynamicImage, String> {
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|e| e.to_string())?;

    let size = tree.size();
    let scale = SVG_RASTER_EDGE as f32 / size.width().max(size.height());
    let width = ((size.width() * scale).round() as u32).clamp(1, SVG_RASTER_EDGE);
    let height = ((size.height() * scale).round() as u32).clamp(1, SVG_RASTER_EDGE);

    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("svg has no area to draw")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    // tiny-skia keeps premultiplied alpha; `image` and WebP expect it straight,
    // and a token's transparent corners must stay transparent, not go black.
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let c = pixel.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    RgbaImage::from_raw(width, height, rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "svg drawing has the wrong size".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256"><circle cx="128" cy="128" r="112" fill="#3d6fd1"/></svg>"##;

    #[test]
    fn an_svg_is_drawn_at_the_raster_edge_and_keeps_its_transparency() {
        let img = rasterise_svg(TOKEN.as_bytes()).expect("draws").to_rgba8();
        assert_eq!(
            (img.width(), img.height()),
            (SVG_RASTER_EDGE, SVG_RASTER_EDGE)
        );
        assert_eq!(img.get_pixel(512, 512).0, [0x3d, 0x6f, 0xd1, 255]);
        assert_eq!(img.get_pixel(2, 2).0[3], 0, "outside the disc is clear");
    }

    #[test]
    fn the_size_a_file_declares_is_not_the_size_it_is_drawn_at() {
        let huge = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="50000"><rect width="100000" height="50000" fill="red"/></svg>"#;
        let img = rasterise_svg(huge.as_bytes()).expect("draws");
        assert_eq!(
            (img.width(), img.height()),
            (SVG_RASTER_EDGE, SVG_RASTER_EDGE / 2)
        );
    }

    #[test]
    fn nothing_the_file_names_is_loaded_from_this_machine() {
        // A real image on this machine's disk, named by path and by file URL.
        // Loaded, either would paint the drawing solid red.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("secret.png");
        RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]))
            .save(&path)
            .expect("write png");
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="64" height="64"><image href="{0}" width="64" height="64"/><image xlink:href="file://{0}" width="64" height="64"/></svg>"#,
            path.display()
        );
        let img = rasterise_svg(svg.as_bytes()).expect("draws").to_rgba8();
        assert!(img.pixels().all(|p| p.0[3] == 0), "nothing was drawn");
    }

    #[test]
    fn an_svg_is_told_apart_from_raster_images_and_from_text() {
        assert!(looks_like_svg(TOKEN.as_bytes()));
        assert!(looks_like_svg(
            b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<!-- art -->\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>"
        ));
        assert!(!looks_like_svg(b"\x89PNG\r\n\x1a\n<svg"));
        assert!(!looks_like_svg(b"hello <svg"));
    }

    #[test]
    fn a_broken_svg_is_an_error_not_a_blank_image() {
        assert!(rasterise_svg(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect").is_err());
    }

    #[test]
    fn an_actor_image_upload_takes_svg_and_stores_webp() {
        let renditions = crate::storage::transcode::transcode_to_lore_renditions(TOKEN.as_bytes())
            .expect("an svg is a valid upload");
        assert_eq!(renditions.original_format, "svg");
        assert_eq!(renditions.full_width, SVG_RASTER_EDGE);
        assert_eq!(
            image::guess_format(&renditions.full_webp_bytes).expect("an image"),
            image::ImageFormat::WebP
        );
        assert_eq!(renditions.thumbnail_width, 256);
    }
}
