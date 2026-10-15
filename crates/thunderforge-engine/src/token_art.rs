//! Where a token's art comes from: a URL the asset server fetches, or the
//! bytes the host already has (`WorldTokenPayload::photo_data`).

use super::*;

/// Where a token's art comes from: a URL the asset server fetches, or bytes
/// the host already has (`WorldTokenPayload::photo_data`).
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct TokenArt<'w> {
    asset_server: Res<'w, AssetServer>,
    images: ResMut<'w, Assets<Image>>,
}

impl TokenArt<'_> {
    /// The handle for the art this payload carries, or `None` when it carries
    /// none (a positional update) or the bytes would not decode.
    pub(crate) fn handle_for(&mut self, token: &WorldTokenPayload) -> Option<Handle<Image>> {
        if let Some(data) = token.photo_data.as_deref() {
            return match decode_token_art(data) {
                Ok(image) => Some(self.images.add(image)),
                Err(why) => {
                    warn!("token {}: photoData not decoded: {why}", token.id);
                    None
                }
            };
        }
        // Owned: `AssetServer::load` borrows for `'static`.
        token
            .photo_url
            .clone()
            .map(|path| self.asset_server.load(path))
    }
}

/// Puts new art on a token's sprite. A token spawned without art is a square
/// of its kind's colour (`Sprite::from_color`); that colour is the sprite's
/// tint, so art arriving later must also clear it, or it is drawn as a flat
/// square of the same colour. The size goes back to `size_tokens_to_grid`,
/// which re-fits it once the new art's dimensions are known.
pub(crate) fn put_art(sprite: &mut Sprite, image: Handle<Image>) {
    sprite.image = image;
    sprite.color = Color::WHITE;
    sprite.custom_size = None;
}

/// Base64 (standard or URL alphabet, padding optional, a `data:` prefix
/// tolerated) to a decoded image. The format is read from the bytes, so a
/// WebP or a PNG both work; `is_srgb` because token art is colour, not data.
fn decode_token_art(data: &str) -> Result<Image, String> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    let payload = data.rsplit_once(',').map_or(data, |(_, b)| b);
    let bytes = decode_base64(payload.trim())?;
    let kind = if bytes.starts_with(b"RIFF") {
        "webp"
    } else {
        "png"
    };
    Image::from_buffer(
        &bytes,
        ImageType::Extension(kind),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .map_err(|e| e.to_string())
}

/// A base64 decoder small enough not to be a dependency: the engine's wasm is
/// counted in bytes, and this is the one place it reads base64.
fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for (i, c) in text.bytes().enumerate() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return Err(format!("not base64 at byte {i}")),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if out.is_empty() {
        return Err("empty".to_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod token_art_tests {
    use super::{decode_base64, put_art};
    use bevy::prelude::*;

    #[test]
    fn art_put_on_a_plain_token_is_not_tinted_its_kind_colour() {
        let mut sprite = Sprite::from_color(Color::srgb(0.3, 0.5, 1.0), Vec2::splat(48.0));
        put_art(&mut sprite, Handle::default());
        assert_eq!(sprite.color, Color::WHITE);
        assert_eq!(sprite.custom_size, None);
    }

    #[test]
    fn decodes_standard_and_url_alphabets_with_or_without_padding() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVsbG8").unwrap(), b"hello");
        assert_eq!(decode_base64("+/8=").unwrap(), b"\xfb\xff");
        assert_eq!(decode_base64("-_8").unwrap(), b"\xfb\xff");
    }

    #[test]
    fn refuses_what_is_not_base64() {
        assert!(decode_base64("").is_err());
        assert!(decode_base64("aGV$sbG8").is_err());
    }
}
