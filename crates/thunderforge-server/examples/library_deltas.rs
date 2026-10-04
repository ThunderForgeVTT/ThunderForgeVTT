//! What a world's changes cost to read (spec 050 FR-072, SC-004; task T076).
//!
//! FR-072 says resolving base plus delta must not make reading content
//! noticeably slower than reading world-owned content, and refuses to say what
//! "noticeably" is until it has been measured. This measures it, against a
//! real Postgres, and prints the section that goes into
//! `specs/049-importing-a-source-book/measurements.md`.
//!
//! ```bash
//! cargo run --release -p thunderforge-server --features test-support \
//!     --example library_deltas
//! ```
//!
//! Release, unlike `library_storage`: that one counts bytes, which no compiler
//! profile changes; this one counts time, and the in-memory half of the
//! resolution is exactly the part a debug build would misrepresent.
//!
//! # What is compared
//!
//! Reads of the same number of rows, decomposed so each cost lands where it
//! belongs:
//!
//! 1. **world-owned** — a world's own items, with descriptions padded to a
//!    book entry's field map, read as text and then decoded as a map. SC-004
//!    names world-owned content as the bar, and the second of these is the
//!    fair one.
//! 2. **inherited, before this phase** — the entries query alone, then Phase
//!    9's book-list fetch around it.
//! 3. **inherited, resolved** — the same book with this phase's resolution,
//!    at 0, 1%, 10% and 100% of its entries changed.
//! 4. **the resolution alone**, in memory, with no database — so the cost of
//!    the rule can be told from the cost of the extra query.
//!
//! # What it leaves behind
//!
//! Nothing: one transaction, rolled back, as `library_storage` does.

#![allow(clippy::print_stdout)] // a command-line tool: its output is the point

use std::time::{Duration, Instant};

use diesel::prelude::*;
use thunderforge_server::compendium::ContentOrigin;
use thunderforge_server::compendium::store::{self, NewBook};
use thunderforge_server::content::{Entry, NameState, ReadValue};
use thunderforge_server::library::{book_list, deltas};
use thunderforge_server::schema::{world_items, worlds};
use thunderforge_server::test_support::{insert_test_user, insert_test_world, test_app_state};
use uuid::Uuid;

/// A large real book, as `library_storage` sizes one.
const ENTRIES: usize = 1_500;

/// Timed repetitions of each read, after `WARM` untimed ones so the first
/// query plan and a cold cache are not what gets reported.
const RUNS: usize = 200;
const WARM: usize = 5;

/// The shares of a book a world has changed. 0% is the common case for a
/// freshly inherited book; 100% is a ceiling nobody reaches by hand, measured
/// so the margin is fixed against the worst case rather than a typical one.
const CHANGED: [usize; 4] = [0, 15, 150, 1_500];

const SYSTEM: &str = "delta-measurement";

fn main() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a database connection");

    let mut measured = None;
    let outcome = conn.transaction::<(), diesel::result::Error, _>(|conn| {
        measured = Some(measure(conn));
        Err(diesel::result::Error::RollbackTransaction)
    });
    assert!(
        matches!(outcome, Err(diesel::result::Error::RollbackTransaction)),
        "the measurement transaction must roll back and nothing else"
    );

    print!("{}", measured.expect("the measurement ran"));
}

#[derive(Clone, Copy)]
struct Timing {
    median: Duration,
    p95: Duration,
}

