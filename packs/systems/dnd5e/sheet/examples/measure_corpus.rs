//! Measure the reader on a corpus of real exports (spec 048 T025, SC-002).
//!
//! No test runs this. Point `THUNDERFORGE_SHEET_CORPUS` at a directory of
//! D&D Beyond PDFs and it prints, per field, how many sheets read it plainly,
//! uncertainly or not at all. It never prints a value, nor an uncertain
//! value's reason, since a reason can quote what the sheet says.
//!
//! ```sh
//! THUNDERFORGE_SHEET_CORPUS=~/sheet-corpus \
//!   cargo run -p thunderforge-system-dnd5e-sheet --example measure_corpus
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)] // a command-line tool: its output is the point

use std::collections::BTreeMap;
use std::path::PathBuf;

use thunderforge_pdf::Document;
use thunderforge_sheet_import::{Certainty, ImportedCharacter, SheetReader};
use thunderforge_system_dnd5e_sheet::DdbPdf;

#[derive(Default)]
struct Counts {
    read: usize,
    uncertain: usize,
    unread: usize,
}

/// "classes.1.level" as "classes.*.level"; content by its kind.
fn pattern(path: &str, reading: &ImportedCharacter) -> String {
    if let Some(index) = path.strip_prefix("content.")
        && let Some(item) = index
            .parse::<usize>()
            .ok()
            .and_then(|i| reading.content.get(i))
    {
        return format!("content[{}]", item.kind);
    }
    path.split('.')
        .map(|part| {
            if part.parse::<usize>().is_ok() {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn main() {
    let Some(dir) = std::env::var_os("THUNDERFORGE_SHEET_CORPUS") else {
        eprintln!("THUNDERFORGE_SHEET_CORPUS is not set; nothing to measure.");
        return;
    };
    let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")))
            .collect(),
        Err(error) => {
            eprintln!("cannot list the corpus: {error}");
            std::process::exit(1);
        }
    };
    files.sort();

    let mut fields: BTreeMap<String, Counts> = BTreeMap::new();
    let (mut read_sheets, mut refused) = (0, 0);
    for (index, file) in files.iter().enumerate() {
        // Sheets are numbered, not named: a file name can be a person's.
        let label = format!("sheet {}", index + 1);
        let reading = Document::open(file)
            .map_err(|e| e.to_string())
            .and_then(|doc| DdbPdf.read(&doc).map_err(|e| e.to_string()));
        let reading = match reading {
            Ok(reading) => reading,
            Err(_) => {
                refused += 1;
                println!("{label}: not read");
                continue;
            }
        };
        read_sheets += 1;
        let mut sheet = Counts::default();
        for leaf in reading.leaves() {
            let counts = fields.entry(pattern(&leaf.path, &reading)).or_default();
            match leaf.certainty {
                Certainty::Read => (counts.read += 1, sheet.read += 1),
                Certainty::Uncertain { .. } => (counts.uncertain += 1, sheet.uncertain += 1),
                Certainty::Unread => (counts.unread += 1, sheet.unread += 1),
            };
        }
        println!(
            "{label}: {} read, {} uncertain, {} unread, {} content rows",
            sheet.read,
            sheet.uncertain,
            sheet.unread,
            reading.content.len()
        );
    }
    println!("\n{read_sheets} sheets read, {refused} not read\n");
    println!(
        "{:<44} {:>6} {:>10} {:>7}",
        "field", "read", "uncertain", "unread"
    );
    for (field, c) in &fields {
        println!(
            "{field:<44} {:>6} {:>10} {:>7}",
            c.read, c.uncertain, c.unread
        );
    }
}
