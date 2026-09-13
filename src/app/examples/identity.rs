//! Does an entry's identity survive a re-parse? (spec 050 FR-029, a gate.)
//!
//!     cargo run -p thunderforge --example identity -- <system-id> <kind> <file.pdf>...
//!
//! Spec 050 keys a world's delta to an entry by **kind and name**, so that a
//! Game Master's changes re-attach to the right entries after a book is read
//! again. FR-029 says that rule is settled by measurement rather than by
//! argument, because a rule invented at a desk for re-attaching somebody's
//! month of work is exactly the kind of guess this project has been burned by.
//!
//! # What the spec asked for, and why it cannot be run
//!
//! FR-029 names the experiment: re-parse books "that exist in more than one
//! file — a re-save, a later printing, or the same file under an improved
//! parser". The corpus has **no such pair**. 246 books, no repeated content
//! hash, and no title appearing twice. The experiment presupposes a property
//! of the library that the library does not have.
//!
//! # What is measured instead, and why it is worth more than nothing
//!
//! Two of FR-029's three questions can still be answered honestly, and the
//! first of them is the one that would kill the rule outright:
//!
//! 1. **Collisions.** How often do two entries in one book share a kind and a
//!    name? If that is common, kind-plus-name is dead regardless of how
//!    re-parses behave, because the rule cannot even identify an entry within
//!    a single reading.
//!
//! 2. **Stability under a processing change.** The browser reads a book in
//!    chunks of pages (`bookImport.ts`), the server may read it whole, and a
//!    parser improvement changes where boundaries fall. Reading the same file
//!    both ways is a real re-parse of exactly the kind that must not move an
//!    entry's identity — and unlike the spec's version, it exercises code that
//!    actually ships.
//!
//! **Renames are not measured.** Answering that needs two editions of one
//! work, and the corpus has none. Stated rather than papered over.

use std::collections::{BTreeMap, BTreeSet};

use blades_server as _;
use cypher_server as _;
use dnd5e_server as _;
use fate_server as _;
use genie_server as _;
use pathfinder2e_server as _;
use yze_server as _;

use thunderforge_canvas_core::content_entry::SourceLine;
use thunderforge_canvas_core::content_patterns::Shape;
use thunderforge_pdf::{Document, layout};
use thunderforge_server::content::{anchored, prose};
use thunderforge_server::content_patterns::content_patterns_for_system;

/// The page window the browser reads in. Must match `bookImport.ts`'s `CHUNK`;
/// the point of the comparison is to exercise what actually ships.
const CHUNK: u32 = 25;

fn main() {
    let mut args = std::env::args().skip(1);
    let system = args
        .next()
        .expect("usage: identity <system-id> <kind> <file>...");
    let kind = args
        .next()
        .expect("usage: identity <system-id> <kind> <file>...");

    let systems_dir =
        std::env::var("THUNDERFORGE_SYSTEMS_DIR").unwrap_or_else(|_| "packs/systems".to_string());
    let patterns = content_patterns_for_system(&systems_dir, &system);
    let Some(pattern) = patterns.for_kind(&kind) else {
        eprintln!("system '{system}' declares no '{kind}' pattern");
        std::process::exit(2);
    };

    let (mut books, mut whole_total, mut collided, mut only_whole, mut only_chunked) =
        (0usize, 0usize, 0usize, 0usize, 0usize);

    for path in args {
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let Ok(document) = Document::open(&path) else {
            continue;
        };

        let pages: Vec<u32> = document.pages().iter().map(|p| p.number).collect();
        if pages.is_empty() {
            continue;
        }
        let last = *pages.iter().max().unwrap_or(&0);

        let read = |from: u32, count: u32| -> Vec<SourceLine> {
            let mut lines = Vec::new();
            for page in document
                .pages()
                .iter()
                .filter(|p| p.number >= from.max(1))
                .take(count as usize)
            {
                let Ok(runs) = document.runs(page) else {
                    continue;
                };
                let assembled = layout::lines(&runs);
                if assembled.is_empty() {
                    continue;
                }
                let body = layout::body_size(&assembled);
                for line in layout::reading_order(assembled, page.geometry) {
                    if line.text.trim().is_empty() {
                        continue;
                    }
                    let suspect = layout::looks_letter_spaced(&line.text)
                        || layout::looks_unreadable(&line.text);
                    let heading = layout::is_heading(&line, body);
                    lines.push(SourceLine {
                        text: line.text,
                        size: line.size,
                        bold: line.bold,
                        page: page.number,
                        suspect,
                        heading,
                    });
                }
            }
            lines
        };

        let find = |lines: &[SourceLine]| match pattern.shape {
            Shape::Anchored => anchored::entries(lines, pattern),
            Shape::Prose => prose::entries(lines, pattern),
        };

        let whole = find(&read(1, last.saturating_add(1)));
        let mut chunked = Vec::new();
        let mut from = 1u32;
        while from <= last {
            chunked.extend(find(&read(from, CHUNK)));
            from += CHUNK;
        }

        // Identity exactly as spec 050 FR-025 defines it.
        let key = |e: &thunderforge_canvas_core::content_entry::Entry| {
            (e.kind.clone(), e.name.trim().to_lowercase())
        };

        let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
        for entry in &whole {
            *seen.entry(key(entry)).or_default() += 1;
        }
        let book_collisions: usize = seen.values().filter(|n| **n > 1).map(|n| n - 1).sum();

        let whole_keys: BTreeSet<_> = whole.iter().map(key).collect();
        let chunked_keys: BTreeSet<_> = chunked.iter().map(key).collect();
        let lost = whole_keys.difference(&chunked_keys).count();
        let gained = chunked_keys.difference(&whole_keys).count();

        if !whole.is_empty() {
            books += 1;
            whole_total += whole.len();
            collided += book_collisions;
            only_whole += lost;
            only_chunked += gained;
            if lost > 0 || gained > 0 {
                // Where a differing entry sits matters: if chunking is what
                // moves identity, the differences land on the seams.
                let page_of: BTreeMap<_, _> = whole.iter().map(|e| (key(e), e.page)).collect();
                for k in whole_keys.difference(&chunked_keys) {
                    let page = page_of.get(k).copied().unwrap_or(0);
                    println!(
                        "      lost   p{page:<5} (chunk seam at p{}) {}",
                        (page.saturating_sub(1) / CHUNK) * CHUNK + 1,
                        k.1
                    );
                }
                let page_of_chunked: BTreeMap<_, _> =
                    chunked.iter().map(|e| (key(e), e.page)).collect();
                for k in chunked_keys.difference(&whole_keys) {
                    let page = page_of_chunked.get(k).copied().unwrap_or(0);
                    println!(
                        "      gained p{page:<5} (chunk seam at p{}) {}",
                        (page.saturating_sub(1) / CHUNK) * CHUNK + 1,
                        k.1
                    );
                }
            }
            if lost > 0 || gained > 0 || book_collisions > 0 {
                println!(
                    "{name:44.44} {:5} entries {book_collisions:4} collided {lost:4} lost {gained:4} gained",
                    whole.len()
                );
            }
        }
    }

    println!(
        "\n{books} books, {whole_total} {kind}s read whole\n\
         {collided} share a kind and name with another entry in the same book\n\
         {only_whole} identities present reading whole but absent reading in chunks\n\
         {only_chunked} present only when read in chunks"
    );
}
