//! Print the lines of a few pages, with size and weight. For designing a
//! reader against what books actually contain.
//!     cargo run -p thunderforge-pdf --example peek -- <file> <page> [count]
use thunderforge_pdf::{Document, layout};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: peek <file> <page> [count]");
    let first: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);

    let document = Document::open(&path).expect("opens");
    for page in document
        .pages()
        .iter()
        .filter(|p| p.number >= first)
        .take(count)
    {
        println!("=== page {} ===", page.number);
        let runs = document.runs(page).expect("runs");
        let assembled = layout::lines(&runs);
        let body = layout::body_size(&assembled);
        for line in layout::reading_order(assembled, page.geometry) {
            if line.text.trim().is_empty() {
                continue;
            }
            println!(
                "{:5.1}{}{} x{:6.1}-{:6.1} | {}",
                line.size,
                if line.bold { "B" } else { " " },
                if layout::is_heading(&line, body) {
                    "H"
                } else {
                    " "
                },
                line.x0,
                line.x1,
                line.text.chars().take(72).collect::<String>()
            );
        }
    }
}
