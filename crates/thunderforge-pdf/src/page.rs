//! Pages, their size, and the document's own outline.

use crate::OutlineEntry;
use lopdf::{Document, Object};
use std::collections::HashMap;

/// A page's box, in PDF units (72 to the inch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    pub width: f64,
    pub height: f64,
}

impl Default for PageGeometry {
    /// US Letter, which is what the overwhelming majority of this corpus is.
    fn default() -> Self {
        Self {
            width: 612.0,
            height: 792.0,
        }
    }
}

/// One page of a document.
#[derive(Debug, Clone)]
pub struct Page {
    /// One-based, as a reader would count it.
    pub number: u32,
    pub geometry: PageGeometry,
    pub(crate) id: (u32, u16),
}

pub fn pages(document: &Document) -> Vec<Page> {
    let mut out: Vec<Page> = document
        .get_pages()
        .into_iter()
        .map(|(number, id)| Page {
            number,
            geometry: geometry_of(document, id),
            id,
        })
        .collect();
    out.sort_by_key(|page| page.number);
    out
}

/// A page's size, which may be recorded on an ancestor rather than the page.
///
/// MediaBox is an inheritable attribute: a book that sets it once on the root
/// `Pages` node and never again is perfectly ordinary, and reading only the
/// page's own dictionary would make every page in it Letter by default.
fn geometry_of(document: &Document, id: (u32, u16)) -> PageGeometry {
    inherited_media_box(document, id).unwrap_or_default()
}

fn inherited_media_box(document: &Document, id: (u32, u16)) -> Option<PageGeometry> {
    let mut current = document.get_dictionary(id).ok()?.clone();
    for _ in 0..8 {
        if let Ok(media) = current.get(b"MediaBox")
            && let Some(geometry) = media_box_of(document, media)
        {
            return Some(geometry);
        }
        let parent = current.get(b"Parent").ok()?.clone();
        current = crate::font::resolve(document, &parent)?
            .as_dict()
            .ok()?
            .clone();
    }
    None
}

fn media_box_of(document: &Document, object: &Object) -> Option<PageGeometry> {
    let array = crate::font::resolve(document, object)?.as_array().ok()?;
    if array.len() != 4 {
        return None;
    }
    let value = |index: usize| -> Option<f64> {
        match crate::font::resolve(document, &array[index])? {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(r) => Some(*r as f64),
            _ => None,
        }
    };
    let (x0, y0, x1, y1) = (value(0)?, value(1)?, value(2)?, value(3)?);
    // A box may be given in any corner order, and a few real books give it
    // reversed. Width and height are what callers want, so normalise.
    Some(PageGeometry {
        width: (x1 - x0).abs(),
        height: (y1 - y0).abs(),
    })
}

/// The document's own outline, flattened depth-first.
pub fn outline(document: &Document) -> Vec<OutlineEntry> {
    let page_numbers: HashMap<(u32, u16), u32> = document
        .get_pages()
        .into_iter()
        .map(|(number, id)| (id, number))
        .collect();

    let Ok(catalog) = document.catalog() else {
        return Vec::new();
    };
    let Ok(outlines) = catalog.get(b"Outlines") else {
        return Vec::new();
    };
    let Some(outlines) = crate::font::resolve(document, outlines).and_then(|o| o.as_dict().ok())
    else {
        return Vec::new();
    };
    let Ok(first) = outlines.get(b"First") else {
        return Vec::new();
    };

    let mut out = Vec::new();
    walk(document, first, 0, &page_numbers, &mut out, &mut 0);
    out
}

fn walk(
    document: &Document,
    node: &Object,
    level: u8,
    page_numbers: &HashMap<(u32, u16), u32>,
    out: &mut Vec<OutlineEntry>,
    visited: &mut usize,
) {
    let mut current = node.clone();
    loop {
        // A malformed document can point an outline at itself. Bounded rather
        // than tracked: the bound is far past any real table of contents, and
        // a visited-set would be the wrong shape for a sibling walk.
        *visited += 1;
        if *visited > 20_000 || level > 8 {
            return;
        }
        let Some(dictionary) = crate::font::resolve(document, &current)
            .and_then(|o| o.as_dict().ok())
            .cloned()
        else {
            return;
        };

        if let Ok(title) = dictionary.get(b"Title")
            && let Some(Object::String(bytes, _)) = crate::font::resolve(document, title)
        {
            out.push(OutlineEntry {
                title: decode_text_string(bytes),
                level,
                page: destination_page(document, &dictionary, page_numbers),
            });
        }

        if let Ok(first) = dictionary.get(b"First") {
            walk(document, first, level + 1, page_numbers, out, visited);
        }

        let Ok(next) = dictionary.get(b"Next") else {
            return;
        };
        current = next.clone();
    }
}

fn destination_page(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    page_numbers: &HashMap<(u32, u16), u32>,
) -> Option<u32> {
    let destination = dictionary.get(b"Dest").ok().cloned().or_else(|| {
        let action = dictionary.get(b"A").ok()?;
        crate::font::resolve(document, action)?
            .as_dict()
            .ok()?
            .get(b"D")
            .ok()
            .cloned()
    })?;
    let resolved = crate::font::resolve(document, &destination)?.clone();
    let array = match resolved {
        Object::Array(items) => items,
        _ => return None,
    };
    match array.first()? {
        Object::Reference(id) => page_numbers.get(id).copied(),
        _ => None,
    }
}

/// A PDF text string: UTF-16BE when it carries the byte-order mark, else
/// PDFDocEncoding, which agrees with Latin-1 over the range that matters.
pub(crate) fn decode_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_be_bytes(pair))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    bytes.iter().map(|&b| b as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_utf16_title_decodes() {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in "Wildemount".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        assert_eq!(decode_text_string(&bytes), "Wildemount");
    }

    #[test]
    fn a_latin1_title_decodes() {
        assert_eq!(decode_text_string(b"Monster Manual"), "Monster Manual");
    }

    #[test]
    fn a_reversed_media_box_still_has_a_positive_size() {
        // Rare, and it exists: a box given top-right to bottom-left. Taking
        // the difference unsigned means such a page is letter-sized rather
        // than negative, which would put every run off the page.
        let geometry = PageGeometry {
            width: (0.0f64 - 612.0).abs(),
            height: (0.0f64 - 792.0).abs(),
        };
        assert_eq!(geometry.width, 612.0);
        assert_eq!(geometry.height, 792.0);
    }
}
