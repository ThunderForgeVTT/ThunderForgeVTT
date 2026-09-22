//! Read every entry of one kind out of a book, through the generic reader.
//!
//!     cargo run -p thunderforge-app --example harvest -- <system-id> <kind> <file.pdf>...
//!
//! Counts and a short sample only. The measurement is whether the reader
//! works, not what the books say — a library of commercial books is not
//! something to page through in a terminal.
//!
//! # Why it lives here
//!
//! It exercises the whole chain, and `src/app` is the only crate that can see
//! all of it: the pack declares the pattern, `thunderforge-server` owns the
//! generic reader, and the pack contributes the refinement for what a
//! declaration cannot express. This crate is the composition root that links
//! packs in (`src/app/src/system_packs.rs`), so it is the only place the chain
//! is whole.
//!
//! The system id arrives as an **argument**, never a literal: shared code may
//! not name a system, and while `src/app/examples` is outside what
//! `check-system-registry` scans, writing one here would be writing it in the
//! one place nobody is watching.
//!
//! # The `use … as _` block is load-bearing
//!
//! An example links its package's *library*, and this package is a binary —
//! so `src/app/src/system_packs.rs`, which exists precisely to force pack
//! linkage, is not linked into an example at all. Without the same block
//! repeated here, `contribution_for` returns `None`, no refinement runs, and
//! the measurement silently reports zero reaches while looking like it worked.
//!
//! That is exactly the failure `system_packs.rs`'s own comment warns about,
//! met from a direction it does not cover. These imports are the same
//! load-bearing thing, for the same reason, and deleting one removes a system
//! from the measurement without any other sign.

#![allow(clippy::print_stdout)] // a command-line tool: its output is the point

// See "The `use … as _` block is load-bearing" above. Nothing here reads a
// symbol from these crates; referencing them at all is what links their
// `inventory` submissions in.
use blades_server as _;
use cypher_server as _;
use dnd5e_server as _;
use fate_server as _;
use genie_server as _;
use pathfinder2e_server as _;
use yze_server as _;

use thunderforge_canvas_core::content_entry::SourceLine;
use thunderforge_canvas_core::content_patterns::Shape;
use thunderforge_canvas_core::system_contribution::contribution_for;
use thunderforge_pdf::{Document, layout};
use thunderforge_server::content::{anchored, prose};
use thunderforge_server::content_patterns::content_patterns_for_system;

fn main() {
    let mut args = std::env::args().skip(1);
    let system = args
        .next()
        .expect("usage: harvest <system-id> <kind> <file.pdf>...");
    let kind = args
        .next()
        .expect("usage: harvest <system-id> <kind> <file>");

    let systems_dir =
        std::env::var("THUNDERFORGE_SYSTEMS_DIR").unwrap_or_else(|_| "packs/systems".to_string());
    let patterns = content_patterns_for_system(&systems_dir, &system);
    let Some(pattern) = patterns.for_kind(&kind) else {
        eprintln!(
            "system '{system}' declares no '{kind}' pattern in {systems_dir}/{system}/system.json"
        );
        std::process::exit(2);
    };
    let refine = contribution_for(&system).and_then(|c| c.refine_content);

    let (mut total, mut with_reach, mut uncertain) = (0usize, 0usize, 0usize);

    for path in args {
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let Ok(document) = Document::open(&path) else {
            println!("{name:44.44} could not open");
            continue;
        };

        let mut lines: Vec<SourceLine> = Vec::new();
        for page in document.pages() {
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
                lines.push(SourceLine {
                    text: line.text,
                    size: line.size,
                    bold: line.bold,
                    page: page.number,
                    // The same judgement the wasm boundary makes for the
                    // browser (`wasm.rs`), so a measurement here and an import
                    // there agree about which text is not to be trusted.
                    suspect,
                    heading,
                });
            }
        }

        let mut found = match pattern.shape {
            Shape::Anchored => anchored::entries(&lines, pattern),
            Shape::Prose => prose::entries(&lines, pattern),
        };
        if let Some(refine) = refine {
            // The refinement sees the lines the entry was built from, which is
            // what lets it read an attack's prose without the shared reader
            // ever knowing such a thing exists.
            for entry in &mut found {
                let slice: Vec<SourceLine> = lines
                    .iter()
                    .filter(|line| line.page == entry.page)
                    .cloned()
                    .collect();
                refine(entry, &slice);
            }
        }

        let reaches = found.iter().filter_map(reach_count).sum::<usize>();
        let unsure = found
            .iter()
            .filter(|entry| {
                entry.name_state == thunderforge_canvas_core::content_entry::NameState::Uncertain
            })
            .count();

        total += found.len();
        with_reach += reaches;
        uncertain += unsure;

        println!(
            "{name:44.44} {:4} {kind}s {reaches:5} reaches {unsure:3} uncertain",
            found.len()
        );
        for entry in found.iter().take(2) {
            println!("      e.g. {:30.30}", entry.name);
        }
    }

    println!(
        "\n{total} {kind}s, {with_reach} attacks with a stated reach, {uncertain} names to check"
    );
}

/// How many of an entry's refined actions stated a reach.
///
/// Reads the pack's own `extras` without understanding it beyond the shape it
/// agreed to publish — which is the whole point of `extras` being opaque.
fn reach_count(entry: &thunderforge_canvas_core::content_entry::Entry) -> Option<usize> {
    let actions = entry.extras.as_ref()?.get("actions")?.as_array()?;
    Some(
        actions
            .iter()
            .filter(|action| action.get("reachFeet").is_some_and(|r| !r.is_null()))
            .count(),
    )
}
