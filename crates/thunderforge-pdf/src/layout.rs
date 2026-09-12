//! Turning placed runs into something with a reading order.
//!
//! Three inferences, in order, each of which a PDF does not record and a
//! reader has to make:
//!
//! 1. **Lines.** Runs sharing a baseline are one line.
//! 2. **Columns.** A page is cut into vertical bands where lines cluster, so
//!    a two-column spread is read down each column rather than across.
//! 3. **Headings.** A line noticeably bigger or bolder than the body around
//!    it starts a section.
//!
//! Everything here is measured against the page it is on. A fixed "14pt is a
//! heading" rule fails on the first book that sets its body at 11pt and its
//! headings at 13, and this corpus has several.

use crate::PageGeometry;
use crate::text::TextRun;

/// One line of text, assembled from the runs that share its baseline.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Line {
    pub text: String,
    /// The baseline, in page coordinates.
    pub y: f64,
    /// Where the line starts and ends horizontally.
    pub x0: f64,
    pub x1: f64,
    /// The largest size any run on this line is drawn at — what decides
    /// whether it reads as a heading. The largest rather than the average,
    /// because a drop cap or a superscript must not drag a body line up or a
    /// heading down.
    pub size: f64,
    /// Whether every run with words on this line is bold. A heading is
    /// usually wholly bold; a body line with one bold word is not.
    pub bold: bool,
    pub italic: bool,
}

/// How close two baselines must be to count as the same line, as a fraction
/// of the text size.
///
/// Generous, because a line that mixes sizes — a statblock's bold label and
/// its roman value — has runs whose baselines differ slightly by design.
const SAME_LINE: f64 = 0.4;

