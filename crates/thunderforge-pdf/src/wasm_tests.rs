use super::*;
use crate::test_pdf::{Spec, document};

#[test]
fn read_pdf_output_is_pinned() {
    // Spec 048 added a function beside this one; this one must not change.
    let bytes = document(Spec {
        pages: &["CHARACTER NAME", "Second page"],
        ..Default::default()
    });
    assert_eq!(read_pdf_json(&bytes, 1, 10).unwrap(), PINNED);
}

const PINNED: &str = r#"{"pages":2,"lines":[{"text":"CHARACTER NAME","page":1,"size":10.0,"bold":false,"italic":false,"heading":false,"suspect":false},{"text":"Second page","page":2,"size":10.0,"bold":false,"italic":false,"heading":false,"suspect":false}],"silent_pages":0,"repaired":false}"#;

#[test]
fn read_page_text_places_each_line() {
    let bytes = document(Spec {
        pages: &["CHARACTER NAME"],
        ..Default::default()
    });
    let json: serde_json::Value =
        serde_json::from_str(&page_text_json(&bytes, 1).unwrap()).unwrap();
    assert_eq!(json["number"], 1);
    assert_eq!(json["lines"][0]["text"], "CHARACTER NAME");
    let rect = &json["lines"][0]["rect"];
    assert_eq!(rect["x0"], 72.0);
    assert!(rect["y0"].as_f64().unwrap() < rect["y1"].as_f64().unwrap());
}

#[test]
fn read_page_text_is_bounded_like_the_server() {
    let bytes = document(Spec {
        pages: &["x"],
        encrypted: true,
        ..Default::default()
    });
    assert_eq!(
        page_text_json(&bytes, 1).unwrap_err(),
        "the document is protected by a password"
    );
}

#[test]
fn read_page_text_refuses_a_page_past_the_end() {
    let bytes = document(Spec {
        pages: &["x"],
        ..Default::default()
    });
    assert!(page_text_json(&bytes, 3).is_err());
}
