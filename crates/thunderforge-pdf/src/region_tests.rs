use super::*;
use crate::text::TextRun;

/// A run the way `layout_tests.rs` draws one: half an em a character.
fn run(text: &str, x: f64, y: f64, size: f64) -> TextRun {
    TextRun {
        width: text.chars().count() as f64 * size * 0.5,
        text: text.to_string(),
        x,
        y,
        size,
        font: "Body".into(),
        bold: false,
        italic: false,
    }
}

fn bold(text: &str, x: f64, y: f64, size: f64) -> TextRun {
    TextRun {
        bold: true,
        font: "Body-Bold".into(),
        ..run(text, x, y, size)
    }
}

const LETTER: PageGeometry = PageGeometry {
    width: 612.0,
    height: 792.0,
};

fn page(runs: &[TextRun]) -> PageText {
    PageText::from_runs(1, LETTER, runs)
}

fn texts(lines: &[&PositionedLine]) -> Vec<String> {
    lines.iter().map(|line| line.text.clone()).collect()
}

// ---------------------------------------------------------------------------
// Coordinates

#[test]
fn a_line_is_placed_from_the_top_of_the_page() {
    // Baseline 700 on a 792pt page: the line sits about 92pt from the top.
    let text = page(&[run("STRENGTH", 50.0, 700.0, 10.0)]);
    let rect = text.lines[0].rect;
    assert!(rect.y0 < rect.y1, "y grows down the page: {rect:?}");
    assert!(
        (rect.y1 - 94.0).abs() < 2.5,
        "bottom near 792 - 700: {rect:?}"
    );
    assert_eq!(rect.x0, 50.0);
    assert_eq!(rect.x1, 90.0, "eight characters at half an em of 10pt");
}

#[test]
fn bold_and_size_are_kept() {
    let text = page(&[bold("FEATURES & TRAITS", 50.0, 400.0, 12.0)]);
    assert!(text.lines[0].bold);
    assert_eq!(text.lines[0].size, 12.0);
}

// ---------------------------------------------------------------------------
// find

#[test]
fn find_returns_every_place_a_label_appears() {
    let text = page(&[
        run("SPEED", 50.0, 700.0, 8.0),
        run("SPEED", 400.0, 300.0, 8.0),
        run("SPEEDY", 50.0, 200.0, 8.0),
    ]);
    let found = text.find("SPEED");
    assert_eq!(found.len(), 2, "a longer word is not the label: {found:?}");
    assert!(found[0].y0 < found[1].y0, "top of the page first");
}

#[test]
fn find_ignores_case_and_runs_of_spaces() {
    let text = page(&[run("Class  &   Level", 50.0, 700.0, 8.0)]);
    assert_eq!(text.find("CLASS & LEVEL").len(), 1);
}

#[test]
fn find_narrows_to_the_label_inside_a_longer_line() {
    // "ARMOR CLASS 16" drawn as one line: the label's rect is its own part.
    let text = page(&[run("ARMOR CLASS 16", 100.0, 700.0, 10.0)]);
    let found = text.find("ARMOR CLASS");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].x0, 100.0);
    assert!(
        found[0].x1 < 160.0,
        "stops before the value: {:?}",
        found[0]
    );
}

#[test]
fn find_on_an_empty_label_finds_nothing() {
    let text = page(&[run("ANYTHING", 50.0, 700.0, 8.0)]);
    assert!(text.find("   ").is_empty());
}

// ---------------------------------------------------------------------------
// Lines at one y

#[test]
fn two_boxes_on_one_baseline_are_two_lines() {
    // A sheet draws STR and DEX side by side; the gap between is a gutter.
    let text = page(&[run("15", 50.0, 600.0, 10.0), run("14", 200.0, 600.0, 10.0)]);
    assert_eq!(text.lines.len(), 2);
    assert_eq!(text.lines[0].rect.y0, text.lines[1].rect.y0);
}

// ---------------------------------------------------------------------------
// right_of

#[test]
fn right_of_reads_the_value_beside_a_label() {
    let text = page(&[
        run("SPEED", 50.0, 700.0, 8.0),
        run("30 ft.", 100.0, 700.0, 8.0),
    ]);
    let label = text.find("SPEED")[0];
    assert_eq!(texts(&text.right_of(label, 80.0)), vec!["30 ft."]);
}

#[test]
fn right_of_stops_at_the_next_column() {
    let text = page(&[
        run("SPEED", 50.0, 700.0, 8.0),
        run("30 ft.", 100.0, 700.0, 8.0),
        // The next column's text, on the same baseline but past max_dx.
        run("Darkvision 60 ft.", 320.0, 700.0, 8.0),
    ]);
    let label = text.find("SPEED")[0];
    assert_eq!(texts(&text.right_of(label, 120.0)), vec!["30 ft."]);
}

