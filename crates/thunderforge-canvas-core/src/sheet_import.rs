//! The slot a pack fills to take character sheets (spec 048).
//!
//! Canvas core never parses a PDF. A pack's reader is handed to the host as a
//! [`SheetReaderHandle`]: bytes in, a reading's JSON out. That keeps a PDF
//! parser out of every pack that never imports a sheet, and keeps this crate
//! free of `thunderforge-pdf`.

/// Why a reader gave no reading. `code` is one of the host's error codes
/// (`SHEET_NOT_RECOGNISED`, `SHEET_UNREADABLE`, `SHEET_ENCRYPTED`, ...);
/// `message` is shown to the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetReadFailure {
    pub code: &'static str,
    pub message: String,
}

/// A pack's reader, as the host runs it.
pub trait SheetReaderHandle: Sync {
    /// Stable, such as "ddb-pdf". Recorded with every import.
    fn id(&self) -> &'static str;
    /// Changes whenever the reading of the same bytes could change.
    fn version(&self) -> &'static str;
    /// Read a document into an `ImportedCharacter`, as JSON.
    fn read(&self, bytes: &[u8]) -> Result<String, SheetReadFailure>;
}

/// A pack's last word on a plan, over JSON: the reading, and the plan to
/// adjust. For what the declaration cannot say: a level that is a sum, an
/// attack that is an item or an ability.
pub type SheetRefineFn = fn(&serde_json::Value, &mut serde_json::Value);

/// Everything a pack contributes to bringing a character in.
pub struct SheetImport {
    /// The readers the server runs, tried in order.
    pub readers: &'static [&'static dyn SheetReaderHandle],
    pub refine: Option<SheetRefineFn>,
}

impl SheetImport {
    /// The reader with this id, if the pack has it.
    pub fn reader(&self, id: &str) -> Option<&'static dyn SheetReaderHandle> {
        self.readers
            .iter()
            .copied()
            .find(|reader| reader.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;

    impl SheetReaderHandle for Fixed {
        fn id(&self) -> &'static str {
            "fixed"
        }
        fn version(&self) -> &'static str {
            "1"
        }
        fn read(&self, bytes: &[u8]) -> Result<String, SheetReadFailure> {
            if bytes.is_empty() {
                return Err(SheetReadFailure {
                    code: "SHEET_UNREADABLE",
                    message: "empty".into(),
                });
            }
            Ok("{}".into())
        }
    }

    static FIXED: Fixed = Fixed;
    static IMPORT: SheetImport = SheetImport {
        readers: &[&FIXED],
        refine: None,
    };

    #[test]
    fn a_reader_is_found_by_id_and_runs_on_bytes() {
        let reader = IMPORT.reader("fixed").expect("declared");
        assert_eq!(reader.version(), "1");
        assert_eq!(reader.read(b"%PDF").unwrap(), "{}");
        assert_eq!(reader.read(b"").unwrap_err().code, "SHEET_UNREADABLE");
        assert!(IMPORT.reader("other").is_none());
    }
}
