//! A page's content stream, as operations.
//!
//! Thin over `lopdf`: it already tokenises the stream. What this adds is a
//! shape the text machine can match on without knowing lopdf's object model,
//! so the interpreter in `text.rs` reads like the PDF spec's operator table
//! rather than like a tree walk.

use lopdf::content::Content;
use lopdf::{Document, Object};

/// One argument to an operator.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Number(f64),
    /// A string, still in the font's own encoding — decoding needs the font,
    /// which the text machine has and this layer does not.
    Bytes(Vec<u8>),
    Name(String),
    /// A `TJ` array: strings with kerning adjustments between them.
    Array(Vec<Operand>),
    Other,
}

impl Operand {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Operand::Number(n) => Some(*n),
            _ => None,
        }
    }
}

/// One operator and its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub operator: String,
    pub operands: Vec<Operand>,
}

fn operand_of(object: &Object) -> Operand {
    match object {
        Object::Integer(i) => Operand::Number(*i as f64),
        Object::Real(r) => Operand::Number(*r as f64),
        Object::String(bytes, _) => Operand::Bytes(bytes.clone()),
        Object::Name(name) => Operand::Name(String::from_utf8_lossy(name).into_owned()),
        Object::Array(items) => Operand::Array(items.iter().map(operand_of).collect()),
        _ => Operand::Other,
    }
}

/// Decode one page's content stream.
///
/// A page whose stream is missing or malformed yields nothing rather than an
/// error. In a library of scraped books that case is common — a cover page
/// that is one big image, a page whose stream uses a filter this build does
/// not have — and a single unreadable page must not cost the whole book.
pub fn operations(document: &Document, page_id: (u32, u16)) -> Vec<Operation> {
    let Ok(data) = document.get_page_content(page_id) else {
        return Vec::new();
    };
    let Ok(content) = Content::decode(&data) else {
        return Vec::new();
    };
    content
        .operations
        .into_iter()
        .map(|op| Operation {
            operator: op.operator,
            operands: op.operands.iter().map(operand_of).collect(),
        })
        .collect()
}
