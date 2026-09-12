use super::*;

/// The map from the bestiary that prompted this module, trimmed to the parts
/// that matter. One-byte codes, which is the case `lopdf` cannot read.
const TYPE3: &str = r"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<00> <FF>
endcodespacerange
3 beginbfchar
<03> <0020>
<22> <003F>
<70> <2019>
endbfchar
1 beginbfrange
<0E> <1E> <002B>
endbfrange
endcmap
";

/// A two-byte `Identity-H` map, which is the common case and must keep
/// working.
const IDENTITY_H: &str = r"
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
2 beginbfchar
<0003> <0020>
<0026> <0043>
endbfchar
";

#[test]
fn a_one_byte_map_is_read_at_one_byte_a_code() {
    // The whole reason this module exists: `lopdf` decodes in two-byte
    // chunks, so every code in a Type 3 font is read as half of something
    // else and the text comes out shifted.
    let map = parse(TYPE3.as_bytes()).expect("parses");
    assert_eq!(map.decode(&[0x03]), " ");
    assert_eq!(map.decode(&[0x22]), "?");
}

#[test]
fn the_range_that_produced_the_twenty_nine_character_shift_decodes() {
    // `<0E> <1E> <002B>`: code 0x0E is U+002B, and the offset of 29 is
    // exactly what the garbled text showed. Code 0x47 — 'G' if the bytes are
    // read as Latin-1 — is really 'd'.
    let map = parse(TYPE3.as_bytes()).expect("parses");
    assert_eq!(map.decode(&[0x0E]), "+");
    assert_eq!(map.decode(&[0x0F]), ",");
    assert_eq!(map.decode(&[0x1E]), ";");
}

#[test]
fn a_two_byte_map_still_reads_two_bytes_at_a_time() {
    let map = parse(IDENTITY_H.as_bytes()).expect("parses");
    assert_eq!(map.decode(&[0x00, 0x03]), " ");
    assert_eq!(map.decode(&[0x00, 0x26]), "C");
    assert_eq!(map.decode(&[0x00, 0x03, 0x00, 0x26]), " C");
}

#[test]
fn a_map_with_no_codespace_range_is_assumed_two_byte() {
    // What a CMap that omits the range almost always is.
    let map = parse(b"1 beginbfchar\n<0041> <0061>\nendbfchar").expect("parses");
    assert_eq!(map.decode(&[0x00, 0x41]), "a");
}

#[test]
fn an_unmapped_code_contributes_nothing_rather_than_a_question_mark() {
    // The map is the font's own statement of what it can say. An unmapped
    // code is usually a glyph with no text meaning — a rule, an ornament —
    // and a replacement character would put noise into a monster's name.
    let map = parse(TYPE3.as_bytes()).expect("parses");
    assert_eq!(map.decode(&[0xFE]), "");
    assert_eq!(map.decode(&[0x03, 0xFE, 0x22]), " ?");
}

#[test]
fn a_range_listing_its_destinations_maps_each_one() {
    // The other `bfrange` form, for a range whose characters are unrelated.
    let map = parse(
        b"1 begincodespacerange\n<00> <FF>\nendcodespacerange\n\
          1 beginbfrange\n<10> <12> [<0041> <005A> <0061>]\nendbfrange",
    )
    .expect("parses");
    assert_eq!(map.decode(&[0x10]), "A");
    assert_eq!(map.decode(&[0x11]), "Z");
    assert_eq!(map.decode(&[0x12]), "a");
}

#[test]
fn a_destination_of_several_characters_survives() {
    // A ligature: one code, two letters.
    let map = parse(
        b"1 begincodespacerange\n<00> <FF>\nendcodespacerange\n\
          1 beginbfchar\n<01> <00660069>\nendbfchar",
    )
    .expect("parses");
    assert_eq!(map.decode(&[0x01]), "fi");
}

#[test]
fn a_trailing_byte_that_cannot_form_a_code_is_dropped() {
    // An odd-length string in a two-byte font is malformed. Dropping the
    // stray byte is better than reading it as half a code and inventing a
    // character.
    let map = parse(IDENTITY_H.as_bytes()).expect("parses");
    assert_eq!(map.decode(&[0x00, 0x03, 0x00]), " ");
}

#[test]
fn a_map_with_nothing_in_it_is_refused() {
    // So a caller falls back to reading the bytes, rather than decoding
    // everything to the empty string and reporting a page with no text.
    assert!(parse(b"begincmap\nendcmap").is_none());
    assert!(parse(b"").is_none());
}

#[test]
fn a_backwards_or_enormous_range_is_refused_rather_than_spun_on() {
    // Malformed input must not allocate a map of the whole code space or
    // loop. Both of these appear in files that have been through a bad
    // rewriter.
    let backwards = parse(
        b"1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
          1 beginbfrange\n<0050> <0010> <0041>\nendbfrange",
    );
    assert!(
        backwards.is_none(),
        "a range that runs backwards maps nothing"
    );

    let enormous = parse(
        b"1 begincodespacerange\n<0000> <FFFFFF>\nendcodespacerange\n\
          1 beginbfrange\n<000000> <FFFFFF> <0041>\nendbfrange",
    );
    assert!(
        enormous.is_none(),
        "a range spanning everything maps nothing"
    );
}
