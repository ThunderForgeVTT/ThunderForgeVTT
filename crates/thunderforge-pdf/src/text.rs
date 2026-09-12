//! The text machine: where each run of text sits, and how big it is.
//!
//! A PDF does not contain lines. It contains instructions that place glyphs,
//! and any notion of a line, a column or a heading is something a reader
//! infers afterwards. This module runs the placement instructions and records
//! what came out; `layout` does the inferring.
//!
//! # What is tracked, and what is not
//!
//! Tracked: the text matrix and line matrix (`Tm`, `Td`, `TD`, `T*`, `TL`),
//! the font and size (`Tf`), horizontal scale (`Tz`), rise (`Ts`), and the
//! current transformation matrix (`cm`, `q`, `Q`) — because a book that lays
//! a sidebar out with a transform puts its text nowhere near where the text
//! matrix alone would say.
//!
//! Not tracked: glyph advance widths. A run's extent is taken from where the
//! *next* placement lands, because real books position almost every run
//! explicitly. Summing widths would mean reading every font's metrics and
//! would still be wrong wherever a designer overrode them.

use crate::content::{Operand, Operation, operations};
use crate::font::{FontInfo, fonts_for_page};
use crate::page::{Page, decode_text_string};
use crate::{PdfError, page};
use lopdf::Document;

/// A 2×3 affine matrix, PDF order: `[a b c d e f]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix([f64; 6]);

impl Matrix {
    pub const IDENTITY: Matrix = Matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    fn multiply(self, other: Matrix) -> Matrix {
        let (a, b) = (self.0, other.0);
        Matrix([
            a[0] * b[0] + a[1] * b[2],
            a[0] * b[1] + a[1] * b[3],
            a[2] * b[0] + a[3] * b[2],
            a[2] * b[1] + a[3] * b[3],
            a[4] * b[0] + a[5] * b[2] + b[4],
            a[4] * b[1] + a[5] * b[3] + b[5],
        ])
    }

    fn translation(&self) -> (f64, f64) {
        (self.0[4], self.0[5])
    }

    /// How much this matrix scales vertically — what turns a font's nominal
    /// size into the size it is actually drawn at.
    fn vertical_scale(&self) -> f64 {
        (self.0[2].powi(2) + self.0[3].powi(2)).sqrt()
    }
}

/// The text state a run was drawn under.
#[derive(Debug, Clone, PartialEq)]
pub struct TextState {
    pub font: FontInfo,
    /// The size the text is actually drawn at: the font size through every
    /// scale in force. A book that sets 10pt type and scales the whole frame
    /// by 1.4 is drawing 14pt type, and a heading detector comparing nominal
    /// sizes would not see it.
    pub size: f64,
}

/// One run of text, placed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextRun {
    pub text: String,
    /// Page coordinates, origin bottom-left, PDF units.
    pub x: f64,
    pub y: f64,
    /// The size this run is drawn at.
    pub size: f64,
    pub font: String,
    pub bold: bool,
    pub italic: bool,
}

