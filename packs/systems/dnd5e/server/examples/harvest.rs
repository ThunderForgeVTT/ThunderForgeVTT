//! Read every statblock out of a book, and report what was found.
//!
//!     cargo run -p dnd5e-server --example harvest -- <file.pdf>...
//!
//! Counts and a short sample only. The measurement is whether the reader
//! works, not what the books say.
use dnd5e_server::statblock::{statblocks, ReadDefault, SourceLine};
use thunderforge_pdf::{layout, Document};

fn main() {
    let mut grand_total = 0usize;
    let mut with_reach = 0usize;
    let mut uncertain = 0usize;

    for path in std::env::args().skip(1) {
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
            for line in layout::reading_order(assembled, page.geometry) {
                if line.text.trim().is_empty() {
                    continue;
                }
                lines.push(SourceLine {
                    text: line.text,
                    size: line.size,
                    bold: line.bold,
                    page: page.number,
                });
            }
        }

        let found = statblocks(&lines);
        let reaches = found
            .iter()
            .flat_map(|b| &b.actions)
            .filter(|a| a.reach_feet.is_some())
            .count();
        let shaky = found
            .iter()
            .filter(|b| b.confidence.name != ReadDefault::Clear)
            .count();
        grand_total += found.len();
        with_reach += reaches;
        uncertain += shaky;

        println!(
            "{name:44.44} {:4} creatures {reaches:4} reaches {shaky:3} uncertain",
            found.len()
        );
        for block in found.iter().take(2) {
            println!(
                "      e.g. {:26.26} AC {:>3} HP {:>4} {} action(s)",
                block.name,
                block.armor_class.map(|v| v.to_string()).unwrap_or_default(),
                block.hit_points.map(|v| v.to_string()).unwrap_or_default(),
                block.actions.len()
            );
        }
    }
    println!("\n{grand_total} creatures, {with_reach} attacks with a stated reach, {uncertain} names to check");
}
