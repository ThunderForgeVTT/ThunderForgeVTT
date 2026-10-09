//! The values a fillable form keeps in its fields (spec 048, research R2).
//!
//! A form can hold what it says without drawing it. D&D Beyond's character
//! export does exactly that: the page content draws the labels, and every
//! value lives in a widget annotation's `/V`, with no appearance stream and
//! no `/AcroForm` entry in the catalog to announce it. Reading the drawn text
//! alone finds the labels and nothing else.
//!
//! This module reads those widgets. Like [`crate::region`] it knows nothing
//! about any game: a field is a name, a value and a box, in the same top-left
//! coordinates a [`Rect`] always uses.

use lopdf::{Dictionary, Object};

use crate::Rect;
use crate::font::resolve;
use crate::page::decode_text_string;

/// What kind of field a widget belongs to (`/FT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FieldKind {
    Text,
    Button,
    Choice,
    Signature,
    Other,
}

/// One widget's field, as the document keeps it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FormField {
    /// The fully qualified name: each ancestor's `/T`, joined with `.`.
    /// Kept exactly, spaces and all; [`crate::PageText::field`] trims.
    pub name: String,
    /// `/V` as text. A button's value is its state name ("Yes", "Off"); a
    /// field with no value reads as empty.
    pub value: String,
    pub kind: FieldKind,
    /// One-based.
    pub page: u32,
    pub rect: Rect,
}

/// How far up a field's parents are followed. Real forms nest two or three
/// deep; a cycle must not hang the reader.
const MAX_DEPTH: usize = 8;

/// Every widget on one page that belongs to a named field, top first, then
/// left to right.
pub(crate) fn fields_on_page(
    document: &lopdf::Document,
    page_id: (u32, u16),
    number: u32,
    page_height: f64,
) -> Vec<FormField> {
    let Some(annots) = document
        .get_dictionary(page_id)
        .ok()
        .and_then(|page| page.get(b"Annots").ok())
        .and_then(|annots| resolve(document, annots))
        .and_then(|annots| annots.as_array().ok())
    else {
        return Vec::new();
    };
    let mut out: Vec<FormField> = annots
        .iter()
        .filter_map(|annot| resolve(document, annot)?.as_dict().ok())
        .filter(|annot| {
            annot.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget".as_slice())
        })
        .filter_map(|widget| field_of(document, widget, number, page_height))
        .collect();
    out.sort_by(|a, b| {
        a.rect
            .y0
            .total_cmp(&b.rect.y0)
            .then(a.rect.x0.total_cmp(&b.rect.x0))
    });
    out
}

fn field_of(
    document: &lopdf::Document,
    widget: &Dictionary,
    number: u32,
    page_height: f64,
) -> Option<FormField> {
    // The widget and its ancestors, nearest first.
    let mut chain: Vec<&Dictionary> = vec![widget];
    while chain.len() < MAX_DEPTH {
        let Some(parent) = chain
            .last()
            .and_then(|node| node.get(b"Parent").ok())
            .and_then(|parent| resolve(document, parent))
            .and_then(|parent| parent.as_dict().ok())
        else {
            break;
        };
        chain.push(parent);
    }

    let mut parts: Vec<String> = chain
        .iter()
        .rev()
        .filter_map(
            |node| match node.get(b"T").ok().and_then(|t| resolve(document, t)) {
                Some(Object::String(bytes, _)) => Some(decode_text_string(bytes)),
                _ => None,
            },
        )
        .collect();
    parts.retain(|part| !part.is_empty());
    if parts.is_empty() {
        return None;
    }

    let inherited = |key: &[u8]| {
        chain
            .iter()
            .find_map(|node| node.get(key).ok())
            .and_then(|value| resolve(document, value))
    };
    let kind = match inherited(b"FT").and_then(|ft| ft.as_name().ok()) {
        Some(b"Tx") => FieldKind::Text,
        Some(b"Btn") => FieldKind::Button,
        Some(b"Ch") => FieldKind::Choice,
        Some(b"Sig") => FieldKind::Signature,
        _ => FieldKind::Other,
    };
    let value = inherited(b"V")
        .map(|v| text_of(document, v))
        .unwrap_or_default();

    let numbers: Vec<f64> = widget
        .get(b"Rect")
        .ok()
        .and_then(|rect| resolve(document, rect))
        .and_then(|rect| rect.as_array().ok())?
        .iter()
        .filter_map(|n| match n {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(r) => Some(f64::from(*r)),
            _ => None,
        })
        .collect();
    let [ax, ay, bx, by] = numbers[..] else {
        return None;
    };
    Some(FormField {
        name: parts.join("."),
        value,
        kind,
        page: number,
        rect: Rect {
            x0: ax.min(bx) as f32,
            y0: (page_height - ay.max(by)) as f32,
            x1: ax.max(bx) as f32,
            y1: (page_height - ay.min(by)) as f32,
        },
    })
}

fn text_of(document: &lopdf::Document, value: &Object) -> String {
    match value {
        Object::String(bytes, _) => decode_text_string(bytes),
        Object::Name(name) => String::from_utf8_lossy(name).into_owned(),
        Object::Integer(i) => i.to_string(),
        Object::Real(r) => r.to_string(),
        // A multi-select choice field.
        Object::Array(items) => items
            .iter()
            .filter_map(|item| resolve(document, item))
            .map(|item| text_of(document, item))
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
