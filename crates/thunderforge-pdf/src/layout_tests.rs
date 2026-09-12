use super::*;

fn run(text: &str, x: f64, y: f64, size: f64) -> TextRun {
    TextRun {
        // Half an em a character, which is what Latin text averages — these
        // fixtures stand for ordinary prose, and the cases where a real
        // font's own widths differ are what the corpus survey is for.
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

fn line(text: &str, y: f64, x0: f64, x1: f64, size: f64) -> Line {
    Line {
        text: text.into(),
        y,
        x0,
        x1,
        size,
        bold: false,
        italic: false,
    }
}

// ---------------------------------------------------------------------------
// Lines
// ---------------------------------------------------------------------------

#[test]
fn runs_sharing_a_baseline_become_one_line() {
    // Positions as a real page has them: about half an em per character, so
    // "Armor " at 10pt is roughly 30pt wide.
    let lines = lines(&[
        run("Armor", 72.0, 700.0, 10.0),
        run("Class", 102.0, 700.0, 10.0),
        run("15", 132.0, 700.0, 10.0),
    ]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].text, "Armor Class 15");
}

#[test]
fn draw_order_does_not_decide_reading_order() {
    // The case that makes everything else possible: a page may draw its
    // second line first, and frequently does.
    let lines = lines(&[
        run("second", 72.0, 680.0, 10.0),
        run("first", 72.0, 700.0, 10.0),
    ]);
    assert_eq!(
        lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        ["first", "second"]
    );
}

#[test]
fn a_line_takes_the_largest_size_on_it_not_the_average() {
    // A drop cap must not drag a body line up into looking like a heading,
    // and a superscript must not drag a heading down.
    let lines = lines(&[
        run("T", 72.0, 700.0, 24.0),
        run("he dragon stirs", 90.0, 700.0, 10.0),
    ]);
    assert_eq!(lines[0].size, 24.0);
}

#[test]
fn separate_placements_do_not_run_words_together() {
    // A book that draws each word with its own placement would otherwise
    // read as `AdultRedDragon`.
    let lines = lines(&[
        run("Adult", 72.0, 700.0, 10.0),
        run("Red", 100.0, 700.0, 10.0),
        run("Dragon", 122.0, 700.0, 10.0),
    ]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].text, "Adult Red Dragon");
}

#[test]
fn a_line_is_bold_only_when_all_of_it_is() {
    let mixed = lines(&[
        bold("Armor Class", 72.0, 700.0, 10.0),
        run("15", 130.0, 700.0, 10.0),
    ]);
    assert!(!mixed[0].bold, "one bold label does not make a bold line");

    let wholly = lines(&[
        bold("Actions", 72.0, 700.0, 10.0),
        bold("cont.", 110.0, 700.0, 10.0),
    ]);
    assert!(wholly[0].bold);
}

#[test]
fn a_baseline_crossing_a_gutter_is_two_lines_not_one() {
    // The defect this exists for, seen on a real Monster Manual page: every
    // body line in the left column shares a baseline with one in the right,
    // and merging them yields "the true dragons, red dragons Red dragons lair
    // in high mountains" — one column's sentence welded to the other's.
    let lines = lines(&[
        run("red dragons", 60.0, 700.0, 9.0),
        run("Red dragons lair", 320.0, 700.0, 9.0),
    ]);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0].text, "red dragons");
    assert_eq!(lines[1].text, "Red dragons lair");
}

#[test]
fn an_ordinary_word_space_is_not_a_gutter() {
    // The other direction, and the one that matters more: over-splitting
    // would shatter every line into words.
    let lines = lines(&[
        run("Armor", 72.0, 700.0, 10.0),
        run("Class", 105.0, 700.0, 10.0),
    ]);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Armor Class");
}

#[test]
fn a_statblock_label_and_its_value_stay_one_line() {
    // A statblock sets its label bold and its value roman, with an ordinary
    // space between. Splitting there would lose which value belongs to which
    // label, which is the whole content of a statblock.
    let lines = lines(&[
        bold("Hit Points", 72.0, 700.0, 9.0),
        run("256 (19d12 + 133)", 122.0, 700.0, 9.0),
    ]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].text, "Hit Points 256 (19d12 + 133)");
}

#[test]
fn blank_runs_are_dropped_rather_than_becoming_lines() {
    let lines = lines(&[
        run("   ", 72.0, 700.0, 10.0),
        run("real", 72.0, 680.0, 10.0),
    ]);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "real");
}

#[test]
fn text_drawn_twice_to_fake_bold_is_read_once() {
    // A real bestiary emboldens its statblock labels by printing them again a
    // third of a point to the side. Read as written it yields
    // `AArrmmoorr CCllaassss` — every glyph interleaved with its own shadow —
    // and nothing looking for "Armor Class" finds it.
    let lines = lines(&[
        bold("Armor", 72.0, 700.0, 10.0),
        bold("Armor", 72.3, 700.0, 10.0),
        bold("Class", 102.0, 700.0, 10.0),
        bold("Class", 102.3, 700.0, 10.0),
    ]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].text, "Armor Class");
}

