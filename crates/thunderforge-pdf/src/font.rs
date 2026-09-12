//! What a page's fonts are called, and what that tells us.
//!
//! A name, whether it reads as bold or italic, how its codes become
//! characters, and how wide each of those characters is.
//!
//! # Widths were left out at first, and the corpus said no
//!
//! The original note here said glyph metrics were out of scope, because books
//! position their text explicitly and half an em a character was close enough
//! to tell a word space from a column gutter. It is not close enough to tell
//! a word space from *nothing*: a book whose font is half a em narrower than
//! the guess yields "Arm orC lass" where it means "Armor Class", and a reader
//! looking for that label finds nothing in the whole book. Every simple font
//! carries a `/Widths` array; using it costs one lookup a glyph.

use lopdf::{Document, Object};
use std::collections::HashMap;

/// Follow an indirect reference, if it is one.
///
/// `lopdf`'s own `dereference` answers with the object *and* the id it came
/// from, wrapped in a `Result` that is an error only for a dangling
/// reference. Every caller here wants the object or nothing, so that shape is
/// flattened once rather than unwrapped three different ways.
pub(crate) fn resolve<'a>(document: &'a Document, object: &'a Object) -> Option<&'a Object> {
    document.dereference(object).ok().map(|(_, value)| value)
}

/// One font, as a page refers to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontInfo {
    /// The PostScript name — `ABCDEF+Bookmania-Bold`, `Helvetica`.
    pub base_font: String,
    /// Whether the name says bold. Style is read from the name because that
    /// is where a subset font records it; the alternative is the font
    /// descriptor's flags, which subsetters routinely get wrong.
    pub bold: bool,
    pub italic: bool,
}

impl FontInfo {
    fn from_base_font(base_font: String) -> Self {
        // Names arrive as `ABCDEF+Family-BoldItalic`, `Family,Bold`, or
        // `Family-Semibold`. Lower-cased substring matching handles all three
        // without a table of every foundry's spelling.
        let lowered = base_font.to_lowercase();
        let bold = lowered.contains("bold")
            || lowered.contains("black")
            || lowered.contains("heavy")
            || lowered.contains("semib");
        let italic = lowered.contains("italic") || lowered.contains("oblique");
        Self {
            base_font,
            bold,
            italic,
        }
    }
}

/// The fonts one page can name, keyed by the name its content stream uses.
pub type FontMap = HashMap<String, FontInfo>;

/// How wide each of a font's glyphs is, in ems.
///
/// Keyed by *code*, and carrying how many bytes make one, because the two
/// kinds of font disagree about both. A simple font has a `/Widths` array
/// indexed from `/FirstChar` and addresses glyphs with one byte; a composite
/// font keeps a `/W` array on its descendant and uses two. Getting the code
/// width wrong halves or doubles every measurement, which is worse than
/// having no widths at all.
#[derive(Debug, Clone)]
pub struct Widths {
    by_code: HashMap<u32, f64>,
    default: f64,
    code_bytes: usize,
}

impl Default for Widths {
    fn default() -> Self {
        Self {
            by_code: HashMap::new(),
            // Half an em, the average of Latin text and this crate's only
            // answer before fonts were read.
            default: 0.5,
            code_bytes: 1,
        }
    }
}

impl Widths {
    /// How many bytes make one code in this font.
    pub fn code_bytes(&self) -> usize {
        self.code_bytes
    }

    /// The total advance of a string, in ems.
    pub fn advance_of(&self, bytes: &[u8]) -> f64 {
        bytes
            .chunks(self.code_bytes)
            .filter(|chunk| chunk.len() == self.code_bytes)
            .map(|chunk| {
                let mut code = 0u32;
                for byte in chunk {
                    code = (code << 8) | u32::from(*byte);
                }
                self.by_code.get(&code).copied().unwrap_or(self.default)
            })
            .sum()
    }

    fn is_usable(&self) -> bool {
        self.by_code.values().any(|width| *width > 0.0)
    }
}

/// A page's fonts, with the encodings needed to read text drawn in them.
///
/// Borrowed from the document rather than owned, because it is used for
/// exactly as long as one page takes to parse — and `lopdf`'s encoding type
/// borrows its byte table from a static it will not name.
pub struct PageFonts<'a> {
    pub info: FontMap,
    /// **Not decoration.** A subsetted font renumbers its glyphs, so its
    /// codes have no relation to ASCII: one book in the reference library
    /// yields `* ROGGUDJRQV` where it means `Gold dragons`, every byte
    /// shifted by 29. Read as Latin-1 that is gibberish which *looks* like
    /// text — the worst failure available, because nothing downstream can
    /// tell that it is wrong.
    pub encodings: HashMap<String, lopdf::Encoding<'a>>,
    /// A font's own `ToUnicode` map, parsed by this crate.
    ///
    /// Preferred over `lopdf`'s encoding when present, because `lopdf`'s
    /// ToUnicode decoder reads two bytes at a time and a Type 3 font
    /// addresses its glyphs with one — see [`crate::cmap`]. A whole bestiary
    /// in the reference library sets its body text in such a font.
    pub to_unicode: HashMap<String, crate::cmap::ToUnicode>,
    /// Per-font glyph advances, for measuring a run rather than guessing it.
    pub widths: HashMap<String, Widths>,
}

