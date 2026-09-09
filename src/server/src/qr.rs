//! A QR code as a grid of booleans, and nothing else.
//!
//! Spec 041 FR-002 asks enrolment to present "both a scannable code and the
//! secret in a typeable form". The typeable half shipped; the scannable half
//! did not, and `apps/web/src/types/twoFactor.ts` has been saying so in its
//! own header — the client type for the matrix exists, the drawing code
//! exists, and the server never sent one.
//!
//! # Why a matrix rather than an image or markup
//!
//! research.md § R10. A server-rendered SVG *string* could only reach the page
//! through `dangerouslySetInnerHTML`, and a security screen is the last place
//! in the product to introduce that. A grid of `0`/`1` cannot carry a script
//! tag, costs the web bundle nothing, and is directly assertable in a test:
//! this exact URI encodes to this exact matrix.
//!
//! # Why the server
//!
//! It already holds the secret and already builds the `otpauth://` URI, so
//! nothing new crosses a boundary. The alternatives were a JavaScript QR
//! library — a dependency and a second implementation of a string the server
//! owns — or an image service, which would put a TOTP secret in a URL sent to
//! a third party and would break the air-gapped first-run instance FR-001b is
//! written for.
//!
//! # Why a failure here is not a failure to enrol
//!
//! `contracts/enrolment.md` rule 6. Every function here returns an `Option`,
//! and the caller sends `None`: a person with the typeable key beside the code
//! can still finish, and refusing to enrol somebody because a QR encoder had
//! an opinion about their username would be absurd.

use serde::Serialize;

/// A QR code as `size` rows of `size` characters, each `0` (light) or `1`
/// (dark).
///
/// Matches `TwoFactorQrMatrix` in `apps/web/src/types/twoFactor.ts`, which
/// draws it as inline SVG rectangles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct QrMatrix {
    pub(crate) size: usize,
    pub(crate) modules: Vec<String>,
}

/// Encode `text` as a QR matrix, or `None` if it cannot be encoded.
///
/// The only realistic failure is a string too long for the largest version,
/// which an `otpauth://` URI is not; it is an `Option` anyway because the
/// caller must not be able to turn a QR problem into an enrolment problem.
pub(crate) fn encode(text: &str) -> Option<QrMatrix> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let colors = code.to_colors();
    let width = code.width();
    if width == 0 || colors.len() != width * width {
        return None;
    }

    let modules = colors
        .chunks(width)
        .map(|row| {
            row.iter()
                .map(|module| {
                    if *module == qrcode::Color::Dark {
                        '1'
                    } else {
                        '0'
                    }
                })
                .collect::<String>()
        })
        .collect();

    Some(QrMatrix {
        size: width,
        modules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str =
        "otpauth://totp/ThunderForge:wizard?secret=JBSWY3DPEHPK3PXP&issuer=ThunderForge";

    #[test]
    fn the_matrix_is_square_and_every_row_is_the_declared_width() {
        let matrix = encode(URI).expect("an otpauth URI encodes");

        assert_eq!(matrix.modules.len(), matrix.size);
        for (index, row) in matrix.modules.iter().enumerate() {
            assert_eq!(
                row.chars().count(),
                matrix.size,
                "row {index} is not `size` modules wide",
            );
        }
    }

    /// The client draws whatever is in these strings. Anything but `0` and `1`
    /// would be a character reaching a security screen from an encoder.
    #[test]
    fn nothing_but_zero_and_one_comes_out() {
        let matrix = encode(URI).expect("encoded");
        for row in &matrix.modules {
            assert!(
                row.chars().all(|c| c == '0' || c == '1'),
                "a row carried something other than 0/1: {row:?}",
            );
        }
    }

    /// A QR code's three finder patterns are 7×7 dark squares in the top-left,
    /// top-right and bottom-left corners. Asserting the top-left one is the
    /// cheapest check that this is a QR code rather than a plausible-looking
    /// grid — a transposed or inverted matrix fails it.
    #[test]
    fn the_finder_pattern_is_where_a_scanner_will_look_for_it() {
        let matrix = encode(URI).expect("encoded");
        let row = |y: usize| matrix.modules[y].as_bytes();

        // The outer ring of the top-left finder: dark, then a light ring.
        assert_eq!(&row(0)[0..7], b"1111111", "the finder's top edge");
        assert_eq!(&row(6)[0..7], b"1111111", "the finder's bottom edge");
        assert_eq!(row(0)[7], b'0', "the separator right of the finder");
        assert_eq!(row(7)[0], b'0', "the separator below the finder");
        // Its 3×3 dark centre.
        assert_eq!(&row(3)[2..5], b"111", "the finder's centre");
    }

    /// Two different secrets must not produce the same code, which is the one
    /// way this could be catastrophically wrong while passing everything
    /// above: a constant matrix is square, is all 0/1, and has finder
    /// patterns.
    #[test]
    fn different_input_encodes_differently() {
        let one = encode(URI).expect("encoded");
        let other = encode(
            "otpauth://totp/ThunderForge:wizard?secret=KRSXG5CTMVRXEZLU&issuer=ThunderForge",
        )
        .expect("encoded");

        assert_ne!(one.modules, other.modules);
    }

    #[test]
    fn an_empty_string_still_encodes_rather_than_panicking() {
        // Not a case the caller can produce — the URI always has an issuer —
        // but this function must never panic on the way to a security screen.
        assert!(encode("").is_some());
    }
}