impl TextRun {
    /// Whether this run is only whitespace.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Clone)]
struct Graphics {
    ctm: Matrix,
}

/// Run one page's content stream and collect its text.
pub fn runs_on_page(document: &Document, page: &Page) -> Result<Vec<TextRun>, PdfError> {
    let fonts = fonts_for_page(document, page.id);
    let operations = operations(document, page.id);
    if operations.is_empty() {
        // A page that draws no text is not a failure — a full-page map, a
        // cover, a scan. The caller distinguishes "nothing here" from "could
        // not read" by asking whether the document opened at all.
        return Ok(Vec::new());
    }

    let mut stack: Vec<Graphics> = Vec::new();
    let mut graphics = Graphics {
        ctm: Matrix::IDENTITY,
    };
    let mut text_matrix = Matrix::IDENTITY;
    let mut line_matrix = Matrix::IDENTITY;
    let mut leading = 0.0f64;
    let mut horizontal_scale = 1.0f64;
    let mut rise = 0.0f64;
    let mut font = FontInfo::default();
    let mut encoding: Option<&lopdf::Encoding> = None;
    let mut font_size = 0.0f64;
    let mut out = Vec::new();

    for Operation { operator, operands } in operations {
        let number = |index: usize| operands.get(index).and_then(Operand::as_number);
        match operator.as_str() {
            "q" => stack.push(graphics.clone()),
            "Q" => {
                if let Some(saved) = stack.pop() {
                    graphics = saved;
                }
            }
            "cm" => {
                if operands.len() >= 6 {
                    let mut values = [0.0; 6];
                    for (index, slot) in values.iter_mut().enumerate() {
                        *slot = number(index).unwrap_or(0.0);
                    }
                    graphics.ctm = Matrix(values).multiply(graphics.ctm);
                }
            }
            "BT" => {
                text_matrix = Matrix::IDENTITY;
                line_matrix = Matrix::IDENTITY;
            }
            "ET" => {}
            "Tf" => {
                if let Some(Operand::Name(name)) = operands.first() {
                    font = fonts.info.get(name).cloned().unwrap_or_default();
                    encoding = fonts.encodings.get(name);
                }
                font_size = number(1).unwrap_or(font_size);
            }
            "TL" => leading = number(0).unwrap_or(leading),
            "Tz" => horizontal_scale = number(0).unwrap_or(100.0) / 100.0,
            "Ts" => rise = number(0).unwrap_or(0.0),
            "Td" => {
                let (tx, ty) = (number(0).unwrap_or(0.0), number(1).unwrap_or(0.0));
                line_matrix = Matrix([1.0, 0.0, 0.0, 1.0, tx, ty]).multiply(line_matrix);
                text_matrix = line_matrix;
            }
            "TD" => {
                let (tx, ty) = (number(0).unwrap_or(0.0), number(1).unwrap_or(0.0));
                leading = -ty;
                line_matrix = Matrix([1.0, 0.0, 0.0, 1.0, tx, ty]).multiply(line_matrix);
                text_matrix = line_matrix;
            }
            "Tm" => {
                if operands.len() >= 6 {
                    let mut values = [0.0; 6];
                    for (index, slot) in values.iter_mut().enumerate() {
                        *slot = number(index).unwrap_or(0.0);
                    }
                    line_matrix = Matrix(values);
                    text_matrix = line_matrix;
                }
            }
            "T*" => {
                line_matrix = Matrix([1.0, 0.0, 0.0, 1.0, 0.0, -leading]).multiply(line_matrix);
                text_matrix = line_matrix;
            }
            "Tj" | "'" | "\"" => {
                // `'` and `"` move to the next line first; `"` also sets
                // spacing, which this does not track.
                if operator != "Tj" {
                    line_matrix = Matrix([1.0, 0.0, 0.0, 1.0, 0.0, -leading]).multiply(line_matrix);
                    text_matrix = line_matrix;
                }
                if let Some(Operand::Bytes(bytes)) = operands.last() {
                    push_run(
                        &mut out,
                        bytes,
                        &text_matrix,
                        &graphics,
                        &font,
                        encoding,
                        font_size,
                        horizontal_scale,
                        rise,
                    );
                }
            }
            "TJ" => {
                if let Some(Operand::Array(items)) = operands.first() {
                    // Kerning numbers between the strings are what separate
                    // words in many books. They are not applied to the
                    // position here — the runs are joined by `layout`, which
                    // decides what a gap means from the run positions it can
                    // see. Applying them would need the font's width table.
                    let joined: Vec<u8> = items
                        .iter()
                        .filter_map(|item| match item {
                            Operand::Bytes(bytes) => Some(bytes.clone()),
                            _ => None,
                        })
                        .flatten()
                        .collect();
                    if !joined.is_empty() {
                        push_run(
                            &mut out,
                            &joined,
                            &text_matrix,
                            &graphics,
                            &font,
                            encoding,
                            font_size,
                            horizontal_scale,
                            rise,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn push_run(
    out: &mut Vec<TextRun>,
    bytes: &[u8],
    text_matrix: &Matrix,
    graphics: &Graphics,
    font: &FontInfo,
    encoding: Option<&lopdf::Encoding>,
    font_size: f64,
    horizontal_scale: f64,
    rise: f64,
) {
    // The font's own encoding first: a subsetted font renumbers its glyphs,
    // and reading its codes as Latin-1 produces confident gibberish.
    let text = encoding
        .and_then(|encoding| encoding.bytes_to_string(bytes).ok())
        .unwrap_or_else(|| decode_text_string(bytes));
    if text.is_empty() {
        return;
    }
    let placed = text_matrix.multiply(graphics.ctm);
    let (x, y) = placed.translation();
    let size = font_size * placed.vertical_scale() * horizontal_scale.max(0.01).min(10.0);
    out.push(TextRun {
        text,
        x,
        y: y + rise,
        // Negative or absurd sizes come from broken documents; clamp rather
        // than drop, because the words are still worth having.
        size: if size.is_finite() && size > 0.0 {
            size
        } else {
            font_size.abs().max(1.0)
        },
        font: font.base_font.clone(),
        bold: font.bold,
        italic: font.italic,
    });
}

#[allow(unused_imports)]
use page as _page_used_for_docs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplying_by_the_identity_changes_nothing() {
        let m = Matrix([2.0, 0.0, 0.0, 3.0, 10.0, 20.0]);
        assert_eq!(m.multiply(Matrix::IDENTITY), m);
        assert_eq!(Matrix::IDENTITY.multiply(m), m);
    }

    #[test]
    fn a_scaling_transform_changes_the_size_text_is_drawn_at() {
        // The case a nominal-size heading detector misses: 10pt type inside a
        // frame scaled by 1.4 is 14pt on the page.
        let scaled = Matrix([1.4, 0.0, 0.0, 1.4, 0.0, 0.0]);
        assert!((scaled.vertical_scale() - 1.4).abs() < 1e-9);
    }

    #[test]
    fn translation_composes() {
        let a = Matrix([1.0, 0.0, 0.0, 1.0, 5.0, 7.0]);
        let b = Matrix([1.0, 0.0, 0.0, 1.0, 100.0, 200.0]);
        assert_eq!(a.multiply(b).translation(), (105.0, 207.0));
    }

    #[test]
    fn a_run_of_only_spaces_is_blank() {
        let run = TextRun {
            text: "   ".into(),
            x: 0.0,
            y: 0.0,
            size: 10.0,
            font: "F".into(),
            bold: false,
            italic: false,
        };
        assert!(run.is_blank());
    }
}
