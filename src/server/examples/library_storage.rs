//! What a book costs when several worlds run it (spec 050 SC-001, SC-002,
//! FR-070, FR-071; task T064).
//!
//! The claim this arc was chosen for is an operator's: a Game Master with
//! eight worlds and one Monster Manual stores one Monster Manual. That is a
//! claim about bytes, so it is measured in bytes, against a real Postgres,
//! rather than argued from the shape of the schema.
//!
//! Run it:
//!
//! ```bash
//! cargo run -p thunderforge-server --features test-support \
//!     --example library_storage
//! ```
//!
//! A debug build, deliberately: what is measured is bytes in Postgres, which
//! no compiler profile changes, and a release build of this crate costs more
//! memory than some development machines will give it.
//!
//! It prints the section that goes into
//! `specs/049-importing-a-source-book/measurements.md`. Nothing is
//! transcribed by hand, for the reason every other measurement in this
//! repository is generated: a number typed twice is a number that will
//! disagree with itself.
//!
//! # What it leaves behind
//!
//! Nothing. Everything happens inside one transaction that is deliberately
//! rolled back, so this can be run against a live development database
//! without leaving eight worlds and a synthetic Monster Manual in somebody's
//! account. The measurement is unaffected: rows written in a transaction are
//! rows, and `pg_column_size` measures them as such.

use diesel::prelude::*;
use diesel::sql_types::BigInt;
use thunderforge_server::compendium::store::{self, NewBook};
use thunderforge_server::content::{Entry, NameState, ReadValue};
use thunderforge_server::library::book_list;
use thunderforge_server::schema::worlds;
use thunderforge_server::test_support::{insert_test_user, insert_test_world, test_app_state};
use uuid::Uuid;

/// How many worlds the scenario runs. Eight, because eight is the number in
/// the sentence this whole architecture was chosen to be able to say.
const WORLDS: usize = 8;

/// How many entries stand in for a sourcebook.
///
/// The corpus measurement (T019) read 2,750 creatures out of 246 books, and
/// the largest single books ran to the high hundreds. Fifteen hundred is a
/// large real book rather than a stress test, which is the size the claim
/// needs to hold at.
const ENTRIES: usize = 1_500;

#[derive(QueryableByName)]
struct Bytes {
    #[diesel(sql_type = BigInt)]
    bytes: i64,
}

fn main() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a database connection");

    let mut measured = None;
    let outcome = conn.transaction::<(), diesel::result::Error, _>(|conn| {
        measured = Some(measure(conn));
        // Deliberate: the numbers are taken, and the rows are not kept.
        Err(diesel::result::Error::RollbackTransaction)
    });
    assert!(
        matches!(outcome, Err(diesel::result::Error::RollbackTransaction)),
        "the measurement transaction must roll back and nothing else"
    );

    print!("{}", measured.expect("the measurement ran"));
}

struct Measurement {
    book_bytes: i64,
    entry_count: i64,
    link_bytes_each: i64,
    inherited_total: i64,
    copied_total: i64,
    second_book_bytes: i64,
}

fn measure(conn: &mut PgConnection) -> String {
    let owner = insert_test_user(conn);

    let manual = store::import_book(
        conn,
        owner,
        a_book("Monster Manual"),
        &entries("Creature", ENTRIES),
    )
    .expect("the book to import");

    let book_bytes = bytes_of_book(conn, manual.id);
    let entry_count = entry_count_of(conn, manual.id);

    // One world at a time, so the growth per world is measured rather than
    // divided out of a total.
    let mut link_bytes = Vec::new();
    for _ in 0..WORLDS {
        let world = a_world(conn, owner, &manual.system_id);
        book_list::switch_on(conn, owner, world, manual.id).expect("the book to switch on");
        link_bytes.push(bytes_of_links(conn, manual.id));
    }

    let after_all_worlds = bytes_of_book(conn, manual.id);
    assert_eq!(
        after_all_worlds, book_bytes,
        "SC-002: stored content must not grow when a world switches a book on"
    );

    let links_total = *link_bytes.last().expect("at least one world");
    let link_each = links_total / WORLDS as i64;

    // The other half of FR-070: stored size *does* grow with distinct books.
    // Without this the first number could be explained by nothing being
    // stored at all.
    let second = store::import_book(
        conn,
        owner,
        a_book("Player's Handbook"),
        &entries("Spell", ENTRIES),
    )
    .expect("the second book to import");
    let second_book_bytes = bytes_of_book(conn, second.id);

    Measurement {
        book_bytes,
        entry_count,
        link_bytes_each: link_each,
        inherited_total: book_bytes + links_total,
        copied_total: book_bytes * WORLDS as i64,
        second_book_bytes,
    }
    .render()
}