/// Read a page's font resources.
///
/// A page naming a font this cannot resolve still yields runs: the text
/// machine falls back to a default, because losing the words to recover the
/// typeface would be the wrong trade.
pub fn fonts_for_page(document: &Document, page_id: (u32, u16)) -> PageFonts<'_> {
    let mut map = FontMap::new();
    let mut encodings = HashMap::new();
    let mut to_unicode = HashMap::new();
    let mut widths = HashMap::new();
    let empty = |map: FontMap| PageFonts {
        info: map,
        encodings: HashMap::new(),
        to_unicode: HashMap::new(),
        widths: HashMap::new(),
    };
    let Ok(resources) = document.get_page_resources(page_id) else {
        return empty(map);
    };
    let Some(dictionary) = resources.0 else {
        return empty(map);
    };
    let Ok(fonts) = dictionary.get(b"Font") else {
        return empty(map);
    };
    let Some(fonts) = resolve(document, fonts).and_then(|o| o.as_dict().ok()) else {
        return empty(map);
    };

    for (name, value) in fonts.iter() {
        let name = String::from_utf8_lossy(name).into_owned();
        let Some(font) = resolve(document, value).and_then(|o| o.as_dict().ok()) else {
            continue;
        };
        let base = font
            .get(b"BaseFont")
            .ok()
            .and_then(|object| match resolve(document, object) {
                Some(Object::Name(bytes)) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .unwrap_or_default();
        if let Some(measured) = widths_of(document, font) {
            widths.insert(name.clone(), measured);
        }
        if let Some(parsed) = to_unicode_of(document, font) {
            to_unicode.insert(name.clone(), parsed);
        }
        if let Some(encoding) = encoding_of(document, font) {
            encodings.insert(name.clone(), encoding);
        }
        map.insert(name, FontInfo::from_base_font(base));
    }
    PageFonts {
        info: map,
        encodings,
        to_unicode,
        widths,
    }
}

/// Read a font's advances, normalised to ems.
///
/// Two entirely different layouts, because there are two kinds of font:
///
/// - A **simple** font (Type 1, TrueType, Type 3) has `/Widths`, an array
///   indexed from `/FirstChar`, in thousandths of an em — except Type 3,
///   which quotes its own glyph space and carries a `/FontMatrix` to convert.
///   Type 3 is precisely the kind that made this necessary, so ignoring the
///   matrix would fix nothing.
/// - A **composite** font (Type 0) has nothing of its own. Its widths live on
///   the descendant font as `/W`, a nested array, with `/DW` for everything
///   the array omits. This is the kind a modern book sets its statblocks in.
fn widths_of(document: &Document, font: &lopdf::Dictionary) -> Option<Widths> {
    let subtype = font
        .get(b"Subtype")
        .ok()
        .and_then(|object| object.as_name_str().ok())
        .unwrap_or_default()
        .to_string();

    if subtype == "Type0" {
        return composite_widths(document, font);
    }
    simple_widths(document, font)
}

fn simple_widths(document: &Document, font: &lopdf::Dictionary) -> Option<Widths> {
    let first_char = font
        .get_deref(b"FirstChar", document)
        .ok()
        .and_then(|object| object.as_i64().ok())
        .unwrap_or(0)
        .max(0) as u32;

    let array = font
        .get_deref(b"Widths", document)
        .ok()?
        .as_array()
        .ok()?
        .clone();

    let scale = type3_scale(document, font).unwrap_or(0.001);
    let mut by_code = HashMap::new();
    for (offset, object) in array.iter().enumerate() {
        let Ok(offset) = u32::try_from(offset) else {
            break;
        };
        if let Some(width) = number_of(document, object) {
            by_code.insert(first_char + offset, width * scale);
        }
    }

    let widths = Widths {
        by_code,
        default: 0.5,
        code_bytes: 1,
    };
    widths.is_usable().then_some(widths)
}

/// A composite font's `/W` array.
///
/// Two forms, mixed freely in one array:
/// `c [w1 w2 …]` gives a width each to codes `c`, `c+1`, …; `first last w`
/// gives one width to every code in the range.
fn composite_widths(document: &Document, font: &lopdf::Dictionary) -> Option<Widths> {
    let descendants = font
        .get_deref(b"DescendantFonts", document)
        .ok()?
        .as_array()
        .ok()?
        .clone();
    let descendant = resolve(document, descendants.first()?)?.as_dict().ok()?;

    // `/DW` is the width of everything `/W` does not mention. Its default is
    // 1000 — a full em — which matters: a CJK font omits most of its array.
    let default = descendant
        .get_deref(b"DW", document)
        .ok()
        .and_then(|object| object.as_i64().ok())
        .unwrap_or(1000) as f64
        / 1000.0;

    let mut by_code = HashMap::new();
    if let Ok(array) = descendant
        .get_deref(b"W", document)
        .and_then(|o| o.as_array())
    {
        let values: Vec<&Object> = array.iter().collect();
        let mut index = 0usize;
        while index < values.len() {
            let Some(first) = number_of(document, values[index]) else {
                index += 1;
                continue;
            };
            let first = first as u32;
            match values.get(index + 1).and_then(|o| resolve(document, o)) {
                Some(Object::Array(list)) => {
                    for (offset, item) in list.iter().enumerate() {
                        if let (Ok(offset), Some(width)) =
                            (u32::try_from(offset), number_of(document, item))
                        {
                            by_code.insert(first + offset, width / 1000.0);
                        }
                    }
                    index += 2;
                }
                _ => {
                    let (Some(last), Some(width)) = (
                        values.get(index + 1).and_then(|o| number_of(document, o)),
                        values.get(index + 2).and_then(|o| number_of(document, o)),
                    ) else {
                        index += 1;
                        continue;
                    };
                    let last = last as u32;
                    // A range covering the whole code space would allocate a
                    // map of it; a backwards one is nonsense.
                    if last >= first && last - first <= 0xFFFF {
                        for code in first..=last {
                            by_code.insert(code, width / 1000.0);
                        }
                    }
                    index += 3;
                }
            }
        }
    }

    Some(Widths {
        by_code,
        default,
        // Identity-H, which is what every composite font in the reference
        // library uses. A two-byte code read one byte at a time doubles the
        // length of every run.
        code_bytes: 2,
    })
}

fn number_of(document: &Document, object: &Object) -> Option<f64> {
    match resolve(document, object)? {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// The horizontal scale of a Type 3 font's glyph space.
/// The horizontal scale of a Type 3 font's glyph space.
fn type3_scale(document: &Document, font: &lopdf::Dictionary) -> Option<f64> {
    let matrix = font
        .get_deref(b"FontMatrix", document)
        .ok()?
        .as_array()
        .ok()?;
    match resolve(document, matrix.first()?)? {
        Object::Real(r) => Some(*r as f64),
        Object::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

/// A font's own `ToUnicode` map, if it has one this can read.
fn to_unicode_of(document: &Document, font: &lopdf::Dictionary) -> Option<crate::cmap::ToUnicode> {
    let stream = font
        .get_deref(b"ToUnicode", document)
        .ok()?
        .as_stream()
        .ok()?;
    crate::cmap::parse(&stream.get_plain_content().ok()?)
}

/// Read a font's encoding, if it declares one this can use.
///
/// `None` means "read the bytes as they are", which is right for a font with
/// no encoding at all and is what this crate did for everything before.
fn encoding_of<'a>(
    document: &'a Document,
    font: &'a lopdf::Dictionary,
) -> Option<lopdf::Encoding<'a>> {
    match font.get_font_encoding(document).ok()? {
        // A named encoding this build has no table for. Falling back to the
        // raw bytes is the same answer as before and no worse.
        lopdf::Encoding::SimpleEncoding(_) => None,
        known => Some(known),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subset_prefix_does_not_hide_the_style() {
        // Every real book's fonts arrive subsetted, with a six-letter tag in
        // front. Matching on the whole lower-cased name sees through it.
        let font = FontInfo::from_base_font("ABCDEF+Bookmania-BoldItalic".into());
        assert!(font.bold);
        assert!(font.italic);
    }

    #[test]
    fn the_spellings_foundries_actually_use_are_all_bold() {
        for name in [
            "Helvetica-Bold",
            "Arial,Bold",
            "Scala-Semibold",
            "Futura-Black",
            "Gotham-Heavy",
        ] {
            assert!(
                FontInfo::from_base_font(name.into()).bold,
                "{name} should read as bold"
            );
        }
    }

    #[test]
    fn oblique_is_italic_by_another_name() {
        assert!(FontInfo::from_base_font("Helvetica-Oblique".into()).italic);
    }

    #[test]
    fn a_plain_face_is_neither() {
        let font = FontInfo::from_base_font("Times-Roman".into());
        assert!(!font.bold);
        assert!(!font.italic);
    }
}
