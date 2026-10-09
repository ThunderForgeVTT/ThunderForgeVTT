//! The reader a system pack provides (spec 048 FR-001, FR-004).
//!
//! A reader recognises its own kind of sheet and reads it into an
//! [`ImportedCharacter`]. It is pure: the same bytes always give the same
//! reading, which is what lets the browser's reading and the server's agree.

use std::fmt;

use thunderforge_pdf::{Document, PdfError};

use crate::character::ImportedCharacter;

/// Whether a reader takes a document as its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recognition {
    Yes,
    No { reason: String },
}

impl Recognition {
    pub fn is_yes(&self) -> bool {
        matches!(self, Recognition::Yes)
    }
}

/// Why a document could not be read. Nothing is applied in part.
#[derive(Debug)]
pub enum ReadError {
    Pdf(PdfError),
    /// The reader does not take this document; the reason is shown.
    NotRecognised(String),
    /// Recognised, but a page or a required block could not be read.
    Page {
        page: u32,
        reason: String,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Pdf(error) => write!(f, "{error}"),
            ReadError::NotRecognised(reason) => f.write_str(reason),
            ReadError::Page { page, reason } => write!(f, "page {page}: {reason}"),
        }
    }
}

impl std::error::Error for ReadError {}

impl From<PdfError> for ReadError {
    fn from(error: PdfError) -> Self {
        ReadError::Pdf(error)
    }
}

/// A pack's reader of one kind of sheet.
pub trait SheetReader: Sync {
    /// Stable, such as "ddb-pdf". Recorded with every import.
    fn id(&self) -> &'static str;
    /// Changes whenever the reading of the same bytes could change.
    fn version(&self) -> &'static str;
    fn recognise(&self, doc: &Document) -> Recognition;
    fn read(&self, doc: &Document) -> Result<ImportedCharacter, ReadError>;
}
