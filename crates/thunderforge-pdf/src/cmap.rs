//! Reading a font's `ToUnicode` map.
//!
//! # Why this is not `lopdf`'s job here
//!
//! `lopdf` has a ToUnicode parser and it is used first. It assumes two-byte
//! codes — its decoder is a `chunks_exact(2)` — which is right for the
//! `Identity-H` fonts that dominate modern PDFs and wrong for a Type 3 font,
//! which addresses its glyphs with **one** byte.
//!
//! That is not an exotic case. A bestiary in the reference library sets its
//! entire body text in such a font, and read without its map it yields:
//!
//! ```text
//! * ROGGUDJRQVKDYHWKHP RVWORYHRIIH\DP RQJDOOGUDJRQNLQG
//! ```
//!
//! which is "Gold dragons have the most love of fey among all dragonkind".
//! The font's own map explains it exactly: `<0E> <1E> <002B>` maps code 0x0E
//! to U+002B, an offset of 29 — the shift the garbled text shows.
//!
//! # The format, and why parsing it by hand is reasonable
//!
//! A ToUnicode CMap is PostScript, but the part that matters is three
//! constructs and nothing else:
//!
//! ```text
//! 1 begincodespacerange   <00> <FF>                 endcodespacerange
//! 3 beginbfchar           <03> <0020>               endbfchar
//! 6 beginbfrange          <0E> <1E> <002B>          endbfrange
//! ```
//!
//! Everything around them is boilerplate a reader may ignore. Scanning for
//! those three keywords is a great deal less machinery than a PostScript
//! interpreter, and it cannot be defeated by the parts of the file it does
//! not look at.

use std::collections::HashMap;

/// A font's code-to-text map.
#[derive(Debug, Clone, Default)]
pub struct ToUnicode {
    /// How many bytes make one code, from the codespace range. One for a
    /// Type 3 font, two for `Identity-H`.
    code_bytes: usize,
    map: HashMap<u32, String>,
}

impl ToUnicode {
    /// Whether this map is worth using.
    pub fn is_usable(&self) -> bool {
        !self.map.is_empty() && self.code_bytes > 0
    }

    /// Decode a string drawn in this font.
    ///
    /// A code with no entry contributes nothing rather than a replacement
    /// character: the map is the font's own statement of what it can say, and
    /// an unmapped code is usually a glyph with no text meaning at all — a
    /// rule, an ornament, a logo.
    pub fn decode(&self, bytes: &[u8]) -> String {
        let mut out = String::new();
        for chunk in bytes.chunks(self.code_bytes) {
            if chunk.len() < self.code_bytes {
                break;
            }
            let mut code = 0u32;
            for byte in chunk {
                code = (code << 8) | u32::from(*byte);
            }
            if let Some(text) = self.map.get(&code) {
                out.push_str(text);
            }
        }
        out
    }
}

/// Parse a ToUnicode CMap stream.
pub fn parse(content: &[u8]) -> Option<ToUnicode> {
    let text = String::from_utf8_lossy(content);
    let mut map = HashMap::new();

    // The codespace range says how wide a code is. Without one, assume two
    // bytes: that is what `Identity-H` uses and what a CMap that omits the
    // range is overwhelmingly likely to be.
    let code_bytes = first_codespace_width(&text).unwrap_or(2);

    for section in sections(&text, "beginbfchar", "endbfchar") {
        let tokens = hex_tokens(section);
        for pair in tokens.chunks(2) {
            if let [code, value] = pair
                && let Some(code) = code_of(code)
            {
                map.insert(code, utf16be(value));
            }
        }
    }

    for section in sections(&text, "beginbfrange", "endbfrange") {
        parse_ranges(section, &mut map);
    }

    let parsed = ToUnicode { code_bytes, map };
    parsed.is_usable().then_some(parsed)
}

