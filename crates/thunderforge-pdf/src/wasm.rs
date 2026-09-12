//! Reading a PDF in the browser.
//!
//! # Why the client, and why that is safe here
//!
//! A source book is read into a world by its **Game Master, from their own
//! world panel**. That is the whole trust argument: a Game Master can already
//! put anything they like into their own world through the authoring tools,
//! so parsing on their machine hands them nothing they did not already have.
//! What it saves is the instance's compute and the round trip — a 70 MB book
//! never leaves the desk it is sitting on.
//!
//! A **player** importing their own character sheet is a different question
//! with a different answer, and the answer is not "trust the parse". It is
//! spec 048's guards: content the world has not adopted is withheld from the
//! play field, a Game Master can roll an import back, and an attempt to use
//! what was never delivered is refused by the server and reported. Those hold
//! whether the parse happened here or there, because they never trusted the
//! parse in the first place.
//!
//! # What this is not
//!
//! Not a second parser. Every function here is a thin wrapper over the same
//! code the server runs, compiled for a different target. Two implementations
//! of "what does this book say" would be the worst of both.

use wasm_bindgen::prelude::*;

use crate::{Document, layout};

/// One line of a page, as the readers above this want it.
#[derive(serde::Serialize)]
struct WasmLine {
    text: String,
    page: u32,
    size: f64,
    bold: bool,
    italic: bool,
    /// Whether this line is set larger or bolder than the body around it.
    heading: bool,
    /// Whether the text looks damaged enough not to be trusted — a
    /// letter-spaced title, or a font this build cannot decode.
    suspect: bool,
}

#[derive(serde::Serialize)]
struct WasmDocument {
    pages: usize,
    lines: Vec<WasmLine>,
    /// How many pages yielded no text at all. A book that is entirely scans
    /// reports every page here, which is what tells a caller to stop rather
    /// than to try harder.
    silent_pages: usize,
    /// Whether the cross-reference table had to be rebuilt to open it.
    repaired: bool,
}

/// Read a PDF's text, in reading order, with the styling a reader needs.
///
/// `from` and `count` page the work, because a 350-page book is seconds of
/// arithmetic and a browser tab that stops answering is worse than a slow
/// import. A caller walks the book a chunk at a time and can show progress.
///
/// Returns JSON. Errors are returned as a thrown `JsValue` carrying the same
/// sentence the server would report, so a person sees one message wherever
/// the parse happened.
#[wasm_bindgen]
pub fn read_pdf(bytes: &[u8], from: u32, count: u32) -> Result<String, JsValue> {
    // Whether it needed repairing is worth reporting rather than hiding: 37%
    // of a real library does, and a caller that knows can say "this file was
    // damaged and we read it anyway" instead of leaving a person wondering.
    let repaired = lopdf::Document::load_mem(bytes).is_err();

    let document =
        Document::from_bytes(bytes).map_err(|error| JsValue::from_str(&error.to_string()))?;

    let pages = document.pages();
    let mut lines = Vec::new();
    let mut silent_pages = 0usize;

    for page in pages
        .iter()
        .filter(|page| page.number >= from.max(1))
        .take(count as usize)
    {
        let Ok(runs) = document.runs(page) else {
            silent_pages += 1;
            continue;
        };
        let assembled = layout::lines(&runs);
        if assembled.is_empty() {
            silent_pages += 1;
            continue;
        }
        let body = layout::body_size(&assembled);
        for line in layout::reading_order(assembled, page.geometry) {
            if line.text.trim().is_empty() {
                continue;
            }
            lines.push(WasmLine {
                heading: layout::is_heading(&line, body),
                suspect: layout::looks_letter_spaced(&line.text)
                    || layout::looks_unreadable(&line.text),
                page: page.number,
                size: line.size,
                bold: line.bold,
                italic: line.italic,
                text: line.text,
            });
        }
    }

    serde_json::to_string(&WasmDocument {
        pages: pages.len(),
        lines,
        silent_pages,
        repaired,
    })
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

/// How many pages a document has, without reading any of them.
///
/// So a caller can size the work before starting it, and show a book's length
/// before it has read a word of it.
#[wasm_bindgen]
pub fn pdf_page_count(bytes: &[u8]) -> Result<usize, JsValue> {
    Document::from_bytes(bytes)
        .map(|document| document.page_count())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

/// A document's own table of contents, as JSON. Frequently absent — only a
/// third of real books have one — which is why nothing may depend on it.
#[wasm_bindgen]
pub fn pdf_outline(bytes: &[u8]) -> Result<String, JsValue> {
    let document =
        Document::from_bytes(bytes).map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&document.outline())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

/// Read a system's declared content out of a book, in the browser.
///
/// `patterns` is the system pack's `contentPatterns` block, verbatim. This
/// function knows nothing about any game: every label it looks for arrives in
/// that argument, which is what lets a second game system be a pack change
/// rather than a change here (spec 049 FR-010 to FR-016, ADR-096).
///
/// # Why this exists at all
///
/// Spec 049 FR-020: nothing read leaves the Game Master's machine before they
/// submit. That means the *entries*, not just the lines, have to be built
/// here — a browser that shipped its lines away to be interpreted would have
/// sent the book. And FR-036 says the server re-checks what arrives, so the
/// same readers run there too, from `thunderforge_content`. One crate,
/// compiled twice.
///
/// Paged like [`read_pdf`], and for the same reason.
#[wasm_bindgen]
pub fn read_content(
    bytes: &[u8],
    patterns: &str,
    from: usize,
    count: usize,
) -> Result<String, JsValue> {
    let declared: thunderforge_canvas_core::content_patterns::ContentPatterns =
        serde_json::from_str(patterns)
            .map_err(|error| JsValue::from_str(&format!("could not read the patterns: {error}")))?;
    if declared.is_empty() {
        // A refusal rather than an empty result. A system that declares
        // nothing has no correct reading, and returning zero entries would be
        // indistinguishable from a book that simply contained none.
        return Err(JsValue::from_str(
            "this game system declares no content patterns, so a book cannot be read into it",
        ));
    }

    let document = Document::from_bytes(bytes)
        .map_err(|error| JsValue::from_str(&format!("could not read the document: {error}")))?;

    let mut lines: Vec<thunderforge_content::SourceLine> = Vec::new();
    for page in document
        .pages()
        .into_iter()
        .skip(from.saturating_sub(1))
        .take(count)
    {
        let Ok(runs) = document.runs(&page) else {
            continue;
        };
        let assembled = layout::lines(&runs);
        let body = layout::body_size(&assembled);
        for line in layout::reading_order(assembled, page.geometry) {
            if line.text.trim().is_empty() {
                continue;
            }
            let suspect =
                layout::looks_letter_spaced(&line.text) || layout::looks_unreadable(&line.text);
            let heading = layout::is_heading(&line, body);
            lines.push(thunderforge_content::SourceLine {
                text: line.text,
                size: line.size,
                bold: line.bold,
                page: page.number,
                suspect,
                heading,
            });
        }
    }

    let mut entries: Vec<thunderforge_content::Entry> = Vec::new();
    for pattern in &declared.patterns {
        let found = match pattern.shape {
            thunderforge_canvas_core::content_patterns::Shape::Anchored => {
                thunderforge_content::anchored::entries(&lines, pattern)
            }
            thunderforge_canvas_core::content_patterns::Shape::Prose => {
                thunderforge_content::prose::entries(&lines, pattern)
            }
        };
        entries.extend(found);
    }

    serde_json::to_string(&entries)
        .map_err(|error| JsValue::from_str(&format!("could not report what was read: {error}")))
}