/// Assemble runs into lines.
///
/// Runs arrive in the order the page *drew* them, which is frequently not the
/// order anybody reads them in. Sorting by baseline first is what makes the
/// rest of this possible.
pub fn lines(runs: &[TextRun]) -> Vec<Line> {
    let mut ordered: Vec<&TextRun> = runs.iter().filter(|run| !run.is_blank()).collect();
    if ordered.is_empty() {
        return Vec::new();
    }
    // Down the page, then across: y descending because PDF's origin is at the
    // bottom.
    ordered.sort_by(|a, b| {
        b.y.partial_cmp(&a.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut out: Vec<Vec<&TextRun>> = Vec::new();
    for run in ordered {
        let tolerance = (run.size * SAME_LINE).max(1.0);
        match out.last_mut() {
            Some(current)
                if current
                    .first()
                    .is_some_and(|first| (first.y - run.y).abs() <= tolerance) =>
            {
                current.push(run)
            }
            _ => out.push(vec![run]),
        }
    }

    out.into_iter().flat_map(split_at_gutters).collect()
}

/// How wide a gap must be, in ems, before it is a gutter rather than a space.
///
/// A word space is about a third of an em and a wide one half. A column
/// gutter is one to two. At 1.4 the two do not overlap, and the failure this
/// prevents is not subtle: without it a two-column page yields lines like
/// "the true dragons, red dragons Red dragons lair in high mountains", the
/// left column's sentence welded to the right column's.
const GUTTER: f64 = 1.4;

/// Split one baseline's runs wherever a gutter separates them.
///
/// Runs sharing a baseline are only one line if nothing but spaces lies
/// between them. On a two-column spread every body line in the left column
/// shares its baseline with one in the right, and they are two lines.
fn split_at_gutters(mut group: Vec<&TextRun>) -> Vec<Line> {
    group.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = Vec::new();
    let mut current: Vec<&TextRun> = Vec::new();
    let mut previous_end: Option<f64> = None;

    for run in group {
        if let Some(end) = previous_end
            && run.x - end > run.size * GUTTER
            && !current.is_empty()
        {
            out.push(assemble(std::mem::take(&mut current)));
        }
        previous_end = Some(estimated_end(run));
        current.push(run);
    }
    if !current.is_empty() {
        out.push(assemble(current));
    }
    out
}

/// Where a run's text stops.
///
/// Measured from the font's own advances (`TextRun::width`). This was an
/// estimate of half an em a character until a book whose type is narrower
/// than that yielded "Arm orC lass" — close enough for gutters, not close
/// enough for spaces.
fn estimated_end(run: &TextRun) -> f64 {
    run.x + run.width
}

/// How close two identical runs must be to be one run drawn twice, as a
/// fraction of the text size.
///
/// Faux bold is drawn by printing the text a second time a fraction of a
/// point to the side. A real repetition — "ha ha", a table of "1 1 1" — is a
/// word space apart at the very least, which is a third of an em.
const OVERPRINT: f64 = 0.12;

fn assemble(group: Vec<&TextRun>) -> Line {
    let group = without_overprints(group);
    let size = group.iter().map(|r| r.size).fold(0.0f64, f64::max);
    let x0 = group.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
    let x1 = group.iter().map(|r| r.x).fold(f64::NEG_INFINITY, f64::max);
    let y = group.first().map(|r| r.y).unwrap_or_default();

    // A gap wider than a space means the runs are separated words rather than
    // one word split across two placements. Without this, a book that draws
    // each word with its own `Tj` reads as `AdultRedDragon`.
    let mut text = String::new();
    let mut previous_end: Option<f64> = None;
    for run in &group {
        if let Some(previous) = previous_end
            && run.x - previous > run.size * 0.18
            && !text.ends_with(' ')
            && !run.text.starts_with(' ')
        {
            text.push(' ');
        }
        text.push_str(&run.text);
        previous_end = Some(estimated_end(run));
    }

    Line {
        text: normalise(&text),
        y,
        x0,
        x1,
        size,
        bold: group.iter().filter(|r| !r.is_blank()).all(|r| r.bold),
        italic: group.iter().filter(|r| !r.is_blank()).all(|r| r.italic),
    }
}

/// Drop the second copy of text that was drawn twice to fake a bold weight.
///
/// Without this, a book that emboldens its statblock labels this way yields
/// `AArrmmoorr CCllaassss..` — every glyph interleaved with its own shadow —
/// and no reader looking for "Armor Class" finds anything in the whole book.
///
/// Compares against the last *kept* run rather than the last seen, so text
/// printed three times (which exists, for a heavier fake weight) collapses to
/// one rather than two.
fn without_overprints(mut group: Vec<&TextRun>) -> Vec<&TextRun> {
    group.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept: Vec<&TextRun> = Vec::with_capacity(group.len());
    for run in group {
        let overprint = kept.last().is_some_and(|previous: &&TextRun| {
            previous.text == run.text && (run.x - previous.x).abs() <= run.size * OVERPRINT
        });
        if !overprint {
            kept.push(run);
        }
    }
    kept
}

/// Collapse runs of whitespace. Nothing else.
///
/// See [`looks_letter_spaced`] for the damage this deliberately does **not**
/// try to repair.
pub fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// A page's text in reading order, one line per entry.
///
/// Columns are found by where lines actually start, not by assuming two. A
/// bestiary page is frequently two columns; its title is frequently one wide
/// line across both, and that line belongs before either column.
pub fn reading_order(lines: Vec<Line>, geometry: PageGeometry) -> Vec<Line> {
    if lines.len() < 4 {
        return lines;
    }

    let midpoint = geometry.width / 2.0;
    // A line that starts left of centre and ends right of it spans the page.
    let spans = |line: &Line| line.x0 < midpoint && line.x1 > midpoint;

    let left: Vec<Line> = lines
        .iter()
        .filter(|l| !spans(l) && l.x0 < midpoint)
        .cloned()
        .collect();
    let right: Vec<Line> = lines
        .iter()
        .filter(|l| !spans(l) && l.x0 >= midpoint)
        .cloned()
        .collect();

    // Only treat it as two columns when both sides carry real weight.
    // Otherwise it is a single column that happens to have a wide figure, and
    // splitting it would interleave nonsense.
    let smaller = left.len().min(right.len());
    if smaller * 4 < lines.len() {
        return lines;
    }

    let mut out: Vec<Line> = lines.iter().filter(|l| spans(l)).cloned().collect();
    out.extend(left);
    out.extend(right);
    out
}

/// How much bigger than the body a line must be to read as a heading.
const HEADING_RATIO: f64 = 1.15;

/// The size most of this page's text is set at.
///
/// The median by *characters*, not by lines: a page with one enormous title
/// and forty lines of body has a body size, and counting lines would let a
/// handful of oversized captions move it.
pub fn body_size(lines: &[Line]) -> f64 {
    let mut weighted: Vec<(f64, usize)> = lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .map(|line| (line.size, line.text.chars().count()))
        .collect();
    if weighted.is_empty() {
        return 0.0;
    }
    weighted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let total: usize = weighted.iter().map(|(_, n)| n).sum();
    let mut seen = 0usize;
    for (size, count) in weighted {
        seen += count;
        if seen * 2 >= total {
            return size;
        }
    }
    0.0
}

/// Whether a line reads as a heading against the page it is on.
///
/// Bigger than the body, or bold and set apart. Measured per page because a
/// fixed threshold fails on the first book that sets 11pt body and 13pt
/// headings — and this corpus has several.
pub fn is_heading(line: &Line, body: f64) -> bool {
    if line.text.trim().is_empty() || body <= 0.0 {
        return false;
    }
    if line.size >= body * HEADING_RATIO {
        return true;
    }
    // Bold at body size counts only when the line is short. A whole bold
    // paragraph is emphasis, not forty headings in a row.
    line.bold && line.size >= body && line.text.chars().count() <= 60
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