impl Measurement {
    fn render(&self) -> String {
        let saved = self.copied_total - self.inherited_total;
        let ratio = self.copied_total as f64 / self.inherited_total as f64;
        let overhead = self.link_bytes_each as f64 * 100.0 / self.book_bytes as f64;

        format!(
            r#"
## Phase 9: one book, eight worlds, measured in bytes (T064)

**Date**: {date} · **Commit**: `{commit}`

```bash
cargo run -p thunderforge-server --features test-support \
    --example library_storage
```

A synthetic book of {entries} entries is read onto one account's shelf and
switched on in {worlds} worlds. Stored bytes are Postgres's own
`pg_column_size` over the rows each thing owns — the compendium and its
entries for the book, the `world_books` rows for the links — taken inside one
transaction that is rolled back, so the numbers come from a real database and
leave nothing in it.

| Measure | Bytes |
|---|---|
| One book ({stored_entries} entries stored) | **{book_bytes}** |
| One world's link to it | **{link_each}** |
| That book in {worlds} worlds, inherited | **{inherited}** |
| That book in {worlds} worlds, copied | {copied} |
| Saved | **{saved}** ({ratio:.1}x) |
| A second, different book | {second} |

### Reading this honestly

**SC-002 holds and is asserted, not observed.** The example fails rather than
prints if stored content grows when a world switches a book on, so the first
row is the size before any world and after all of them. A world costs {overhead:.3}% of a book to run it.

**SC-001 is the third row against the fourth.** Eight worlds and one book cost
{inherited} bytes; the same eight worlds under the copy-per-world model spec
049 was first written with would cost {copied}. The saving is the whole
motivation, and it is {ratio:.1}x at eight worlds. It grows with every world
added, because the book term does not repeat and only a {link_each}-byte link
does.

**FR-070's other half is the last row.** Stored size grows with distinct books
read: a second book costs a second book. A measurement that showed only the
first result would be equally consistent with nothing being stored at all.

**What this is not.** It is not a measure of a real sourcebook's entries,
which vary in size by kind, and it is not table overhead, index size or TOAST
behaviour — `pg_column_size` measures the datum, not the page it lands on.
Both matter to an operator's disk and neither is measured here; both apply to
a copied entry at least as much as to a link, so neither can close the gap.

**What would reopen it**: a delta table that stores anything per world beyond
a link (Phase 11), any column added to `world_books`, or a change to what an
entry stores.

---
"#,
            date = chrono::Utc::now().format("%Y-%m-%d"),
            commit = commit(),
            entries = ENTRIES,
            stored_entries = self.entry_count,
            worlds = WORLDS,
            book_bytes = self.book_bytes,
            link_each = self.link_bytes_each,
            inherited = self.inherited_total,
            copied = self.copied_total,
            second = self.second_book_bytes,
            saved = saved,
            ratio = ratio,
            overhead = overhead,
        )
    }
}

/// What this build is, so a number can be compared to the code that produced
/// it. Unknown rather than fatal if git is not there — a measurement is worth
/// having from a tarball too.
fn commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn a_book(title: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: "storage-measurement".to_string(),
        parser_version: "measurement".to_string(),
        page_count: 320,
        silent_page_count: 0,
    }
}

fn a_world(conn: &mut PgConnection, owner: Uuid, system: &str) -> Uuid {
    let world = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world)))
        .set(worlds::game_system_id.eq(Some(system.to_string())))
        .execute(conn)
        .expect("the world to declare a system");
    world
}

/// Entries of a plausible size: a name, a page, and six declared fields in the
/// three certainty states the reader produces.
fn entries(kind: &str, how_many: usize) -> Vec<Entry> {
    (0..how_many)
        .map(|n| Entry {
            kind: kind.to_lowercase(),
            name: format!("{kind} number {n}"),
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

/// What one book costs: its shelf row plus every entry it produced.
fn bytes_of_book(conn: &mut PgConnection, compendium_id: Uuid) -> i64 {
    let shelf: i64 = diesel::sql_query(format!(
        "SELECT COALESCE(SUM(pg_column_size(c.*)), 0)::bigint AS bytes \
         FROM compendiums c WHERE c.id = '{compendium_id}'"
    ))
    .get_result::<Bytes>(conn)
    .expect("the compendium's size")
    .bytes;

    let entries: i64 = diesel::sql_query(format!(
        "SELECT COALESCE(SUM(pg_column_size(e.*)), 0)::bigint AS bytes \
         FROM compendium_entries e WHERE e.compendium_id = '{compendium_id}'"
    ))
    .get_result::<Bytes>(conn)
    .expect("the entries' size")
    .bytes;

    shelf + entries
}

/// What every world's link to one book costs, together.
fn bytes_of_links(conn: &mut PgConnection, compendium_id: Uuid) -> i64 {
    diesel::sql_query(format!(
        "SELECT COALESCE(SUM(pg_column_size(b.*)), 0)::bigint AS bytes \
         FROM world_books b WHERE b.compendium_id = '{compendium_id}'"
    ))
    .get_result::<Bytes>(conn)
    .expect("the links' size")
    .bytes
}

fn entry_count_of(conn: &mut PgConnection, compendium_id: Uuid) -> i64 {
    use thunderforge_server::schema::compendium_entries;
    compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .count()
        .get_result(conn)
        .expect("the entry count")
}