#[test]
fn text_printed_three_times_for_a_heavier_weight_still_reads_once() {
    let lines = lines(&[
        bold("Speed", 72.0, 700.0, 10.0),
        bold("Speed", 72.2, 700.0, 10.0),
        bold("Speed", 72.4, 700.0, 10.0),
    ]);
    assert_eq!(lines[0].text, "Speed");
}

#[test]
fn a_word_genuinely_repeated_is_kept_twice() {
    // The other direction: "ha ha" is two words, and a table of repeated
    // values is real data. A word space apart is a third of an em, well past
    // the overprint threshold.
    let lines = lines(&[run("ha", 72.0, 700.0, 10.0), run("ha", 85.0, 700.0, 10.0)]);
    assert_eq!(lines[0].text, "ha ha");
}

// ---------------------------------------------------------------------------
// Letter-spacing repair
// ---------------------------------------------------------------------------

#[test]
fn letter_spaced_damage_is_reported_rather_than_guessed_at() {
    // Straight from the corpus: 19 of the 132 documents with an outline have
    // headings like these.
    //
    // The tempting repair — rejoin runs of single letters — produces
    // "CALENDA RAND TI M E", because the damage does not fall on word
    // boundaries. Saying "do not trust this" is the honest answer; a lexicon
    // that silently corrected a monster's name would be worse than none.
    assert!(looks_letter_spaced("CALENDA R A N D TI M E"));
    assert!(looks_letter_spaced("H I STORY OF WI LDE MOUNT"));
    assert_eq!(
        normalise("CALENDA R A N D TI M E"),
        "CALENDA R A N D TI M E",
        "the text is left exactly as it was"
    );
}

#[test]
fn ordinary_prose_is_not_reported_as_damaged() {
    // English's two real one-letter words are extremely common, and a
    // detector that fired on them would condemn most of every book.
    assert!(!looks_letter_spaced("a dragon I saw in a cave"));
    assert!(!looks_letter_spaced("The dragon makes three attacks"));
    assert!(!looks_letter_spaced("Armor Class 19 natural armor"));
}

#[test]
fn a_fully_fragmented_short_string_is_still_damage() {
    // Four lone letters in a row is not a sentence in any book.
    assert!(looks_letter_spaced("T I M E"));
}

#[test]
fn a_string_too_short_to_judge_is_left_alone() {
    assert!(!looks_letter_spaced("I am"));
}

#[test]
fn a_statblocks_abbreviations_and_numbers_are_not_damage() {
    // The line this must not condemn: a statblock is made of short numbers
    // and short units, and flagging every one of them would make the whole
    // reader useless.
    assert!(!looks_letter_spaced("Speed 40 ft climb 40 ft fly 80 ft"));
    assert!(!looks_letter_spaced(
        "STR 23 DEX 14 CON 21 INT 14 WIS 13 CHA 17"
    ));
}

#[test]
fn ordinary_whitespace_is_collapsed() {
    assert_eq!(normalise("  Adult   Red \n Dragon "), "Adult Red Dragon");
    assert_eq!(normalise("   "), "");
}

#[test]
fn a_font_with_no_usable_encoding_is_caught_rather_than_imported() {
    // Straight from the corpus: "Gold dragons have the most love of fey among
    // all dragonkind", every byte shifted by 29 and the spaces gone with
    // them. It has letters, capitals and punctuation, and a bestiary reading
    // it would import a creature called `* ROGGUDJRQV`.
    assert!(looks_unreadable(
        "* ROGGUDJRQVKDYHWKHP RVWORYHRIIHADP RQJDOOGUDJRQNLQG"
    ));
    // And the line under it, which runs even longer.
    assert!(looks_unreadable(
        "LQVSLWHRIWKHIH\\WHQGHQF\\WRZ DUGVP LVFKLHI3 L[LHVDUHRIWHQ"
    ));
}

#[test]
fn ordinary_text_is_readable_however_long_the_line() {
    assert!(!looks_unreadable(
        "The most covetous of the true dragons, red dragons tirelessly seek to \
         increase their treasure hoards."
    ));
    assert!(!looks_unreadable(
        "Bite. Melee Weapon Attack: +17 to hit, reach 15ft."
    ));
    assert!(!looks_unreadable("Hit Points 546 (28d20 + 252)"));
    assert!(!looks_unreadable(""));
}

#[test]
fn a_long_word_is_not_on_its_own_evidence_of_garbage() {
    // German compounds and chemical names exist, and so do very long proper
    // nouns in fantasy books. Thirty is well past any of them.
    assert!(!looks_unreadable("Antidisestablishmentarianism"));
    assert!(!looks_unreadable("The Sibilant Death, Merrshaulk"));
}

// ---------------------------------------------------------------------------
// Body size and headings
// ---------------------------------------------------------------------------

