//! Every place the disclosure is written says Appendix A's words (FR-035,
//! SC-012). Markup and line breaks are ignored, so a re-wrapped paragraph or
//! a bullet turned into a table cell still passes; a changed word does not.

use super::startup_line::{OFF_LINE, ON_TAIL};

const SPEC: &str = include_str!("../../../../specs/086-full-telemetry/spec.md");
const README: &str = include_str!("../../../../README.md");
const GUIDE: &str = include_str!("../../../../docs/guides/telemetry.md");

/// The words, without Markdown or line breaks.
fn words(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = line.trim_start();
            let line = line.trim_start_matches(['#', '>', '|']);
            let line = line.strip_prefix("- ").unwrap_or(line);
            line.replace(['*', '`', '|'], " ")
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The fenced Markdown block under an appendix heading.
fn appendix_block(heading: &str) -> &'static str {
    let at = SPEC.find(heading).unwrap_or_else(|| panic!("{heading}"));
    let rest = &SPEC[at..];
    let open = rest.find("```markdown\n").expect("opening fence") + "```markdown\n".len();
    let body = &rest[open..];
    let close = body.find("\n```\n").expect("closing fence");
    &body[..close]
}

/// The A.3 line that starts with `label`.
fn a3(label: &str) -> String {
    let at = SPEC.find("### A.3").expect("A.3");
    let section = &SPEC[at..SPEC[at..].find("### A.4").map_or(SPEC.len(), |e| at + e)];
    let line = section
        .lines()
        .find(|l| l.starts_with(label))
        .unwrap_or_else(|| panic!("{label}"));
    let start = line.find('`').expect("quoted") + 1;
    let end = line.rfind('`').expect("quoted");
    line[start..end].to_string()
}

#[test]
fn the_readme_holds_a1() {
    let a1 = words(appendix_block("### A.1"));
    assert!(words(README).contains(&a1), "README.md differs from A.1");
}

#[test]
fn the_guide_holds_a2_in_full() {
    let a2 = words(appendix_block("### A.2"));
    assert!(
        words(GUIDE).contains(&a2),
        "docs/guides/telemetry.md differs from A.2"
    );
}

#[test]
fn the_startup_line_is_a3() {
    assert_eq!(OFF_LINE, a3("- **Off:**"));
    let on = a3("- **Otherwise:**");
    let tail = on.split(". ").skip(1).collect::<Vec<_>>().join(". ");
    assert_eq!(ON_TAIL, tail);
    assert!(on.starts_with(
        "telemetry: server → <server destination> (<tier>), browsers → <browser destination> (<tier>). "
    ));
}

#[test]
fn words_ignore_markup_and_wrapping() {
    assert_eq!(words("- **What:** `a`\n  b\n\n## C"), words("What: a b C"));
}