#[test]
fn right_of_ignores_a_line_above_or_below_the_band() {
    let text = page(&[
        run("SPEED", 50.0, 700.0, 8.0),
        run("above", 100.0, 720.0, 8.0),
        run("below", 100.0, 680.0, 8.0),
    ]);
    let label = text.find("SPEED")[0];
    assert!(text.right_of(label, 200.0).is_empty());
}

#[test]
fn right_of_does_not_read_text_to_the_left() {
    let text = page(&[
        run("left", 10.0, 700.0, 8.0),
        run("SPEED", 100.0, 700.0, 8.0),
    ]);
    let label = text.find("SPEED")[0];
    assert!(text.right_of(label, 200.0).is_empty());
}

// ---------------------------------------------------------------------------
// below

#[test]
fn below_reads_down_a_box_in_order() {
    let text = page(&[
        run("EQUIPMENT", 50.0, 500.0, 8.0),
        run("Longsword", 50.0, 488.0, 8.0),
        run("Shield", 50.0, 476.0, 8.0),
        // Far below: past max_dy.
        run("Backpack", 50.0, 300.0, 8.0),
    ]);
    let label = text.find("EQUIPMENT")[0];
    assert_eq!(texts(&text.below(label, 40.0)), vec!["Longsword", "Shield"]);
}

#[test]
fn below_keeps_to_the_box_horizontally() {
    let text = page(&[
        run("EQUIPMENT", 50.0, 500.0, 8.0),
        run("Longsword", 55.0, 488.0, 8.0),
        // Another box's text at the same height, well to the right.
        run("Fireball", 300.0, 488.0, 8.0),
    ]);
    let label = text.find("EQUIPMENT")[0];
    assert_eq!(texts(&text.below(label, 40.0)), vec!["Longsword"]);
}

#[test]
fn below_does_not_include_the_label_itself_or_text_above() {
    let text = page(&[
        run("above", 50.0, 520.0, 8.0),
        run("EQUIPMENT", 50.0, 500.0, 8.0),
    ]);
    let label = text.find("EQUIPMENT")[0];
    assert!(text.below(label, 100.0).is_empty());
}

// ---------------------------------------------------------------------------
// within

#[test]
fn within_returns_lines_inside_an_area() {
    let text = page(&[
        run("inside", 100.0, 600.0, 8.0),
        run("outside", 400.0, 600.0, 8.0),
        run("too low", 100.0, 100.0, 8.0),
    ]);
    let area = Rect {
        x0: 90.0,
        y0: 180.0,
        x1: 300.0,
        y1: 210.0,
    };
    assert_eq!(texts(&text.within(area)), vec!["inside"]);
}

#[test]
fn a_rect_knows_its_centre_and_overlap() {
    let a = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 10.0,
        y1: 10.0,
    };
    let b = Rect {
        x0: 5.0,
        y0: 20.0,
        x1: 15.0,
        y1: 30.0,
    };
    assert!(a.overlaps_horizontally(&b));
    assert_eq!(a.centre_y(), 5.0);
    assert!(a.contains_point(5.0, 5.0));
    assert!(!a.contains_point(5.0, 15.0));
}

// ---------------------------------------------------------------------------
// Reading a real document

fn document_with(rotate: Option<i64>, text: &str) -> Vec<u8> {
    crate::test_pdf::document(crate::test_pdf::Spec {
        pages: &[text],
        rotate,
        ..Default::default()
    })
}

#[test]
fn read_finds_a_label_on_a_real_page() {
    let doc = crate::Document::from_bytes(&document_with(None, "CHARACTER NAME")).unwrap();
    let text = PageText::read(&doc, 1).unwrap();
    assert_eq!(text.number, 1);
    assert_eq!(text.find("CHARACTER NAME").len(), 1);
}

#[test]
fn a_rotated_page_is_refused_by_number() {
    let doc = crate::Document::from_bytes(&document_with(Some(90), "CHARACTER NAME")).unwrap();
    match PageText::read(&doc, 1) {
        Err(crate::PdfError::Page { page, reason }) => {
            assert_eq!(page, 1);
            assert!(reason.contains("rotated"), "{reason}");
        }
        Err(other) => panic!("expected a page refusal, got {other}"),
        Ok(_) => panic!("a rotated page must be refused, not read sideways"),
    }
}

#[test]
fn a_page_past_the_end_is_refused_by_number() {
    let doc = crate::Document::from_bytes(&document_with(None, "x")).unwrap();
    assert!(matches!(
        PageText::read(&doc, 2),
        Err(crate::PdfError::Page { page: 2, .. })
    ));
}