#[test]
fn the_body_size_is_what_most_of_the_characters_are_set_at() {
    let page = vec![
        line("ADULT RED DRAGON", 700.0, 72.0, 300.0, 28.0),
        line("Gargantuan dragon, chaotic evil", 680.0, 72.0, 300.0, 10.0),
        line("Armor Class 19 (natural armor)", 660.0, 72.0, 300.0, 10.0),
        line("Hit Points 256 (19d12 + 133)", 640.0, 72.0, 300.0, 10.0),
    ];
    assert_eq!(body_size(&page), 10.0);
}

#[test]
fn one_enormous_title_does_not_become_the_body_size() {
    // Weighted by characters rather than lines, so a short huge title cannot
    // outvote the paragraphs under it.
    let mut page = vec![line("T", 700.0, 72.0, 300.0, 72.0)];
    for i in 0..5 {
        page.push(line(
            "a reasonably long line of body text",
            690.0 - i as f64 * 12.0,
            72.0,
            300.0,
            9.0,
        ));
    }
    assert_eq!(body_size(&page), 9.0);
}

#[test]
fn a_bigger_line_is_a_heading_and_a_body_line_is_not() {
    let body = 10.0;
    assert!(is_heading(&line("ACTIONS", 700.0, 72.0, 200.0, 14.0), body));
    assert!(!is_heading(
        &line("The dragon makes three attacks.", 680.0, 72.0, 300.0, 10.0),
        body
    ));
}

#[test]
fn headings_are_relative_to_the_page_not_to_a_fixed_size() {
    // The book that breaks a fixed threshold: 11pt body, 13pt headings. A
    // rule saying "14pt is a heading" finds none of them.
    let body = 11.0;
    assert!(is_heading(&line("Traits", 700.0, 72.0, 200.0, 13.0), body));
    // And the same 13pt line is ordinary in a book set at 13.
    assert!(!is_heading(&line("Traits", 700.0, 72.0, 200.0, 13.0), 13.0));
}

#[test]
fn a_whole_bold_paragraph_is_emphasis_rather_than_many_headings() {
    let long = "The dragon can use its Frightful Presence, then makes three \
                attacks: one with its bite and two with its claws.";
    let mut bold_line = line(long, 700.0, 72.0, 300.0, 10.0);
    bold_line.bold = true;
    assert!(!is_heading(&bold_line, 10.0));

    let mut short = line("Frightful Presence", 700.0, 72.0, 200.0, 10.0);
    short.bold = true;
    assert!(is_heading(&short, 10.0), "a short bold line is a heading");
}

#[test]
fn a_page_with_no_text_has_no_body_size_and_no_headings() {
    assert_eq!(body_size(&[]), 0.0);
    assert!(!is_heading(&line("anything", 0.0, 0.0, 0.0, 12.0), 0.0));
}

// ---------------------------------------------------------------------------
// Columns
// ---------------------------------------------------------------------------

fn letter() -> PageGeometry {
    PageGeometry {
        width: 612.0,
        height: 792.0,
    }
}

#[test]
fn a_two_column_page_is_read_down_each_column() {
    // Interleaved by baseline, which is how they arrive once sorted — and
    // reading them in that order gives alternating nonsense.
    let mut page = Vec::new();
    for i in 0..6 {
        let y = 700.0 - i as f64 * 12.0;
        page.push(line(&format!("left {i}"), y, 60.0, 280.0, 10.0));
        page.push(line(&format!("right {i}"), y, 320.0, 550.0, 10.0));
    }
    let ordered = reading_order(page, letter());
    let texts: Vec<&str> = ordered.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts[0], "left 0");
    assert_eq!(texts[5], "left 5");
    assert_eq!(texts[6], "right 0");
}

#[test]
fn a_title_spanning_both_columns_comes_first() {
    let mut page = vec![line("ADULT RED DRAGON", 720.0, 60.0, 550.0, 24.0)];
    for i in 0..6 {
        let y = 700.0 - i as f64 * 12.0;
        page.push(line(&format!("left {i}"), y, 60.0, 280.0, 10.0));
        page.push(line(&format!("right {i}"), y, 320.0, 550.0, 10.0));
    }
    let ordered = reading_order(page, letter());
    assert_eq!(ordered[0].text, "ADULT RED DRAGON");
}

#[test]
fn a_single_column_page_with_a_wide_figure_is_left_alone() {
    // The failure this guard prevents: treating an incidental right-hand
    // caption as a column and interleaving the whole page.
    let mut page: Vec<Line> = (0..12)
        .map(|i| {
            line(
                &format!("body {i}"),
                700.0 - i as f64 * 12.0,
                60.0,
                280.0,
                10.0,
            )
        })
        .collect();
    page.push(line("caption", 500.0, 400.0, 550.0, 8.0));
    let ordered = reading_order(page.clone(), letter());
    assert_eq!(ordered[0].text, page[0].text, "order is unchanged");
    assert_eq!(ordered.len(), page.len());
}

#[test]
fn a_very_short_page_is_never_split_into_columns() {
    let page = vec![
        line("Title", 700.0, 60.0, 200.0, 20.0),
        line("one", 680.0, 60.0, 200.0, 10.0),
    ];
    assert_eq!(reading_order(page.clone(), letter()).len(), page.len());
}