fn time<T>(mut read: impl FnMut() -> T) -> Timing {
    for _ in 0..WARM {
        std::hint::black_box(read());
    }
    let mut runs: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            std::hint::black_box(read());
            started.elapsed()
        })
        .collect();
    runs.sort();
    Timing {
        median: runs[RUNS / 2],
        p95: runs[(RUNS * 95) / 100],
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

struct Row {
    label: String,
    timing: Timing,
}

fn measure(conn: &mut PgConnection) -> String {
    let owner = insert_test_user(conn);
    let world = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world)))
        .set(worlds::game_system_id.eq(Some(SYSTEM.to_string())))
        .execute(conn)
        .expect("the world to declare a system");

    let book_entries = entries(ENTRIES);
    let book = store::import_book(conn, owner, a_book(), &book_entries).expect("the book");
    book_list::switch_on(conn, owner, world, book.id).expect("the book to switch on");

    // World-owned content of the same size, so the bar is not lower merely
    // because its rows are thinner.
    let padding = serde_json::to_string(&book_entries[0].values).expect("fields serialise");
    let now = chrono::Utc::now().naive_utc();
    let items: Vec<_> = (0..ENTRIES)
        .map(|n| {
            (
                world_items::id.eq(Uuid::now_v7()),
                world_items::world_id.eq(world),
                world_items::name.eq(format!("Item number {n}")),
                world_items::description.eq(Some(padding.clone())),
                world_items::created_by.eq(owner),
                world_items::created_at.eq(now),
                world_items::updated_at.eq(now),
            )
        })
        .collect();
    for chunk in items.chunks(500) {
        diesel::insert_into(world_items::table)
            .values(chunk)
            .execute(conn)
            .expect("world items");
    }
    diesel::sql_query("ANALYZE world_items, compendium_entries, world_entry_deltas")
        .execute(conn)
        .expect("fresh statistics");

    let mut rows = Vec::new();

    let world_owned = time(|| {
        world_items::table
            .filter(world_items::world_id.eq(world))
            .order(world_items::name.asc())
            .select((world_items::id, world_items::name, world_items::description))
            .load::<(Uuid, String, Option<String>)>(conn)
            .expect("world items read")
    });
    rows.push(Row {
        label: format!("World-owned: {ENTRIES} of a world's own items, as text"),
        timing: world_owned,
    });

    // The same world-owned rows, with their fields decoded the way a book
    // entry's are. A world's own item stores prose; a book entry stores a
    // tagged field map that has to become a value before it can be handed
    // out. Without this row the comparison would charge the delta model for
    // the difference between a string and a map.
    let world_owned_decoded = time(|| {
        world_items::table
            .filter(world_items::world_id.eq(world))
            .order(world_items::name.asc())
            .select((world_items::id, world_items::name, world_items::description))
            .load::<(Uuid, String, Option<String>)>(conn)
            .expect("world items read")
            .into_iter()
            .map(|(id, name, description)| {
                let fields: serde_json::Value =
                    serde_json::from_str(description.as_deref().unwrap_or("{}"))
                        .expect("padding is JSON");
                (id, name, fields)
            })
            .collect::<Vec<_>>()
    });
    rows.push(Row {
        label: format!("World-owned: the same {ENTRIES}, fields decoded as a book's are"),
        timing: world_owned_decoded,
    });

    let query_alone = time(|| {
        store::entries_for(conn, owner, book.id, None).expect("the entries, from the shelf")
    });
    rows.push(Row {
        label: "Inherited: the entries query alone, no book-list checks".to_string(),
        timing: query_alone,
    });

    let unresolved =
        time(|| book_list::entries_served_by(conn, world, book.id, None).expect("the fetch"));
    rows.push(Row {
        label: "Inherited: Phase 9's fetch, no delta model".to_string(),
        timing: unresolved,
    });

    let mut resolved = Vec::new();
    let mut changed_so_far = 0;
    for target in CHANGED {
        for n in changed_so_far..target {
            deltas::change_entry(
                conn,
                owner,
                world,
                book.id,
                "creature",
                &format!("Creature number {n}"),
                deltas::Content::Fields(
                    [(
                        "hits".to_string(),
                        ReadValue::Clear(format!("{} (retuned)", n % 90)),
                    )]
                    .into_iter()
                    .collect(),
                ),
            )
            .expect("a change");
        }
        changed_so_far = target;
        diesel::sql_query("ANALYZE world_entry_deltas")
            .execute(conn)
            .expect("fresh statistics");

        let timing = time(|| {
            let (_, read) =
                deltas::world_reads(conn, world, book.id, None, false).expect("the resolution");
            assert_eq!(read.entries.len(), ENTRIES);
            read
        });
        resolved.push((target, timing));
        rows.push(Row {
            label: format!(
                "Inherited, resolved: {target} of {ENTRIES} changed ({}%)",
                target * 100 / ENTRIES
            ),
            timing,
        });
    }

    // The rule alone, on lists already in memory. `resolve` consumes its
    // inputs, so a fresh copy is made for every run *before* the clock starts
    // — timing the copy would charge the rule for work a real read does not do.
    let (_, base) = book_list::entries_served_by(conn, world, book.id, None).expect("the fetch");
    let held = deltas::deltas_over(conn, world, book.id, None).expect("the deltas");
    let mut inputs: Vec<_> = (0..WARM + RUNS)
        .map(|_| (base.clone(), held.clone()))
        .collect();
    let in_memory = time(|| {
        let (base, held) = inputs.pop().expect("an input per run");
        deltas::resolve(ContentOrigin::Uploaded, base, held, false)
    });
    rows.push(Row {
        label: format!("The resolution alone, in memory, {} changed", held.len()),
        timing: in_memory,
    });

    render(&Measured {
        rows,
        world_owned,
        world_owned_decoded,
        query_alone,
        unresolved,
        resolved,
        in_memory,
    })
}

