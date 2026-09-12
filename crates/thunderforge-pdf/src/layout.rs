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
    let columns = column_starts(&ordered);
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

    out.into_iter()
        .flat_map(|group| split_at_gutters(group, &columns))
        .collect()
}

/// How close a run must start to a column edge to be counted as beginning it.
const COLUMN_TOLERANCE: f64 = 3.0;

/// Where this page's columns begin.
///
/// # Why the page is asked rather than the gap measured
///
/// Measuring each gap and calling anything wide enough a gutter works until a
/// book sets its columns close together. The Player's Handbook leaves **under
/// one em** between them — narrower than the threshold that separates a
/// gutter from a wide word space — so every spell on the page came out with
/// the left column's sentence welded to the right column's: "A ring is At
/// Higher Levels. When you cast this spell".
///
/// A page cannot hide where its columns are, though. Hundreds of lines start
/// at exactly the same x, and nothing else on a page does that. Finding those
/// positions first turns a threshold that has to be right for every book into
/// a measurement of the book in hand.
fn column_starts(runs: &[&TextRun]) -> Vec<f64> {
    if runs.len() < 20 {
        return Vec::new();
    }
    // Counted in whole points: two runs beginning the same column agree to
    // within rounding, never exactly.
    let mut tally: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for run in runs {
        *tally.entry(run.x.round() as i64).or_default() += 1;
    }

    // A real column edge carries a substantial share of the page's runs. The
    // fraction is deliberately low: a two-column page splits its lines between
    // them, and a short column still marks its own edge.
    let threshold = (runs.len() / 12).max(4);
    let mut starts: Vec<f64> = tally
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(x, _)| x as f64)
        .collect();
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Neighbouring positions are the same edge seen through rounding and
    // hinting. Keeping both would split a line twice at the same place.
    let mut merged: Vec<f64> = Vec::new();
    for start in starts {
        if merged
            .last()
            .is_none_or(|previous| start - previous > COLUMN_TOLERANCE * 2.0)
        {
            merged.push(start);
        }
    }
    merged
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
fn split_at_gutters(mut group: Vec<&TextRun>, columns: &[f64]) -> Vec<Line> {
    group.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = Vec::new();
    let mut current: Vec<&TextRun> = Vec::new();
    let mut previous_end: Option<f64> = None;

    for run in group {
        // A run that begins a column the page has one of is a new line,
        // however narrow the gap in front of it. This is what a close-set book
        // needs and what measuring the gap alone cannot give.
        let begins_a_column = !current.is_empty()
            && columns
                .iter()
                .skip(1)
                .any(|start| (run.x - start).abs() <= COLUMN_TOLERANCE);
        let wide_gap = previous_end.is_some_and(|end| run.x - end > run.size * GUTTER);

        if !current.is_empty() && (begins_a_column || wide_gap) {
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
    // Where the text stops, not where its last run starts — the difference is
    // a whole run, and `reading_order` uses this to decide what spans a page.
    let x1 = group
        .iter()
        .map(|r| r.x + r.width)
        .fold(f64::NEG_INFINITY, f64::max);
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

/// A page's text in reading order, one line per entry.
///
/// # Why not a midpoint
///
/// The obvious rule — anything starting left of centre is the left column —
/// fails on the first book whose columns are not centred. The Player's
/// Handbook begins its right column at x=317 on a page 648 wide, so the
/// midpoint is 324 and *both* columns read as the left one. Every spell came
/// out interleaved with its neighbour.
///
/// So the columns are found the same way [`lines`] finds them: by where lines
/// actually start. A line is assigned to the nearest column edge at or before
/// it, and a line that reaches past the next edge spans the page — a title
/// across both columns, which belongs before either.
pub fn reading_order(lines: Vec<Line>, geometry: PageGeometry) -> Vec<Line> {
    if lines.len() < 4 {
        return lines;
    }

    let columns = line_columns(&lines);
    if columns.len() < 2 {
        // One column, or too little evidence for two. Down the page as it is.
        let _ = geometry;
        return lines;
    }

    let column_of = |line: &Line| -> Option<usize> {
        let index = columns
            .iter()
            .rposition(|start| line.x0 + COLUMN_TOLERANCE >= *start)?;
        // A line reaching well into the next column is not in this one — it
        // spans, and a spanning line is ordered before the columns.
        match columns.get(index + 1) {
            Some(next) if line.x1 > next + COLUMN_TOLERANCE => None,
            _ => Some(index),
        }
    };

    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    out.extend(
        lines
            .iter()
            .filter(|line| column_of(line).is_none())
            .cloned(),
    );
    for index in 0..columns.len() {
        out.extend(
            lines
                .iter()
                .filter(|line| column_of(line) == Some(index))
                .cloned(),
        );
    }
    out
}

/// Where this page's columns begin, judged from assembled lines.
///
/// The same measurement [`column_starts`] makes over runs, over lines
/// instead: by this point a line begins exactly where its column does, which
/// is a cleaner signal than the runs were.
fn line_columns(lines: &[Line]) -> Vec<f64> {
    let mut tally: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for line in lines {
        *tally.entry(line.x0.round() as i64).or_default() += 1;
    }
    // A column has to hold a real share of the page. Below this it is a
    // hanging indent or a caption, and treating it as a column would order the
    // page around a footnote.
    let threshold = (lines.len() / 8).max(3);
    let mut starts: Vec<f64> = tally
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(x, _)| x as f64)
        .collect();
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut merged: Vec<f64> = Vec::new();
    for start in starts {
        if merged
            .last()
            .is_none_or(|previous| start - previous > COLUMN_TOLERANCE * 2.0)
        {
            merged.push(start);
        }
    }
    merged
}

/// How much bigger than the body a line must be to read as a heading.
const HEADING_RATIO: f64 = 1.15;

/// Whether text is letter-spaced past the point of trust, and whether it is
/// language at all.
///
/// Implemented in `thunderforge_content::text` and re-exported here. They are
/// judgements about content rather than geometry, and this crate has to depend
/// on that one anyway — the readers, the layout pass and the browser must all
/// reach the same verdict, which they do by being one function rather than
/// three that agree today.
pub use thunderforge_content::text::{looks_letter_spaced, looks_unreadable};

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
