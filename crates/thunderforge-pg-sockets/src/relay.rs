//! What one poll of the event table means: what to broadcast, and where the
//! cursor lands afterwards.
//!
//! # Why this is a separate thing from [`crate::cursor`]
//!
//! [`settled_cursor`] answers one question — how far the "I have seen
//! everything up to here" mark may advance. That is the subtle half, and it
//! was extracted first. But it is not the whole decision. Around it sits a
//! second rule that was never extracted and never tested: *which rows in this
//! batch have I already sent?*
//!
//! Those two are coupled, and the coupling is the part that bites. The cursor
//! deliberately lags behind delivery, so every poll re-reads rows it has
//! already broadcast. Something has to remember them, that memory has to be
//! pruned or it grows forever, and the pruning is only correct if it agrees
//! exactly with where the cursor is. Get that agreement wrong in either
//! direction and you produce one of the two quietest bugs in the system:
//! every event delivered twice, or an event delivered never.
//!
//! In the server that logic lived inline in an async task next to a
//! connection pool, a database and a 100ms timer. It could only be exercised
//! by running the whole stack and hoping the interleaving showed up — which
//! is how a burst-shaped delivery failure survived in it long enough to need
//! a live packet-level investigation to characterise. Here it is a struct
//! with two fields and no I/O, and the interleavings are just test cases.
//!
//! # What is deliberately still in the server
//!
//! The poll query, the pool, the timer and the broadcast. This type is handed
//! the rows somebody else fetched and returns the rows somebody else should
//! publish. It cannot block, cannot fail, and cannot touch a socket.

use std::collections::BTreeSet;

use chrono::NaiveDateTime;

use crate::cursor::{RowStamp, settled_cursor};

/// A row this relay can reason about: an id and a commit time, nothing more.
///
/// Implemented by the server for its `WorldEvent` rather than this crate
/// knowing what an event is — the same boundary the rest of the crate keeps.
pub trait Stamped {
    fn stamp(&self) -> RowStamp;
}

impl Stamped for RowStamp {
    fn stamp(&self) -> RowStamp {
        *self
    }
}

/// Tracks what has been broadcast and how far the cursor has settled.
///
/// Construct it with the high-water mark to stream *after* — a fresh listener
/// starts from the newest existing row, because a listener is for what
/// happens next and replaying the table to every client on boot is not that.
#[derive(Debug)]
pub struct Relay {
    cursor: i64,
    /// Ids already broadcast but not yet passed by the cursor.
    ///
    /// Bounded by the number of events inside one [`crate::COMMIT_GRACE`]
    /// window, because everything at or below the cursor is dropped on each
    /// pass. Without that pruning this is a leak that only shows up on a
    /// server that has been up for weeks.
    delivered: BTreeSet<i64>,
}

impl Relay {
    pub fn new(start_after: i64) -> Self {
        Self {
            cursor: start_after,
            delivered: BTreeSet::new(),
        }
    }

    /// The id the next poll should bound on: fetch rows with `id > cursor()`.
    pub fn cursor(&self) -> i64 {
        self.cursor
    }

    /// How many ids are being remembered for de-duplication.
    ///
    /// Exposed for metrics: if this climbs without bound, the cursor has
    /// stopped settling and delivery is running on borrowed time.
    pub fn tracked(&self) -> usize {
        self.delivered.len()
    }

