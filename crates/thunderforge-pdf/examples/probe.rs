//! Look closely at a handful of documents: pages, and where text starts.
//!
//!     cargo run -p thunderforge-pdf --example probe -- <file>...

#![allow(clippy::print_stdout)] // a command-line tool: its output is the point

use thunderforge_pdf::{Document, layout};

fn main() {
    for path in std::env::args().skip(1) {
        match Document::open(&path) {
            Err(error) => println!("{path}: {error}"),
            Ok(document) => {
                let pages = document.pages();
                let mut first_text: Option<(u32, usize)> = None;
                let mut total = 0usize;
                for page in pages.iter().take(60) {
                    let Ok(runs) = document.runs(page) else {
                        continue;
                    };
                    let chars: usize = layout::lines(&runs)
                        .iter()
                        .map(|l| l.text.chars().count())
                        .sum();
                    total += chars;
                    if chars > 100 && first_text.is_none() {
                        first_text = Some((page.number, chars));
                    }
                }
                let name = path.rsplit('/').next().unwrap_or(&path);
                println!(
                    "{:52.52} pages {:4}  first text: {:>12}  chars in 60pp {}",
                    name,
                    pages.len(),
                    first_text
                        .map(|(p, _)| format!("page {p}"))
                        .unwrap_or_else(|| "none".into()),
                    total
                );
            }
        }
    }
}
