//! Read a directory of PDFs and report what this crate got out of them.
//!
//! Not a test — a measurement. Run against a real library it says how many
//! documents opened, how many yielded text, and where the parser is weak:
//!
//!     cargo run -p thunderforge-pdf --example survey -- <dir> [pages-per-doc]
//!
//! Deliberately reports only counts and short samples. A library of
//! commercial books is not something to page through in a terminal, and the
//! question here is whether the parser works, not what the books say.

use std::path::PathBuf;
use thunderforge_pdf::{Document, layout};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().expect("usage: survey <dir> [pages]"));
    let sample_pages: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);

    let mut files: Vec<PathBuf> = Vec::new();
    collect(&root, &mut files);
    files.sort();
    eprintln!("{} PDFs under {}", files.len(), root.display());

    let (mut opened, mut failed, mut with_text, mut silent) = (0, 0, 0, 0);
    let mut pages_read = 0usize;
    let mut lines_read = 0usize;
    let mut headings = 0usize;
    let mut damaged_lines = 0usize;
    let mut with_outline = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut no_pages = 0usize;
    let mut empties: Vec<String> = Vec::new();

    for path in &files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let document = match Document::open(path) {
            Ok(document) => document,
            Err(error) => {
                failed += 1;
                if failures.len() < 12 {
                    failures.push(format!("{name}: {error}"));
                }
                continue;
            }
        };
        opened += 1;
        if document.pages().is_empty() {
            no_pages += 1;
            if empties.len() < 10 {
                empties.push(name.to_string());
            }
            continue;
        }
        if !document.outline().is_empty() {
            with_outline += 1;
        }

        let mut text_here = 0usize;
        for page in document.pages().into_iter().take(sample_pages) {
            let Ok(runs) = document.runs(&page) else {
                continue;
            };
            pages_read += 1;
            let assembled = layout::lines(&runs);
            let body = layout::body_size(&assembled);
            for line in layout::reading_order(assembled, page.geometry) {
                if line.text.trim().is_empty() {
                    continue;
                }
                lines_read += 1;
                text_here += line.text.chars().count();
                if layout::is_heading(&line, body) {
                    headings += 1;
                }
                if layout::looks_letter_spaced(&line.text) {
                    damaged_lines += 1;
                }
            }
        }
        if text_here > 200 {
            with_text += 1;
        } else {
            silent += 1;
        }
    }

    println!("\n--- documents ---");
    println!("  opened            {opened}");
    println!("  failed to open    {failed}");
    println!("  open but 0 pages  {no_pages}");
    println!("  yielded text      {with_text}");
    println!("  silent (scans?)   {silent}");
    println!("  have an outline   {with_outline}");
    println!("\n--- from the first {sample_pages} pages of each ---");
    println!("  pages read        {pages_read}");
    println!("  lines             {lines_read}");
    println!(
        "  headings          {headings}  ({:.1}% of lines)",
        percent(headings, lines_read)
    );
    println!(
        "  letter-spaced     {damaged_lines}  ({:.1}% of lines)",
        percent(damaged_lines, lines_read)
    );
    if !empties.is_empty() {
        println!("\n--- opened with no page tree (object streams?) ---");
        for name in &empties {
            println!("  {name}");
        }
    }
    if !failures.is_empty() {
        println!("\n--- documents that would not open ---");
        for failure in &failures {
            println!("  {failure}");
        }
    }
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 * 100.0 / whole as f64
    }
}

fn collect(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            out.push(path);
        }
    }
}