    /// Take the rows one poll returned; get back the rows to broadcast.
    ///
    /// Rows are returned in the order given, which the caller is expected to
    /// have made ascending by id. Anything already broadcast is filtered out;
    /// the cursor advances only as far as [`settled_cursor`] permits, which is
    /// usually *not* as far as the last row seen.
    ///
    /// Broadcasting deliberately does not wait for the cursor to settle.
    /// Making every event wait out the commit grace would put that delay on
    /// the latency of every token drag to buy safety against a race that
    /// affects almost none of them.
    pub fn absorb<T: Stamped>(&mut self, rows: Vec<T>, now: NaiveDateTime) -> Vec<T> {
        let stamps: Vec<RowStamp> = rows.iter().map(Stamped::stamp).collect();
        let settled = settled_cursor(&stamps, self.cursor, now);

        let mut publish = Vec::new();
        for row in rows {
            // `insert` returns false when we have sent this id before, which
            // is the ordinary case for the rows between the cursor and the
            // newest — the cursor lags on purpose, so they come back.
            if self.delivered.insert(row.stamp().id) {
                publish.push(row);
            }
        }

        // The cursor never moves backwards: `settled_cursor` is given the
        // current value as its floor and returns at least it.
        self.cursor = settled;
        // Anything the cursor has passed can never be returned by a future
        // poll (`id > cursor`), so remembering it serves no purpose.
        self.delivered = self.delivered.split_off(&(self.cursor + 1));

        publish
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    fn old(id: i64, now: NaiveDateTime) -> RowStamp {
        RowStamp {
            id,
            created_at: now - chrono::Duration::seconds(60),
        }
    }

    fn fresh(id: i64, now: NaiveDateTime) -> RowStamp {
        RowStamp {
            id,
            created_at: now,
        }
    }

    #[test]
    fn every_row_in_a_settled_batch_is_published_once_and_the_cursor_passes_it() {
        let now = now();
        let mut relay = Relay::new(0);

        let out = relay.absorb(vec![old(1, now), old(2, now), old(3, now)], now);

        assert_eq!(out.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(relay.cursor(), 3);
        // Nothing left to remember: the cursor has passed all of it.
        assert_eq!(relay.tracked(), 0);
    }

    /// The re-read that the lagging cursor makes inevitable must not duplicate.
    ///
    /// A row too young to settle stays behind the cursor, so the next poll
    /// fetches it again. It has already been broadcast, and broadcasting it
    /// twice would show a player the same token move twice.
    #[test]
    fn a_row_re_read_because_the_cursor_lagged_is_not_published_twice() {
        let now = now();
        let mut relay = Relay::new(0);

        // 7 is too young for the cursor to pass, so the cursor stops at 6.
        let first = relay.absorb(vec![old(6, now), fresh(7, now)], now);
        assert_eq!(first.iter().map(|r| r.id).collect::<Vec<_>>(), vec![6, 7]);
        assert_eq!(relay.cursor(), 6);
        assert_eq!(
            relay.tracked(),
            1,
            "7 is remembered; the cursor has not passed it"
        );

        // The next poll bounds on `id > 6` and so sees 7 again.
        let second = relay.absorb(vec![fresh(7, now)], now);
        assert!(second.is_empty(), "7 was already broadcast");
    }

    /// The bug [`settled_cursor`] exists for, now proven end to end.
    ///
    /// `BIGSERIAL` takes its value at INSERT and the row appears at COMMIT, so
    /// a lower id can commit *after* a higher one. If the cursor advanced to
    /// the highest id seen, the straggler could never satisfy `id > cursor`
    /// again — lost with no log, no metric and no error.
    #[test]
    fn a_lower_id_committing_late_is_still_delivered() {
        let now = now();
        let mut relay = Relay::new(100);

        // 103 commits first and is still young, so the cursor must not pass it.
        let first = relay.absorb(vec![fresh(103, now)], now);
        assert_eq!(first.iter().map(|r| r.id).collect::<Vec<_>>(), vec![103]);
        assert_eq!(
            relay.cursor(),
            100,
            "the cursor may not pass an unsettled row"
        );

        // 102 appears a moment later. The cursor never passed it, so the
        // `id > 100` bound still finds it.
        let second = relay.absorb(vec![fresh(102, now), fresh(103, now)], now);
        assert_eq!(
            second.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![102],
            "102 is new, 103 was already sent"
        );
    }

    /// A burst larger than one poll batch must drain, not wedge.
    ///
    /// This is the shape that produced a live delivery freeze: a storm of
    /// events arriving faster than one poll could carry them. The poll is
    /// bounded by a `LIMIT`, so the relay only ever sees a window of the
    /// backlog, and the question is whether repeated polls make progress or
    /// return the same window forever.
    #[test]
    fn a_backlog_deeper_than_one_batch_drains_over_successive_polls() {
        let base = now();
        const BATCH: usize = 8;

        // 40 settled rows waiting, and a poll that can only carry 8.
        let backlog: Vec<RowStamp> = (1..=40).map(|id| old(id, base)).collect();
        let mut relay = Relay::new(0);
        let mut published = Vec::new();

        for _ in 0..10 {
            let window: Vec<RowStamp> = backlog
                .iter()
                .filter(|r| r.id > relay.cursor())
                .take(BATCH)
                .copied()
                .collect();
            if window.is_empty() {
                break;
            }
            published.extend(relay.absorb(window, base).into_iter().map(|r| r.id));
        }

        assert_eq!(
            published,
            (1..=40).collect::<Vec<_>>(),
            "the whole backlog drains, in order and once each"
        );
        assert_eq!(relay.cursor(), 40);
    }

    /// The de-duplication memory must not grow without bound.
    ///
    /// Every id ever seen is inserted into `delivered`; only the pruning stops
    /// that being a leak on a server that stays up for weeks.
    #[test]
    fn the_dedupe_set_does_not_grow_with_total_events_delivered() {
        let base = now();
        let mut relay = Relay::new(0);

        for id in 1..=5_000 {
            relay.absorb(vec![old(id, base)], base);
        }

        assert_eq!(relay.cursor(), 5_000);
        assert_eq!(
            relay.tracked(),
            0,
            "settled ids are forgotten; only the unsettled tail is held"
        );
    }

    /// An empty poll is not a signal to move, and must not lose the tail.
    #[test]
    fn an_empty_poll_leaves_the_cursor_and_the_pending_tail_alone() {
        let now = now();
        let mut relay = Relay::new(10);
        relay.absorb(vec![fresh(11, now)], now);
        assert_eq!(relay.tracked(), 1);

        let out = relay.absorb(Vec::<RowStamp>::new(), now);

        assert!(out.is_empty());
        assert_eq!(relay.cursor(), 10);
        assert_eq!(relay.tracked(), 1, "11 is still awaiting the cursor");
    }
}
