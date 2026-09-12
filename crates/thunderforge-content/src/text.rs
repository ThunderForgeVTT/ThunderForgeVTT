//! Judging whether text can be trusted, and pulling a value off a label.
//!
//! These live here rather than in the layout pass because they are judgements
//! about *content*, not about geometry — and because this crate has to sit
//! underneath both the PDF layer and the server without depending on either.
//! `thunderforge_pdf::layout` re-exports them, so the layout pass and the
//! browser agree with the readers by construction rather than by coincidence.

/// The text after a label, if the line starts with it.
///
/// Case- and punctuation-insensitive, because books genuinely disagree:
/// `Armor Class 15`, `ARMOR CLASS 15`, `Armor Class: 15`, and — in a book
/// that emboldens its labels as a run of their own — `Armor Class. 15`, where
/// the full stop is styling rather than a sentence.
///
/// Lifted verbatim in behaviour from the 5e reader, which learned each of
/// those forms from a real book rather than from a specification.
pub fn after_label<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let trimmed = text.trim_start();
    let label_len = label.chars().count();
    let candidate: String = trimmed.chars().take(label_len).collect();
    if !candidate.eq_ignore_ascii_case(label) {
        return None;
    }
    let rest = &trimmed[candidate.len()..];
    let rest = rest.trim_start_matches([':', '.', '—', '-', ' ', '\t']);
    Some(rest.trim())
}

/// Whether a string is made of letters rather than symbols.
///
/// One bestiary embeds a font with no `ToUnicode` map *and* no encoding — its
/// codes are raw glyph indices, meaningful only to the font program beside
/// them — and its pages decode to `. / * D D & ! ! $ * D`. A name read from
/// one arrives as `* ROGGUDJRQV`. Not recoverable without reading the
/// embedded font, and refused outright rather than flagged, because there is
/// nothing there for a person to correct.
pub fn is_mostly_letters(text: &str) -> bool {
    let considered = text.chars().filter(|c| !c.is_whitespace()).count();
    if considered == 0 {
        return false;
    }
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    letters * 2 >= considered
}

/// Whether a line looks letter-spaced past the point of being trustworthy.
///
/// # Why this reports rather than repairs
///
/// A designer who letter-spaces a title leaves the spacing in the text
/// stream, and the words arrive as `CALENDA R A N D TI M E`. Nineteen of the
/// 132 documents in the reference library that have an outline carry headings
/// like it.
///
/// The tempting fix is to rejoin runs of single letters. It does not work.
/// That example rejoins to `CALENDA RAND TI M E` — the damage does not fall
/// on word boundaries, so the boundaries cannot be recovered from the text
/// alone. Recovering `CALENDAR AND TIME` needs a lexicon, and a lexicon that
/// is wrong on a proper noun is worse than no repair: a monster called
/// `Ba'lath` would be silently corrected into something else.
///
/// So this says "do not trust this string" and leaves it intact, which is the
/// same answer spec 048 FR-003 gives for a field that could not be read: a
/// value reported as uncertain is useful, and a value quietly mangled is not.
/// The short words English actually has.
///
/// Used only to decide whether a *fragment* is a word — never to rewrite
/// anything. That is the difference between this and the lexicon the doc
/// comment above rules out: being wrong here costs a mis-flagged line, not a
/// silently altered monster name.
const SHORT_WORDS: &[&str] = &[
    "a", "i", "an", "as", "at", "be", "by", "do", "go", "he", "if", "in", "is", "it", "me", "my",
    "no", "of", "on", "or", "so", "to", "up", "us", "we", "am", "the", "and", "for", "you", "hp",
    "ac", "ft", "dc", "xp", "cr", "lb", "st",
];

pub fn looks_letter_spaced(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    // Too little evidence either way below this.
    if words.len() < 3 {
        return false;
    }

    let mut fragments = 0usize;
    for word in &words {
        let letters: String = word.chars().filter(|c| c.is_alphabetic()).collect();
        // Numbers and punctuation are not fragments — a statblock is full of
        // short numbers and would otherwise read as damaged throughout.
        if letters.is_empty() || letters.chars().count() > 2 {
            continue;
        }
        if !SHORT_WORDS.contains(&letters.to_lowercase().as_str()) {
            fragments += 1;
        }
    }

    // A quarter of the line being one- or two-letter non-words is well past
    // what prose produces. Ordinary English throws up the odd initial or
    // abbreviation; it does not throw up four in a row.
    fragments * 4 >= words.len()
}

/// Whether a line decoded into something that is not language.
///
/// # The failure this catches
///
/// A subsetted font may carry no `/ToUnicode` map and no standard encoding at
/// all — its codes mean something only to the glyph program embedded beside
/// it. Read as bytes, such a font yields text like:
///
/// ```text
/// * ROGGUDJRQVKDYHWKHP RVWORYHRIIH\DP RQJDOOGUDJRQNLQG
/// ```
///
/// which is "Gold dragons have the most love of fey among all dragonkind"
/// with every byte shifted by 29. It is the worst kind of failure, because it
/// *looks* like text: it has letters, capitals and punctuation, and nothing
/// downstream can tell it is wrong. A bestiary read this way would import
/// creatures named `* ROGGUDJRQV`.
///
/// # The signal
///
/// Average word length. The space glyph is shifted along with everything
/// else, so real spaces vanish and what is left runs together. English
/// averages four to five characters a word; the line above averages twelve.
///
/// The mean rather than the longest word, which was the first attempt and
/// does not work: that line's longest run is eighteen characters, well inside
/// what a real sentence can contain. It is the *whole line* being built of
/// such runs that gives it away.
pub fn looks_unreadable(text: &str) -> bool {
    let words: Vec<usize> = text
        .split_whitespace()
        .map(|word| word.chars().count())
        .collect();

    // One very long token is a compound noun or a URL, not evidence about a
    // line. Three is enough to have an average worth trusting.
    if words.len() < 3 {
        return words.first().is_some_and(|only| *only > 40);
    }
    let mean = words.iter().sum::<usize>() as f64 / words.len() as f64;
    mean > 10.0
}

/// Whether text is too damaged to present as read.
///
/// The same judgement [`looks_letter_spaced`] makes, named for what a caller
/// is actually asking.
pub fn looks_damaged(text: &str) -> bool {
    looks_letter_spaced(text)
}
