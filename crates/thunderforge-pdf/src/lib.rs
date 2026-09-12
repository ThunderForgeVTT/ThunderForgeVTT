//! Reading a PDF: text, where it sits, and what it looks like.
//!
//! Knows nothing about games. It takes a document and yields text runs with
//! the position and styling needed to tell a heading from a value from a
//! table cell (spec 048 FR-001a), and everything above it — a character
//! sheet reader, a statblock reader, one day a source book — is a matter of
//! asking what sits at a given anchor.
//!
//! # Why not the table of contents
//!
//! Because most books do not have one. Measured against a 405-document
//! library of real tabletop PDFs, **132 have a PDF outline at all** — a third.
//! Of those, one in seven has headings the extractor mangles into
//! `"CALENDA R A N D TI M E"`, because the designer letter-spaced the title
//! and the spacing survives as literal spaces.
//!
//! So structure is derived from layout: a heading is text that is bigger, or
//! bolder, than the body around it. That works on the other two thirds too.
//!
//! # Why not Poppler
//!
//! `pdftotext` is excellent and is what `sorting-hat` uses to skim this same
//! corpus. It is also an external binary, which would make Poppler a
//! deployment dependency of the server, and it yields plain text — the one
//! thing this crate exists to go beyond.

mod content;
mod font;
mod page;
mod repair;
mod text;

pub use content::{Operand, Operation, operations};
pub use font::{FontInfo, FontMap};
pub use page::{Page, PageGeometry, pages};
pub use text::{TextRun, TextState, runs_on_page};

use std::path::Path;

/// What went wrong reading a document.
#[derive(Debug)]
pub enum PdfError {
    /// The file could not be opened or is not a PDF this parser understands.
    Unreadable(String),
    /// The document opened, but a specific page could not be read. Carries
    /// the page number so a caller can report which — a book with one broken
    /// page is still worth reading.
    Page { page: u32, reason: String },
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfError::Unreadable(why) => write!(f, "could not read the document: {why}"),
            PdfError::Page { page, reason } => write!(f, "page {page}: {reason}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// An open document.
pub struct Document {
    inner: lopdf::Document,
}

impl Document {
    /// Open a PDF from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PdfError> {
        let bytes = std::fs::read(path).map_err(|e| PdfError::Unreadable(e.to_string()))?;
        Self::from_bytes(&bytes)
    }

    /// Open a PDF already in memory.
    ///
    /// A document whose cross-reference table is unusable is repaired and
    /// tried again — see [`repair`]. That is not an edge case: 37% of a real
    /// 246-book library fails to open without it.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PdfError> {
        match lopdf::Document::load_mem(bytes) {
            Ok(inner) => Ok(Self { inner }),
            Err(first) => {
                let Some(repaired) = repair::rebuild_xref(bytes) else {
                    return Err(PdfError::Unreadable(first.to_string()));
                };
                lopdf::Document::load_mem(&repaired)
                    .map(|inner| Self { inner })
                    // The original complaint, not the repaired one: "invalid
                    // file trailer" says what is wrong with the document a
                    // person has, and a second failure after repair is this
                    // crate running out of ideas rather than new information.
                    .map_err(|_| PdfError::Unreadable(first.to_string()))
            }
        }
    }

    /// How many pages the document has.
    pub fn page_count(&self) -> usize {
        self.inner.get_pages().len()
    }

    /// The document's pages, in order.
    pub fn pages(&self) -> Vec<Page> {
        page::pages(&self.inner)
    }

    /// Every text run on one page, in the order the content stream draws them.
    ///
    /// **Draw order, not reading order.** A two-column page frequently draws
    /// its columns interleaved, and a designer may place a pull-quote
    /// anywhere at all. Ordering is [`layout`]'s job, and it needs to see
    /// every run before it can decide.
    pub fn runs(&self, page: &Page) -> Result<Vec<TextRun>, PdfError> {
        text::runs_on_page(&self.inner, page)
    }

    /// The document's own outline, when it has one.
    ///
    /// Only about a third of real books do, which is why nothing downstream
    /// may depend on it. Useful as corroboration, never as the source of
    /// structure.
    pub fn outline(&self) -> Vec<OutlineEntry> {
        page::outline(&self.inner)
    }
}

/// One entry in a document's own table of contents.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutlineEntry {
    pub title: String,
    pub level: u8,
    /// One-based, as a reader would count. `None` when the destination could
    /// not be resolved to a page, which happens in older documents.
    pub page: Option<u32>,
}

pub mod layout;
pub use repair::rebuild_xref;