/// The width, in bytes, of the first code in the codespace range.
fn first_codespace_width(text: &str) -> Option<usize> {
    let section = sections(text, "begincodespacerange", "endcodespacerange")
        .next()?
        .to_string();
    let first = hex_tokens(&section).into_iter().next()?;
    // Two hex digits to a byte. A `<00> <FF>` range is one byte wide; a
    // `<0000> <FFFF>` range is two.
    (first.len() / 2).clamp(1, 4).into()
}

/// Every region between two keywords.
fn sections<'a>(text: &'a str, open: &'a str, close: &'a str) -> impl Iterator<Item = &'a str> {
    let mut from = 0usize;
    std::iter::from_fn(move || {
        let start = text[from..].find(open)? + from + open.len();
        let end = text[start..].find(close)? + start;
        from = end + close.len();
        Some(&text[start..end])
    })
}

/// The `<hex>` tokens in a section, in order, as their hex digits.
fn hex_tokens(section: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let inside: String = rest[open + 1..open + close]
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect();
        if !inside.is_empty() {
            out.push(inside);
        }
        rest = &rest[open + close + 1..];
    }
    out
}

fn code_of(hex: &str) -> Option<u32> {
    u32::from_str_radix(hex, 16).ok()
}

/// A destination: UTF-16BE hex digits, possibly several characters.
fn utf16be(hex: &str) -> String {
    let units: Vec<u16> = hex
        .as_bytes()
        .chunks(4)
        .filter(|chunk| chunk.len() == 4)
        .filter_map(|chunk| u16::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok())
        .collect();
    String::from_utf16_lossy(&units)
}

/// `bfrange` has two forms, and a real file uses both.
///
/// `<lo> <hi> <dst>` walks the destination up alongside the code — which is
/// the form that produced the 29-character shift. `<lo> <hi> [<a> <b> ...]`
/// lists a destination per code, for a range whose characters are unrelated.
fn parse_ranges(section: &str, map: &mut HashMap<u32, String>) {
    let mut rest = section;
    loop {
        let Some((lo, after_lo)) = next_hex(rest) else {
            return;
        };
        let Some((hi, after_hi)) = next_hex(after_lo) else {
            return;
        };
        let (Some(lo), Some(hi)) = (code_of(&lo), code_of(&hi)) else {
            return;
        };
        // A malformed range must not spin: `hi` below `lo` is nonsense, and a
        // range spanning the whole code space would allocate a map of it.
        if hi < lo || hi - lo > 0xFFFF {
            return;
        }

        let trimmed = after_hi.trim_start();
        if let Some(list) = trimmed.strip_prefix('[') {
            let Some(end) = list.find(']') else {
                return;
            };
            for (offset, value) in hex_tokens(&list[..end]).into_iter().enumerate() {
                let Ok(offset) = u32::try_from(offset) else {
                    break;
                };
                if lo + offset > hi {
                    break;
                }
                map.insert(lo + offset, utf16be(&value));
            }
            rest = &list[end + 1..];
            continue;
        }

        let Some((destination, after)) = next_hex(trimmed) else {
            return;
        };
        // Walk the destination up with the code. Only the last UTF-16 unit
        // advances: a range mapping to a multi-character string increments
        // its final character, which is what the specification says and what
        // a ligature range relies on.
        let base = utf16be(&destination);
        for code in lo..=hi {
            let step = code - lo;
            let mut units: Vec<u16> = base.encode_utf16().collect();
            if let Some(last) = units.last_mut() {
                *last = last.wrapping_add(step as u16);
            }
            map.insert(code, String::from_utf16_lossy(&units));
        }
        rest = after;
    }
}

/// The next `<hex>` token and what follows it.
fn next_hex(text: &str) -> Option<(String, &str)> {
    let open = text.find('<')?;
    let close = text[open..].find('>')? + open;
    let inside: String = text[open + 1..close]
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    inside
        .is_empty()
        .then_some(())
        .map_or(Some((inside.clone(), &text[close + 1..])), |()| {
            Some((inside, &text[close + 1..]))
        })
}

#[cfg(test)]
#[path = "cmap_tests.rs"]
mod tests;
