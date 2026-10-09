//! Spec 048 T009: what a document may be before anyone reads it.

use crate::test_pdf::{Spec, document};
use crate::{Document, Limits, PdfError};

const SMALL: Limits = Limits {
    max_bytes: 1024 * 1024,
    max_pages: 3,
};

#[test]
fn an_ordinary_file_within_bounds_opens() {
    let bytes = document(Spec {
        pages: &["CHARACTER NAME", "FEATURES & TRAITS"],
        ..Default::default()
    });
    let doc = Document::from_bytes_bounded(&bytes, SMALL).expect("opens");
    assert_eq!(doc.page_count(), 2);
}

#[test]
fn a_file_over_the_byte_limit_is_refused_before_it_is_parsed() {
    // Not a PDF at all: if this were parsed it would be Unreadable.
    let bytes = vec![b'x'; 2048];
    let limits = Limits {
        max_bytes: 1024,
        max_pages: 3,
    };
    match Document::from_bytes_bounded(&bytes, limits) {
        Err(PdfError::TooLarge { bytes, limit }) => {
            assert_eq!(bytes, 2048);
            assert_eq!(limit, 1024);
        }
        Err(other) => panic!("expected TooLarge, got {other}"),
        Ok(_) => panic!("expected TooLarge"),
    }
}

#[test]
fn a_file_at_exactly_the_byte_limit_is_allowed() {
    let bytes = document(Spec {
        pages: &["x"],
        ..Default::default()
    });
    let limits = Limits {
        max_bytes: bytes.len(),
        max_pages: 3,
    };
    assert!(Document::from_bytes_bounded(&bytes, limits).is_ok());
}

#[test]
fn a_document_with_too_many_pages_is_refused() {
    let bytes = document(Spec {
        pages: &["1", "2", "3", "4"],
        ..Default::default()
    });
    match Document::from_bytes_bounded(&bytes, SMALL) {
        Err(PdfError::TooManyPages { pages, limit }) => {
            assert_eq!(pages, 4);
            assert_eq!(limit, 3);
        }
        Err(other) => panic!("expected TooManyPages, got {other}"),
        Ok(_) => panic!("expected TooManyPages"),
    }
}

#[test]
fn an_encrypted_document_is_refused() {
    let bytes = document(Spec {
        pages: &["CHARACTER NAME"],
        encrypted: true,
        ..Default::default()
    });
    assert!(matches!(
        Document::from_bytes_bounded(&bytes, SMALL),
        Err(PdfError::Encrypted)
    ));
}

#[test]
fn the_default_limits_are_ten_megabytes_and_twenty_pages() {
    let limits = Limits::default();
    assert_eq!(limits.max_bytes, 10 * 1024 * 1024);
    assert_eq!(limits.max_pages, 20);
}

#[test]
fn each_refusal_says_what_was_wrong() {
    assert!(PdfError::Encrypted.to_string().contains("password"));
    assert!(
        PdfError::TooLarge {
            bytes: 2048,
            limit: 1024
        }
        .to_string()
        .contains("1024")
    );
    assert!(
        PdfError::TooManyPages {
            pages: 40,
            limit: 20
        }
        .to_string()
        .contains("40")
    );
}
