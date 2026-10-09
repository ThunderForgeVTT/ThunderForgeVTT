use super::*;
use crate::test_pdf::{Spec, Widget, document};

fn read(spec: Spec<'_>) -> Vec<FormField> {
    let doc = crate::Document::from_bytes(&document(spec)).unwrap();
    let pages = doc.pages();
    pages
        .iter()
        .flat_map(|page| doc.form_fields(page))
        .collect()
}

#[test]
fn a_text_field_gives_its_name_value_and_box() {
    let fields = read(Spec {
        pages: &["STRENGTH"],
        widgets: &[Widget {
            name: "STR",
            value: Some("15"),
            rect: [50, 680, 90, 700],
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(fields.len(), 1);
    let field = &fields[0];
    assert_eq!(field.name, "STR");
    assert_eq!(field.value, "15");
    assert_eq!(field.kind, FieldKind::Text);
    assert_eq!(field.page, 1);
    // From the top-left: 792 - 700 = 92 down to 792 - 680 = 112.
    assert_eq!(
        field.rect,
        Rect {
            x0: 50.0,
            y0: 92.0,
            x1: 90.0,
            y1: 112.0
        }
    );
}

#[test]
fn a_field_with_no_value_reads_as_empty() {
    let fields = read(Spec {
        pages: &["x"],
        widgets: &[Widget {
            name: "Ideals",
            rect: [0, 0, 10, 10],
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(fields[0].value, "");
}

#[test]
fn a_reversed_rect_is_normalised() {
    let fields = read(Spec {
        pages: &["x"],
        widgets: &[Widget {
            name: "AC",
            value: Some("16"),
            rect: [90, 700, 50, 680],
            ..Default::default()
        }],
        ..Default::default()
    });
    assert!(fields[0].rect.x0 < fields[0].rect.x1);
    assert!(fields[0].rect.y0 < fields[0].rect.y1);
}

#[test]
fn a_utf16_value_is_decoded() {
    let fields = read(Spec {
        pages: &["x"],
        widgets: &[Widget {
            name: "CharacterName",
            value: Some("Brynjólfr Æsir"),
            rect: [0, 0, 10, 10],
            utf16: true,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(fields[0].value, "Brynjólfr Æsir");
}

#[test]
fn a_button_value_is_its_state_name() {
    let fields = read(Spec {
        pages: &["x"],
        widgets: &[Widget {
            name: "Check Box 12",
            value: Some("Yes"),
            rect: [0, 0, 10, 10],
            button: true,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(fields[0].kind, FieldKind::Button);
    assert_eq!(fields[0].value, "Yes");
}

#[test]
fn a_widget_inherits_name_type_and_value_from_its_parent() {
    let fields = read(Spec {
        pages: &["x"],
        widgets: &[Widget {
            name: "0",
            parent: Some("Eq Name"),
            value: Some("Rope"),
            rect: [0, 0, 10, 10],
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(fields[0].name, "Eq Name.0");
    assert_eq!(fields[0].value, "Rope");
    assert_eq!(fields[0].kind, FieldKind::Text);
}

#[test]
fn widgets_are_read_per_page_top_first() {
    let fields = read(Spec {
        pages: &["one", "two"],
        widgets: &[
            Widget {
                page: 1,
                name: "Low",
                value: Some("b"),
                rect: [0, 100, 10, 110],
                ..Default::default()
            },
            Widget {
                page: 1,
                name: "High",
                value: Some("a"),
                rect: [0, 600, 10, 610],
                ..Default::default()
            },
            Widget {
                page: 0,
                name: "First",
                value: Some("c"),
                rect: [0, 0, 10, 10],
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let names: Vec<(u32, &str)> = fields.iter().map(|f| (f.page, f.name.as_str())).collect();
    assert_eq!(names, vec![(1, "First"), (2, "High"), (2, "Low")]);
}

#[test]
fn a_page_text_carries_its_fields_and_finds_one_by_name() {
    let doc = crate::Document::from_bytes(&document(Spec {
        pages: &["DEXTERITY"],
        widgets: &[Widget {
            // D&D Beyond really does name one field with a trailing space.
            name: "DEXmod ",
            value: Some("+2"),
            rect: [0, 0, 10, 10],
            ..Default::default()
        }],
        ..Default::default()
    }))
    .unwrap();
    let text = crate::PageText::read(&doc, 1).unwrap();
    assert_eq!(text.fields.len(), 1);
    assert_eq!(text.field("DEXmod").map(|f| f.value.as_str()), Some("+2"));
    assert!(text.field("DEX").is_none());
}

#[test]
fn a_page_with_no_widgets_has_no_fields() {
    let fields = read(Spec {
        pages: &["only drawn text"],
        ..Default::default()
    });
    assert!(fields.is_empty());
}
