//! What a page's fonts are called, and what that tells us.
//!
//! Only what layout needs: a name, and whether it reads as bold or italic.
//! Glyph metrics are deliberately out of scope — this crate measures text by
//! where the content stream *puts* it, not by summing advance widths, because
//! every real book positions its own text far more often than it relies on
//! natural flow.

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
#[derive(Debug, Clone, PartialEq, Default)]
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

/// Read a page's font resources.
///
/// A page naming a font this cannot resolve still yields runs: the text
/// machine falls back to a default, because losing the words to recover the
/// typeface would be the wrong trade.
pub fn fonts_for_page(document: &Document, page_id: (u32, u16)) -> FontMap {
    let mut map = FontMap::new();
    let Ok(resources) = document.get_page_resources(page_id) else {
        return map;
    };
    let Some(dictionary) = resources.0 else {
        return map;
    };
    let Ok(fonts) = dictionary.get(b"Font") else {
        return map;
    };
    let Some(fonts) = resolve(document, fonts).and_then(|o| o.as_dict().ok()) else {
        return map;
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
        map.insert(name, FontInfo::from_base_font(base));
    }
    map
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
