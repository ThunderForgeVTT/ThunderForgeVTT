//! Opening a PDF whose cross-reference table is wrong.
//!
//! # Why this exists
//!
//! Measured against a 246-file library of real tabletop PDFs, **90 of them —
//! 37% — will not open** at all, and the failures are almost entirely two
//! messages: `Invalid file trailer`, and `invalid start value in Prev field`.
//! Both mean the same thing. The file's index of where its objects live is
//! wrong, and a strict reader gives up.
//!
//! Nothing is wrong with the objects themselves. Every real-world reader
//! recovers from this by ignoring the index and finding the objects directly,
//! which is what this does: scan the bytes for object headers, write a fresh
//! cross-reference table from what was actually found, and hand the result
//! back to the ordinary parser.
//!
//! # Why rewriting the bytes, rather than the index in memory
//!
//! Because `lopdf`'s object parser is not reachable from outside the crate —
//! it needs a `Reader` that is private. Appending a corrected table to a copy
//! of the bytes uses the parser that already exists instead of duplicating
//! it, and a duplicate PDF object parser is a great deal of surface to own
//! for this.
//!
//! The original file is never touched. The repair happens on a copy in
//! memory.

/// Rebuild a cross-reference table for a document whose own is unusable.
///
/// Returns the repaired bytes, or `None` when the file yields no objects at
/// all — which means it is not a PDF, or is one this cannot help with.
pub fn rebuild_xref(bytes: &[u8]) -> Option<Vec<u8>> {
    let objects = find_objects(bytes);
    if objects.is_empty() {
        return None;
    }
    let root = find_catalog(bytes, &objects)?;
    let max_id = objects.iter().map(|(id, _, _)| *id).max()?;

    let mut out = Vec::with_capacity(bytes.len() + objects.len() * 20 + 256);
    out.extend_from_slice(bytes);
    // A table must start on its own line.
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let xref_offset = out.len();

    // One contiguous section from 0, which is what every simple writer emits
    // and what every reader accepts. Ids nothing was found for are marked
    // free rather than omitted: a sparse table would need one subsection per
    // gap, and a free entry costs twenty bytes.
    out.extend_from_slice(b"xref\n");
    out.extend_from_slice(format!("0 {}\n", max_id + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");

    for id in 1..=max_id {
        match objects
            .iter()
            .rev()
            .find(|(object_id, _, _)| *object_id == id)
        {
            // `rev`: a file that was incrementally updated contains the same
            // id more than once, and the last one written is the live one.
            Some((_, generation, offset)) => {
                out.extend_from_slice(format!("{offset:010} {generation:05} n \n").as_bytes())
            }
            None => out.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }

    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{}\n%%EOF\n",
            max_id + 1,
            root,
            xref_offset
        )
        .as_bytes(),
    );
    Some(out)
}

/// Every `N G obj` header in the file, as (id, generation, offset).
fn find_objects(bytes: &[u8]) -> Vec<(u32, u16, usize)> {
    let mut out = Vec::new();
    let mut index = 0usize;

    while let Some(found) = find_from(bytes, b"obj", index) {
        index = found + 3;
        // Walk backwards over "N G " to the start of the header, which is
        // where the offset must point.
        let Some((id, generation, start)) = header_before(bytes, found) else {
            continue;
        };
        out.push((id, generation, start));
    }
    out
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

/// Read the `N G ` immediately before an `obj` keyword.
fn header_before(bytes: &[u8], obj_at: usize) -> Option<(u32, u16, usize)> {
    let mut cursor = obj_at;
    let skip_space = |cursor: &mut usize| {
        while *cursor > 0 && bytes[*cursor - 1].is_ascii_whitespace() {
            *cursor -= 1;
        }
    };
    let take_digits = |cursor: &mut usize| -> Option<u64> {
        let end = *cursor;
        while *cursor > 0 && bytes[*cursor - 1].is_ascii_digit() {
            *cursor -= 1;
        }
        if *cursor == end {
            return None;
        }
        std::str::from_utf8(&bytes[*cursor..end]).ok()?.parse().ok()
    };

    skip_space(&mut cursor);
    let generation = take_digits(&mut cursor)?;
    skip_space(&mut cursor);
    let id = take_digits(&mut cursor)?;

    // The header must be at the start of a line, or `obj` was part of a word
    // — `/Subject` and stream data both contain the letters.
    let at_line_start = cursor == 0 || bytes[cursor - 1] == b'\n' || bytes[cursor - 1] == b'\r';
    if !at_line_start {
        return None;
    }
    Some((
        u32::try_from(id).ok()?,
        u16::try_from(generation).ok()?,
        cursor,
    ))
}

/// The object id of the document catalogue.
///
/// Found by looking inside each object for `/Type` … `/Catalog`, because the
/// trailer that would normally name it is the thing that is broken.
fn find_catalog(bytes: &[u8], objects: &[(u32, u16, usize)]) -> Option<u32> {
    for (id, _, offset) in objects {
        // A catalogue dictionary is small and sits at the top of its object.
        let end = (*offset + 2048).min(bytes.len());
        let window = &bytes[*offset..end];
        if contains(window, b"/Catalog") && contains(window, b"/Type") {
            return Some(*id);
        }
    }
    None
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find_from(haystack, needle, 0).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest thing that is recognisably a PDF, with no xref at all.
    fn broken_pdf() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"%PDF-1.4\n");
        bytes.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        bytes.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n");
        bytes
    }

    #[test]
    fn a_document_with_no_table_gets_one() {
        let repaired = rebuild_xref(&broken_pdf()).expect("rebuilt");
        let text = String::from_utf8_lossy(&repaired);
        assert!(text.contains("xref\n0 3\n"), "a table for ids 0..2");
        assert!(text.contains("/Root 1 0 R"), "the catalogue is named");
        assert!(text.contains("startxref"));
    }

    #[test]
    fn the_offsets_point_at_the_object_headers() {
        let original = broken_pdf();
        let repaired = rebuild_xref(&original).expect("rebuilt");
        let text = String::from_utf8_lossy(&repaired);
        // Offset of `1 0 obj` is right after the header line.
        let expected = original
            .windows(7)
            .position(|w| w == b"1 0 obj")
            .expect("the object is there");
        assert!(
            text.contains(&format!("{expected:010} 00000 n")),
            "table should point at {expected}"
        );
    }

    #[test]
    fn the_word_obj_inside_other_text_is_not_an_object() {
        // `/Subject` contains the letters, and so does plenty of stream data.
        // Without the line-start rule this invents objects out of prose.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"%PDF-1.4\n");
        bytes.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Subject (12 obj here) >>\nendobj\n");
        let found = find_objects(&bytes);
        assert_eq!(found.len(), 1, "only the real header: {found:?}");
        assert_eq!(found[0].0, 1);
    }

    #[test]
    fn an_incrementally_updated_file_keeps_the_last_version_of_an_object() {
        // Saving a PDF twice appends; both copies of object 1 are in the file
        // and the later one is live. Taking the first would resurrect the
        // document's previous state.
        let mut bytes = broken_pdf();
        let second_at = bytes.len();
        bytes.extend_from_slice(
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Updated true >>\nendobj\n",
        );
        let repaired = rebuild_xref(&bytes).expect("rebuilt");
        let text = String::from_utf8_lossy(&repaired);
        assert!(
            text.contains(&format!("{second_at:010} 00000 n")),
            "the later copy of object 1 should win"
        );
    }

    #[test]
    fn something_that_is_not_a_pdf_is_refused_rather_than_guessed_at() {
        assert!(rebuild_xref(b"this is not a pdf at all").is_none());
        assert!(rebuild_xref(b"").is_none());
    }

    #[test]
    fn a_file_with_objects_but_no_catalogue_is_refused() {
        // Without a catalogue there is no page tree to reach, so a rebuilt
        // table would open a document with nothing in it — worse than saying
        // it could not be read.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"%PDF-1.4\n");
        bytes.extend_from_slice(b"1 0 obj\n<< /Type /Pages >>\nendobj\n");
        assert!(rebuild_xref(&bytes).is_none());
    }
}