struct Measured {
    rows: Vec<Row>,
    world_owned: Timing,
    world_owned_decoded: Timing,
    query_alone: Timing,
    unresolved: Timing,
    resolved: Vec<(usize, Timing)>,
    in_memory: Timing,
}

/// A margin above an observed ratio: the observation plus a tenth, rounded up
/// to the next quarter. Never the observation itself — a margin equal to what
/// was measured fails on the first noisy run, and a margin that fails on noise
/// stops being read.
fn margin_above(ratio: f64) -> f64 {
    ((ratio + 0.1) * 4.0).ceil() / 4.0
}

fn render(m: &Measured) -> String {
    let table: String = m
        .rows
        .iter()
        .map(|row| {
            format!(
                "| {} | {:.2} | {:.2} |\n",
                row.label,
                ms(row.timing.median),
                ms(row.timing.p95)
            )
        })
        .collect();

    let ratio = |over: Timing, under: Timing| ms(over.median) / ms(under.median);
    let (_, none_changed) = m.resolved[0];
    let (_, all_changed) = *m.resolved.last().expect("at least one level");
    // "A world has changed some of a book", as a Game Master does it by hand:
    // every level up to and including 10%.
    let realistic_worst = m
        .resolved
        .iter()
        .filter(|(changed, _)| changed * 10 <= ENTRIES)
        .map(|(_, timing)| ratio(*timing, m.unresolved))
        .fold(0.0_f64, f64::max);
    let realistic_vs_owned = m
        .resolved
        .iter()
        .filter(|(changed, _)| changed * 10 <= ENTRIES)
        .map(|(_, timing)| ratio(*timing, m.world_owned_decoded))
        .fold(0.0_f64, f64::max);

    format!(
        r#"
## Phase 11: what a world's changes cost to read (T076)

**Date**: {date} · **Commit**: `{commit}`

```bash
cargo run --release -p thunderforge-server --features test-support \
    --example library_deltas
```

One synthetic book of {entries} creatures, switched on in one world, beside
{entries} of that world's own items whose descriptions are padded to the same
field map. Every read is the whole book at once. {warm} untimed runs, then
{runs} timed; median and 95th percentile in milliseconds, wall clock, against
a local Postgres, inside one transaction that is rolled back.

| Read | Median ms | p95 ms |
|---|---|---|
{table}
### Reading this honestly

**An inherited book was already slower to read than a world's own content,
before any delta existed**, and the reason is measurable rather than
guessable. A world's own rows as text: {owned:.2} ms. The same rows with their
fields decoded into a map, as a book entry's must be: {owned_decoded:.2} ms.
The book's entries query alone: {query_alone:.2} ms, and Phase 9's fetch
around it, with the book-list and system checks: {unresolved:.2} ms. Of the
{gap:.2} ms between a world's text and the entries query, decoding a field map
accounts for {decode_share:.0}%; the rest is the wider stored row (page, name
certainty, suspicion, extras, JSONB rather than text). The checks add
{checks:+.2} ms. None of it is this phase's. It is recorded because SC-004
names world-owned content as the bar, and a bar of plain text would have
charged the delta model for Phase 9's shape.

**What the delta model adds**, over Phase 9's fetch at the median: with
nothing changed, {none_cost:+.2} ms ({none_ratio:.2}x); at every level up to
10% of the book changed, at most {realistic_worst:.2}x; at every entry changed,
{all_ratio:.2}x. The rule itself, in memory, with every entry changed, is
{mem:.2} ms of that.

**The margins SC-004 and FR-072 were waiting for**, fixed from the numbers
above plus a tenth, rounded up to the next quarter:

- **FR-072**: resolving a book a world has changed by hand — up to 10% of its
  entries — MUST stay within **{fr072:.2}x** the time of Phase 9's
  unresolved fetch of the same book. A world that has changed every entry MUST
  stay within **{fr072_all:.2}x**.
- **SC-004**: reading an inherited book, resolved, up to 10% changed, MUST
  stay within **{sc004:.2}x** reading the same number of world-owned rows
  carrying the same field map (the second row above).

Ratios and not milliseconds on purpose: milliseconds are a property of the
machine, and a ratio of two reads taken in the same run on the same machine is
a property of the code.

**Threats to validity.**
- *One machine, a local Postgres, one run.* The ordering of adjacent levels
  (0% against 1%) is within run-to-run noise and should not be read as one
  being faster; the 100% row is far outside it. A remote database adds a round
  trip to every read, and this phase adds one query, so its share would grow
  there.
- *Synthetic entries of one shape.* Real entries vary by kind. The padded
  world-owned row matches the book entry's field map in bytes and in decoding,
  not the variety of a real book.
- *The whole book per read.* The wire pages at 200 entries at most, but the
  resolution happens before paging, so every page pays for resolving the whole
  book. That is a real cost of this design and it is what is measured: a book
  several times larger than {entries} entries is where resolving per page
  would be worth its complexity.
- *Inside a transaction*, so reads see their own uncommitted rows; every
  compared read pays that alike.

**What would reopen it**: resolving per page, any change to
`book_list::entries_served_by` or to what an entry stores, or a book an order
of magnitude larger.

---
"#,
        date = chrono::Utc::now().format("%Y-%m-%d"),
        commit = commit(),
        entries = ENTRIES,
        warm = WARM,
        runs = RUNS,
        table = table,
        owned = ms(m.world_owned.median),
        owned_decoded = ms(m.world_owned_decoded.median),
        query_alone = ms(m.query_alone.median),
        unresolved = ms(m.unresolved.median),
        gap = ms(m.query_alone.median) - ms(m.world_owned.median),
        decode_share = 100.0 * (ms(m.world_owned_decoded.median) - ms(m.world_owned.median))
            / (ms(m.query_alone.median) - ms(m.world_owned.median)),
        checks = ms(m.unresolved.median) - ms(m.query_alone.median),
        none_cost = ms(none_changed.median) - ms(m.unresolved.median),
        none_ratio = ratio(none_changed, m.unresolved),
        realistic_worst = realistic_worst,
        all_ratio = ratio(all_changed, m.unresolved),
        mem = ms(m.in_memory.median),
        fr072 = margin_above(realistic_worst),
        fr072_all = margin_above(ratio(all_changed, m.unresolved)),
        sc004 = margin_above(realistic_vs_owned),
    )
}

