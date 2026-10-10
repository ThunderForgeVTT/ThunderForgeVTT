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

impl ReadError {
    /// The refusal code the browser and the server both answer with
    /// (contracts/graphql-sheet-import.md, "Common errors").
    pub fn code(&self) -> &'static str {
        match self {
            ReadError::Pdf(PdfError::Encrypted) => "SHEET_ENCRYPTED",
            ReadError::Pdf(PdfError::TooLarge { .. }) => "SHEET_TOO_LARGE",
            ReadError::Pdf(PdfError::TooManyPages { .. }) => "SHEET_TOO_MANY_PAGES",
            ReadError::Pdf(PdfError::Unreadable(_) | PdfError::Page { .. })
            | ReadError::Page { .. } => "SHEET_UNREADABLE",
            ReadError::NotRecognised(_) => "SHEET_NOT_RECOGNISED",
        }
    }

    /// What a player reads, and what to do about it.
    pub fn sentence(&self) -> String {
        const MB: usize = 1024 * 1024;
        match self {
            ReadError::Pdf(PdfError::Encrypted) => {
                "The sheet is protected by a password. Export it again without one.".into()
            }
            ReadError::Pdf(PdfError::TooLarge { limit, .. }) => format!(
                "The file is over {} MB, which is more than a character sheet needs.",
                limit.div_ceil(MB)
            ),
            ReadError::Pdf(PdfError::TooManyPages { pages, limit }) => {
                format!("The file has {pages} pages, and a character sheet has at most {limit}.")
            }
            ReadError::Pdf(PdfError::Unreadable(_)) => {
                "The file could not be read as a PDF.".into()
            }
            ReadError::Pdf(PdfError::Page { page, .. }) => {
                format!("Page {page} of the file could not be read.")
            }
            ReadError::Page { page, reason } => {
                format!(
                    "Page {page} of the sheet could not be read: {}.",
                    reason.trim_end_matches('.')
                )
            }
            ReadError::NotRecognised(reason) => reason.clone(),
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
