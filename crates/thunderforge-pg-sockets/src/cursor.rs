//! When it is safe to say "I have seen everything up to here".
//!
//! Moved out of the server's listener so the rule can be tested as a rule,
//! against plain values, rather than only against a live database.

use std::time::Duration;

use chrono::NaiveDateTime;

/// How long a row must have existed before the cursor may pass it.
///
/// This is the window in which a transaction can still be holding an id it
/// took but has not committed. It bounds the race described on
/// [`settled_cursor`]: a transaction that keeps its id longer than this can
/// still be missed, and one that commits within it cannot.
///
/// Two seconds is chosen against what actually writes here — single-statement
/// inserts at the end of a mutation that has already done its work — where
/// the gap between taking an id and committing is sub-millisecond. It costs
/// nothing at delivery time: rows are broadcast the moment they are seen, and
/// only the cursor waits.
pub const COMMIT_GRACE: Duration = Duration::from_secs(2);

/// One row's identity, as far as the cursor is concerned.
///
/// A tuple rather than the server's `WorldEvent` on purpose — the decision
/// needs an id and a creation time and nothing else, and taking only those
/// two is what lets this be exercised without a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowStamp {
    pub id: i64,
    pub created_at: NaiveDateTime,
}

/// The highest id the cursor may advance to, given what this poll returned.
///
/// # The bug this exists to prevent
///
/// `world_events.id` is `BIGSERIAL`. A sequence value is taken at **INSERT**
/// and the row becomes visible at **COMMIT**, and those two orders need not
/// agree. If transaction A takes id 102 and B takes 103, and B commits first,
/// a poll landing between them sees only `[103]`. Advancing the cursor to 103
/// means 102 — committing a millisecond later — can never satisfy
/// `id > cursor` again. It is lost permanently, with no log, no metric and no
/// `Lagged`: the quietest failure in the whole delivery path.
///
/// That is not hypothetical. The server's
/// `a_poll_between_out_of_order_commits_still_delivers_the_lower_id` stages
/// exactly that interleaving against a real database, and before this rule
/// existed it lost the event every time.
///
/// # The rule
///
/// Walk the batch in ascending id order and stop at the first row young
/// enough that an older sibling could still be uncommitted. Everything before
/// it is settled: any transaction that took a lower id has had longer than
/// [`COMMIT_GRACE`] to commit, so if it has not appeared by now it never will
/// (it rolled back, and its id is a permanent gap).
///
/// Delivery does not wait for this. The caller broadcasts rows as soon as it
/// sees them and must therefore remember which ids it has already sent, since
/// a lagging cursor re-reads them on the next pass.
pub fn settled_cursor(rows: &[RowStamp], current: i64, now: NaiveDateTime) -> i64 {
    let cutoff = now - chrono::Duration::from_std(COMMIT_GRACE).unwrap_or_default();
    let mut settled = current;
    for row in rows {
        if row.created_at >= cutoff {
            break;
        }
        settled = settled.max(row.id);
    }
    settled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(id: i64, created_at: NaiveDateTime) -> RowStamp {
        RowStamp { id, created_at }
    }

    fn now() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    #[test]
    fn the_cursor_stops_at_the_first_row_too_young_to_be_settled() {
        let now = now();
        let old = now - chrono::Duration::seconds(60);
        let fresh = now - chrono::Duration::milliseconds(10);

        // 13 is old but sits *after* a young row: order decides where the walk
        // stops, not age. Passing 13 would strand whatever took id 12.
        let rows = [at(10, old), at(11, old), at(12, fresh), at(13, old)];

        assert_eq!(settled_cursor(&rows, 9, now), 11);
    }

    #[test]
    fn an_empty_poll_moves_nothing() {
        assert_eq!(settled_cursor(&[], 9, now()), 9);
    }

    #[test]
    fn a_batch_of_nothing_but_fresh_rows_leaves_the_cursor_alone() {
        let now = now();
        let fresh = now - chrono::Duration::milliseconds(10);
        assert_eq!(settled_cursor(&[at(10, fresh), at(11, fresh)], 9, now), 9);
    }

    #[test]
    fn an_entirely_settled_batch_advances_all_the_way() {
        let now = now();
        let old = now - chrono::Duration::seconds(60);
        assert_eq!(settled_cursor(&[at(10, old), at(11, old)], 9, now), 11);
    }

    #[test]
    fn the_cursor_never_goes_backwards() {
        // A batch that somehow arrives with ids below the cursor — a replay,
        // a caller passing a stale value — must not rewind it, or events
        // already delivered would be delivered again forever.
        let now = now();
        let old = now - chrono::Duration::seconds(60);
        assert_eq!(settled_cursor(&[at(3, old), at(4, old)], 99, now), 99);
    }
}