fn commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn a_book() -> NewBook {
    NewBook {
        book_title: "Monster Manual".to_string(),
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: SYSTEM.to_string(),
        parser_version: "measurement".to_string(),
        page_count: 320,
        silent_page_count: 0,
    }
}

/// The entries `library_storage` measures, so the two measurements describe
/// the same book.
fn entries(how_many: usize) -> Vec<Entry> {
    (0..how_many)
        .map(|n| Entry {
            kind: "creature".to_string(),
            name: format!("Creature number {n}"),
            name_state: NameState::Clear,
            page: (n % 320) as u32 + 1,
            values: [
                ("armour".to_string(), ReadValue::Clear("15".to_string())),
                ("hits".to_string(), ReadValue::Clear("7 (2d6)".to_string())),
                (
                    "speed".to_string(),
                    ReadValue::Clear("30 ft., fly 60 ft.".to_string()),
                ),
                (
                    "challenge".to_string(),
                    ReadValue::Uncertain("1/4 (50 XP)".to_string()),
                ),
                ("alignment".to_string(), ReadValue::Unread),
                (
                    "senses".to_string(),
                    ReadValue::Clear("darkvision 60 ft., passive Perception 9".to_string()),
                ),
            ]
            .into_iter()
            .collect(),
            text: None,
            suspect: false,
            extras: None,
        })
        .collect()
}
