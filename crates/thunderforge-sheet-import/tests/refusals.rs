//! Spec 048 T047/T048: every way a sheet is refused has its code and the
//! sentence a player reads, the same in the browser and on the server
//! (contracts/graphql-sheet-import.md, "Common errors").

use thunderforge_pdf::PdfError;
use thunderforge_sheet_import::ReadError;

fn refusal(error: ReadError) -> (&'static str, String) {
    (error.code(), error.sentence())
}

#[test]
fn each_refusal_has_its_code() {
    let cases = [
        (ReadError::Pdf(PdfError::Encrypted), "SHEET_ENCRYPTED"),
        (
            ReadError::Pdf(PdfError::TooLarge {
                bytes: 11 * 1024 * 1024,
                limit: 10 * 1024 * 1024,
            }),
            "SHEET_TOO_LARGE",
        ),
        (
            ReadError::Pdf(PdfError::TooManyPages {
                pages: 21,
                limit: 20,
            }),
            "SHEET_TOO_MANY_PAGES",
        ),
        (
            ReadError::Pdf(PdfError::Unreadable("no header".into())),
            "SHEET_UNREADABLE",
        ),
        (
            ReadError::Pdf(PdfError::Page {
                page: 2,
                reason: "bad stream".into(),
            }),
            "SHEET_UNREADABLE",
        ),
        (
            ReadError::Page {
                page: 1,
                reason: "no ability scores".into(),
            },
            "SHEET_UNREADABLE",
        ),
        (
            ReadError::NotRecognised("This is a book.".into()),
            "SHEET_NOT_RECOGNISED",
        ),
    ];
    for (error, code) in cases {
        assert_eq!(error.code(), code, "{error}");
    }
}

#[test]
fn each_refusal_is_a_sentence_a_player_can_act_on() {
    let (_, encrypted) = refusal(ReadError::Pdf(PdfError::Encrypted));
    assert!(encrypted.contains("password"), "{encrypted}");

    let (_, large) = refusal(ReadError::Pdf(PdfError::TooLarge {
        bytes: 11 * 1024 * 1024,
        limit: 10 * 1024 * 1024,
    }));
    assert!(large.contains("10 MB"), "{large}");

    let (_, pages) = refusal(ReadError::Pdf(PdfError::TooManyPages {
        pages: 21,
        limit: 20,
    }));
    assert!(pages.contains("21") && pages.contains("20"), "{pages}");

    let (_, page) = refusal(ReadError::Page {
        page: 3,
        reason: "no ability scores".into(),
    });
    assert!(page.contains("Page 3"), "{page}");

    let (_, other) = refusal(ReadError::NotRecognised("This is a book.".into()));
    assert_eq!(other, "This is a book.");

    for sentence in [encrypted, large, pages, page, other] {
        let first = sentence.chars().next().unwrap();
        assert!(first.is_uppercase(), "{sentence}");
        assert!(sentence.ends_with('.'), "{sentence}");
    }
}
