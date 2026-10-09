//! Asking what sits where on a page (spec 048 FR-001a).
//!
//! A form-like document, a character sheet first, is read by anchor: find
//! the label, then read what is beside it or under it. This module is that
//! question and nothing more. It knows no game and no sheet; a reader built
//! on it supplies the labels.
//!
//! # Coordinates
//!
//! A [`Rect`] is measured from the **top-left** of the page, with y growing
//! down, because that is how a person points at a box on a sheet and how a
//! review screen draws a highlight over it. PDF's own origin is the
//! bottom-left; the conversion happens once, here.
//!
//! # Lines
//!
//! The lines are the ones [`crate::layout::lines`] assembles, so a sheet and
//! a book agree on what a line is: runs on one baseline, split at a gutter.

use crate::layout::{self, Line};
use crate::text::TextRun;
use crate::{Document, FormField, PageGeometry, PdfError};

/// A box on a page, in points from the top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn centre_x(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }

    pub fn centre_y(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }

    /// Whether the two share any horizontal span.
    pub fn overlaps_horizontally(&self, other: &Rect) -> bool {
        self.x0 < other.x1 && other.x0 < self.x1
    }

    /// The smallest box holding both.
    pub fn union(&self, other: &Rect) -> Rect {
        Rect {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }
}

/// One line of text and the box it occupies.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PositionedLine {
    pub text: String,
    pub rect: Rect,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
}

/// How much of a line's size sits above the baseline, and below it.
///
/// Fonts differ; these are what Latin text averages, and they only have to
/// be good enough that two lines a line apart do not overlap.
const ASCENT: f64 = 0.8;
const DESCENT: f64 = 0.2;

/// Every line on one page, placed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PageText {
    /// One-based, as a reader would count.
    pub number: u32,
    pub geometry: PageGeometry,
    /// Top of the page first, then left to right.
    pub lines: Vec<PositionedLine>,
    /// The form fields whose widgets sit on this page, in the same order.
    /// A sheet that keeps its values in fields draws only its labels, so
    /// these hold what `lines` cannot (research R2).
    #[serde(default)]
    pub fields: Vec<FormField>,
}

impl PageText {
    /// Read one page of a document. `page` is one-based.
    ///
    /// A page turned by `/Rotate` is refused rather than read: its text runs
    /// sideways in page space, and "right of" would quietly mean "below".
    pub fn read(doc: &Document, page: u32) -> Result<PageText, PdfError> {
        let Some(found) = doc.pages().into_iter().find(|p| p.number == page) else {
            return Err(PdfError::Page {
                page,
                reason: format!("the document has {} pages", doc.page_count()),
            });
        };
        let degrees = crate::page::rotation(&doc.inner, found.id);
        if degrees != 0 {
            return Err(PdfError::Page {
                page,
                reason: format!("the page is rotated {degrees} degrees"),
            });
        }
        let runs = doc.runs(&found)?;
        let mut text = Self::from_runs(page, found.geometry, &runs);
        text.fields = doc.form_fields(&found);
        Ok(text)
    }

    /// Place runs already read. A reader's own tests build pages this way,
    /// with no PDF at all.
    pub fn from_runs(number: u32, geometry: PageGeometry, runs: &[TextRun]) -> PageText {
        let mut lines: Vec<PositionedLine> = layout::lines(runs)
            .into_iter()
            .map(|line| place(&line, geometry.height))
            .collect();
        lines.sort_by(|a, b| {
            a.rect
                .y0
                .total_cmp(&b.rect.y0)
                .then(a.rect.x0.total_cmp(&b.rect.x0))
        });
        PageText {
            number,
            geometry,
            lines,
            fields: Vec::new(),
        }
    }

    /// The field named `name`, ignoring spaces at either end of both.
    pub fn field(&self, name: &str) -> Option<&FormField> {
        let wanted = name.trim();
        self.fields.iter().find(|field| field.name.trim() == wanted)
    }

    /// Every place `label` appears, top of the page first.
    ///
    /// Matched on whole words, ignoring case and runs of spaces, so "SPEED"
    /// does not find "SPEEDY". When the label is part of a longer line, the
    /// box is narrowed to the label's share of it.
    pub fn find(&self, label: &str) -> Vec<Rect> {
        let wanted = fold(label);
        if wanted.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for line in &self.lines {
            let text = fold(&line.text);
            let total = text.chars().count();
            if total == 0 {
                continue;
            }
            for (start, _) in text.match_indices(&wanted) {
                let before = text[..start].chars().next_back();
                let after = text[start + wanted.len()..].chars().next();
                if before.is_some_and(char::is_alphanumeric)
                    || after.is_some_and(char::is_alphanumeric)
                {
                    continue;
                }
                let from = text[..start].chars().count() as f32 / total as f32;
                let to =
                    (text[..start].chars().count() + wanted.chars().count()) as f32 / total as f32;
                let width = line.rect.width();
                out.push(Rect {
                    x0: line.rect.x0 + width * from,
                    x1: line.rect.x0 + width * to,
                    ..line.rect
                });
            }
        }
        out
    }

    /// Lines whose centre falls inside `area`.
    pub fn within(&self, area: Rect) -> Vec<&PositionedLine> {
        self.lines
            .iter()
            .filter(|line| area.contains_point(line.rect.centre_x(), line.rect.centre_y()))
            .collect()
    }

    /// Lines on `of`'s band that start at or after its right edge, up to
    /// `max_dx` beyond it. Left to right.
    pub fn right_of(&self, of: Rect, max_dx: f32) -> Vec<&PositionedLine> {
        let mut out: Vec<&PositionedLine> = self
            .lines
            .iter()
            .filter(|line| {
                let centre = line.rect.centre_y();
                centre >= of.y0
                    && centre <= of.y1
                    && line.rect.x0 >= of.x1 - EDGE
                    && line.rect.x0 <= of.x1 + max_dx
            })
            .collect();
        out.sort_by(|a, b| a.rect.x0.total_cmp(&b.rect.x0));
        out
    }

    /// Lines that share some of `of`'s horizontal span and sit under it, no
    /// further than `max_dy` below its bottom edge. Top first.
    pub fn below(&self, of: Rect, max_dy: f32) -> Vec<&PositionedLine> {
        self.lines
            .iter()
            .filter(|line| {
                line.rect.overlaps_horizontally(&of)
                    && line.rect.centre_y() > of.y1
                    && line.rect.y0 <= of.y1 + max_dy
            })
            .collect()
    }
}

/// How far a line may start inside a label's right edge and still count as
/// beside it: a label's width is an estimate, and a tight form sets its value
/// a hair after the label ends.
const EDGE: f32 = 1.0;

fn place(line: &Line, page_height: f64) -> PositionedLine {
    PositionedLine {
        text: line.text.clone(),
        rect: Rect {
            x0: line.x0 as f32,
            y0: (page_height - line.y - line.size * ASCENT) as f32,
            x1: line.x1 as f32,
            y1: (page_height - line.y + line.size * DESCENT) as f32,
        },
        size: line.size as f32,
        bold: line.bold,
        italic: line.italic,
    }
}

/// What `find` compares: collapsed spaces, upper case.
fn fold(text: &str) -> String {
    layout::normalise(text).to_uppercase()
}

#[cfg(test)]
#[path = "region_tests.rs"]
mod tests;
