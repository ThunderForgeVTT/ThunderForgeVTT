use super::*;
use crate::lore_sync::document::DocumentHeader;
use crate::test_support::{
    insert_test_lore_entry, insert_test_user, insert_test_world, test_app_state,
};
use chrono::{TimeZone, Utc};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// A connection row. The repository reference is unique per run because
/// FR-033's constraint is instance-wide: a fixed name passes once and fails
/// for the rest of the database's life.
fn connection(world_id: Uuid, owner: Uuid, incoming_enabled: bool) -> LoreRepositoryConnection {
    let now = Utc::now().naive_utc();
    LoreRepositoryConnection {
        id: Uuid::now_v7(),
        world_id,
        host_kind: "test".to_string(),
        installation_ref: "test-installation".to_string(),
        repository_ref: format!("owner/{}", Uuid::now_v7()),
        branch: "main".to_string(),
        directory: format!("lore-{}", Uuid::now_v7().simple()),
        incoming_enabled,
        notice_acknowledged_at: Some(now),
        state: "working".to_string(),
        state_reason: None,
        repository_is_public: Some(false),
        visibility_checked_at: Some(now),
        deactivated_at: None,
        deactivated_reason: None,
        last_synced_at: Some(now),
        last_written_commit: None,
        created_by: owner,
        updated_by: owner,
        created_at: now,
        updated_at: now,
    }
}

fn exported(
    lore_entry_id: Uuid,
    title: &str,
    path: &str,
    body: &str,
    revision: Option<Uuid>,
) -> ExportedEntry {
    ExportedEntry {
        lore_entry_id,
        title: title.to_string(),
        current_path: path.to_string(),
        exported_revision_id: revision,
        exported_body: body.to_string(),
        current_revision_id: revision,
        current_body: body.to_string(),
    }
}

/// A file as export would have written it.
fn file_for(entry: &ExportedEntry, id: Uuid, body: &str) -> RepositoryFile {
    let header = DocumentHeader {
        id,
        title: entry.title.clone(),
        tags: Vec::new(),
        updated: Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        unresolvable_links: Vec::new(),
    };
    RepositoryFile {
        path: entry.current_path.clone(),
        contents: document::render(&header, body),
    }
}

fn gate_for(connection: &LoreRepositoryConnection) -> IncomingEnabled {
    IncomingEnabled::for_connection(connection).expect("incoming acceptance is enabled")
}

// ---------------------------------------------------------------------
// FR-022 — the rule that keeps every existing world as safe as yesterday
// ---------------------------------------------------------------------

/// FR-022, and the most important test in this file.
///
/// A world that never enabled incoming acceptance must never be modified by
/// anything in its repository. This asserts the gate refuses, but the real
/// guarantee is stronger and is not expressible as an assertion: `detect`,
/// `record`, `accept`, `decline` and `pending` all take an
/// `&IncomingEnabled`, whose fields are private and whose only constructor
/// is the one exercised here. There is no call, in this crate or any
/// future one, that reaches a lore write for such a world — not because
/// every path remembers to check, but because no such path compiles.
#[test]
fn a_connection_without_incoming_enabled_yields_no_gate() {
    let world = Uuid::now_v7();
    let owner = Uuid::now_v7();
    let connection = connection(world, owner, false);

    assert!(
        IncomingEnabled::for_connection(&connection).is_none(),
        "a world that never enabled incoming acceptance produced a gate — FR-022 is broken",
    );
}

/// FR-041a. An enforcement deactivation a repository could still write
/// through would not be a deactivation.
#[test]
fn a_deactivated_connection_yields_no_gate() {
    let mut connection = connection(Uuid::now_v7(), Uuid::now_v7(), true);
    connection.state = "deactivated".to_string();
    connection.deactivated_at = Some(Utc::now().naive_utc());

    assert!(
        IncomingEnabled::for_connection(&connection).is_none(),
        "a deactivated connection produced a gate",
    );
}

/// The other half: a world that DID enable it gets a gate naming the right
/// connection and world. A gate that never opens would pass the test above
/// and make the feature dead.
#[test]
fn an_enabled_connection_yields_a_gate_naming_its_world() {
    let world = Uuid::now_v7();
    let connection = connection(world, Uuid::now_v7(), true);
    let gate = gate_for(&connection);

    assert_eq!(gate.connection_id(), connection.id);
    assert_eq!(gate.world_id(), world);
}

// ---------------------------------------------------------------------
// Detection — pure, no database, no clone
// ---------------------------------------------------------------------

/// SC-008's round trip, from detection's side: export a file, change
/// nothing, and there is nothing to propose. A detector that proposed the
/// world's own text back at it would bury real changes in noise.
#[test]
fn an_unchanged_file_proposes_nothing() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let entry = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "A castle above the bay.",
        Some(Uuid::now_v7()),
    );
    let file = file_for(&entry, entry.lore_entry_id, &entry.exported_body);

    assert_eq!(
        detect(
            &gate,
            &[file],
            &[entry],
            &WithheldByModeration::exactly(&[])
        ),
        Vec::new()
    );
}

/// FR-027 and T058. A file a Game Master wrote in their editor carries no
/// front matter at all, so it is offered as a new entry — and its text is
/// carried whole, including whatever it has at the top, because a parser
/// that could not read a header has no standing to delete it.
#[test]
fn a_file_with_no_front_matter_is_a_proposed_new_entry() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let file = RepositoryFile {
        path: "westeros/the-salt-road.md".to_string(),
        contents: "# The Salt Road\n\nIt runs east.".to_string(),
    };

    assert_eq!(
        detect(&gate, &[file], &[], &WithheldByModeration::exactly(&[])),
        vec![DetectedChange::NewEntry {
            repository_path: "westeros/the-salt-road.md".to_string(),
            proposed_title: "the-salt-road".to_string(),
            incoming_body: "# The Salt Road\n\nIt runs east.".to_string(),
        }],
    );
}

/// FR-027, at its sharpest. The file sits at exactly the path of an
/// existing entry and carries exactly its title, and still is not matched
/// to it — because the identifier in its header is not one this world
/// knows.
///
/// The existing entry is reported as a proposed deletion in the same pass,
/// which is the honest reading: from the repository's side its file is
/// gone and a different file has taken the name. Both are proposals, and a
/// human sees both.
#[test]
fn a_file_is_never_matched_to_an_entry_by_path_or_title() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let entry = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "A castle above the bay.",
        Some(Uuid::now_v7()),
    );
    // Same path, same title, a stranger's id.
    let file = file_for(&entry, Uuid::now_v7(), "Somebody else's castle.");

    let changes = detect(
        &gate,
        &[file],
        std::slice::from_ref(&entry),
        &WithheldByModeration::exactly(&[]),
    );

    assert!(
        !changes
            .iter()
            .any(|change| matches!(change, DetectedChange::Update { .. })),
        "a file was matched to an entry by path or title — FR-027 is broken: {changes:?}",
    );
    assert!(changes.contains(&DetectedChange::NewEntry {
        repository_path: "westeros/the-red-keep.md".to_string(),
        proposed_title: "The Red Keep".to_string(),
        incoming_body: "Somebody else's castle.".to_string(),
    }));
    assert!(changes.contains(&DetectedChange::Deletion {
        lore_entry_id: entry.lore_entry_id,
        repository_path: "westeros/the-red-keep.md".to_string(),
    }));
}

/// FR-024. Both sides moved, so the change is flagged as such — and the
/// proposal carries the repository's text WHOLE, with no trace of the
/// app's. There is nothing here that could have merged them, which is the
/// point: the reviewer is choosing between two texts, not reviewing a
/// third that nobody wrote.
#[test]
fn an_entry_changed_on_both_sides_is_presented_whole_and_never_merged() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let base_revision = Uuid::now_v7();
    let app_revision = Uuid::now_v7();
    let mut entry = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "A castle above the bay.",
        Some(base_revision),
    );
    // The app moved on.
    entry.current_revision_id = Some(app_revision);
    entry.current_body = "A castle above the bay, and a sept below.".to_string();

    let file = file_for(
        &entry,
        entry.lore_entry_id,
        "A castle above the bay, built by Maegor.",
    );

    let changes = detect(
        &gate,
        &[file],
        std::slice::from_ref(&entry),
        &WithheldByModeration::exactly(&[]),
    );

    assert_eq!(
        changes,
        vec![DetectedChange::Update {
            lore_entry_id: entry.lore_entry_id,
            repository_path: "westeros/the-red-keep.md".to_string(),
            incoming_body: "A castle above the bay, built by Maegor.".to_string(),
            base_revision_id: Some(base_revision),
            app_revision_id: Some(app_revision),
            also_changed_in_app: true,
        }],
    );

    let DetectedChange::Update { incoming_body, .. } = &changes[0] else {
        unreachable!("asserted above");
    };
    assert!(
        !incoming_body.contains("sept"),
        "the proposal carried text from the app as well as the repository — \
         something merged prose, which FR-024 forbids at any priority",
    );
}

/// The other side of FR-024: a change only the repository made is not
/// dressed up as a conflict. Everything being a conflict trains a reviewer
/// to stop reading the warning.
#[test]
fn a_change_on_one_side_only_is_not_flagged_as_a_conflict() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let entry = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "A castle above the bay.",
        Some(Uuid::now_v7()),
    );
    let file = file_for(&entry, entry.lore_entry_id, "A castle, and a bay.");

    let changes = detect(
        &gate,
        &[file],
        &[entry],
        &WithheldByModeration::exactly(&[]),
    );
    let DetectedChange::Update {
        also_changed_in_app,
        ..
    } = &changes[0]
    else {
        panic!("expected an update, got {changes:?}");
    };
    assert!(!also_changed_in_app);
}

/// FR-026. A file that is gone produces a PROPOSAL and nothing else. There
/// is no variant of `DetectedChange` that deletes, and detection has no
/// database handle with which to.
#[test]
fn a_missing_file_is_a_proposed_deletion_and_nothing_more() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let entry = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "A castle above the bay.",
        Some(Uuid::now_v7()),
    );

    assert_eq!(
        detect(
            &gate,
            &[],
            std::slice::from_ref(&entry),
            &WithheldByModeration::exactly(&[])
        ),
        vec![DetectedChange::Deletion {
            lore_entry_id: entry.lore_entry_id,
            repository_path: "westeros/the-red-keep.md".to_string(),
        }],
    );
}

/// FR-015, and the reason detection is told what export is withholding.
///
/// An entry a takedown disabled is absent from the repository because we
/// removed its file. Without this rule that absence reads as a proposed
/// deletion, and worse, a stale clone pushing the file back would offer
/// the disabled text as a NEW entry — laundering removed content into the
/// world under a fresh identifier.
#[test]
fn a_withheld_entry_is_invisible_in_both_directions() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let entry = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "A castle above the bay.",
        Some(Uuid::now_v7()),
    );
    let resurrected = file_for(&entry, entry.lore_entry_id, "The text a takedown removed.");
    let withheld = WithheldByModeration::exactly(&[entry.lore_entry_id]);

    assert_eq!(
        detect(
            &gate,
            &[resurrected],
            std::slice::from_ref(&entry),
            &withheld
        ),
        Vec::new(),
        "a moderation-disabled entry was offered back",
    );
    assert_eq!(
        detect(&gate, &[], &[entry], &withheld),
        Vec::new(),
        "our own withholding of a file was reported as the user deleting it",
    );
}

/// A repository link comes back as the app's own link syntax, so that
/// accepting an untouched file changes nothing (SC-008) and accepting an
/// edited one does not silently break the entry's cross-links.
#[test]
fn repository_links_come_back_in_authored_form() {
    let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
    let keep = exported(
        Uuid::now_v7(),
        "The Red Keep",
        "westeros/the-red-keep.md",
        "See [[Blackwater Bay]].",
        Some(Uuid::now_v7()),
    );
    let bay = exported(
        Uuid::now_v7(),
        "Blackwater Bay",
        "westeros/blackwater-bay.md",
        "Water.",
        Some(Uuid::now_v7()),
    );

    let destination = paths::relative_link(&keep.current_path, &bay.current_path);
    let file = RepositoryFile {
        path: keep.current_path.clone(),
        contents: document::render(
            &DocumentHeader {
                id: keep.lore_entry_id,
                title: keep.title.clone(),
                tags: Vec::new(),
                updated: Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
                unresolvable_links: Vec::new(),
            },
            &format!("Sail to [Blackwater Bay]({destination}) at dawn."),
        ),
    };

    let changes = detect(
        &gate,
        &[file],
        &[keep.clone(), bay],
        &WithheldByModeration::exactly(&[]),
    );
    let DetectedChange::Update { incoming_body, .. } = &changes[0] else {
        panic!("expected an update, got {changes:?}");
    };
    assert_eq!(incoming_body, "Sail to [[Blackwater Bay]] at dawn.");
}

// ---------------------------------------------------------------------
// Recording, accepting and declining — against the database
// ---------------------------------------------------------------------

struct Fixture {
    conn: diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>,
    user: Uuid,
    world: Uuid,
    gate: IncomingEnabled,
}

fn fixture(incoming_enabled: bool) -> Fixture {
    let mut conn = test_app_state().db_pool.get().expect("a connection");
    let user = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, user);
    let row = connection(world, user, incoming_enabled);
    diesel::insert_into(crate::schema::lore_repository_connections::table)
        .values(row.clone())
        .execute(&mut conn)
        .expect("the connection is accepted");
    let gate = gate_for(&row);
    Fixture {
        conn,
        user,
        world,
        gate,
    }
}

fn entry_content(conn: &mut PgConnection, entry_id: Uuid) -> String {
    world_lore_entries::table
        .filter(world_lore_entries::id.eq(entry_id))
        .select(world_lore_entries::content)
        .first(conn)
        .expect("the entry still exists")
}

fn revision_count(conn: &mut PgConnection, entry_id: Uuid) -> i64 {
    world_lore_revisions::table
        .filter(world_lore_revisions::lore_entry_id.eq(entry_id))
        .count()
        .get_result(conn)
        .expect("counted")
}

/// FR-023, User Story 3 acceptance scenario 1. Detection plus recording
/// leaves the world exactly as it was: the entry's text is untouched, no
/// revision has appeared, and the only thing that changed anywhere is a row
/// saying somebody should look.
#[test]
fn detection_records_a_proposal_and_alters_no_lore() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);

    let change = DetectedChange::Update {
        lore_entry_id: entry_id,
        repository_path: "lore/entry.md".to_string(),
        incoming_body: "Text from the repository.".to_string(),
        base_revision_id: None,
        app_revision_id: None,
        also_changed_in_app: false,
    };
    record(&mut f.conn, &f.gate, &[change]).expect("recorded");

    assert_eq!(entry_content(&mut f.conn, entry_id), "");
    assert_eq!(revision_count(&mut f.conn, entry_id), 0);
    assert_eq!(
        pending(&mut f.conn, &f.gate).expect("listed").len(),
        1,
        "the proposal was not recorded, so nobody would ever be asked",
    );
}

/// FR-025 and T057. Accepting writes an ORDINARY revision — same table as
/// every revision a person typed — authored by the accepting user, and the
/// proposal points at it, which is what makes the revision identifiable as
/// having come from the repository.
#[test]
fn accepting_writes_an_ordinary_revision_attributed_to_the_accepting_user() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let accepter = insert_test_user(&mut f.conn);

    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "Text from the repository.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        }],
    )
    .expect("recorded");

    let outcome = accept(&mut f.conn, &f.gate, ids[0], accepter).expect("accepted");
    let Acceptance::Updated {
        lore_entry_id,
        revision_id,
    } = outcome
    else {
        panic!("expected an update, got {outcome:?}");
    };
    assert_eq!(lore_entry_id, entry_id);

    assert_eq!(
        entry_content(&mut f.conn, entry_id),
        "Text from the repository.",
    );

    let (author, restored_from, content): (Uuid, Option<Uuid>, String) =
        world_lore_revisions::table
            .filter(world_lore_revisions::id.eq(revision_id))
            .select((
                world_lore_revisions::author_id,
                world_lore_revisions::restored_from_revision_id,
                world_lore_revisions::content_markdown,
            ))
            .first(&mut f.conn)
            .expect("the revision exists");
    assert_eq!(
        author, accepter,
        "the revision was not attributed to the accepting user"
    );
    assert_eq!(content, "Text from the repository.");
    assert_eq!(
        restored_from, None,
        "an incoming change was recorded as a restore, which the history does not mean",
    );

    let current: Option<Uuid> = world_lore_entries::table
        .filter(world_lore_entries::id.eq(entry_id))
        .select(world_lore_entries::current_revision_id)
        .first(&mut f.conn)
        .expect("the entry exists");
    assert_eq!(current, Some(revision_id));

    assert!(
        is_repository_originated(&mut f.conn, revision_id).expect("checked"),
        "the revision is not identifiable as originating from the repository — FR-025",
    );
}

/// FR-023 and FR-024 together. Declining a conflicted change leaves the
/// entry byte for byte as the app had it — no revision, no partial write,
/// no merge.
#[test]
fn declining_leaves_the_entry_byte_for_byte() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    diesel::update(world_lore_entries::table.filter(world_lore_entries::id.eq(entry_id)))
        .set(world_lore_entries::content.eq("What the app says."))
        .execute(&mut f.conn)
        .expect("set up the app's text");

    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "What the repository says.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: true,
        }],
    )
    .expect("recorded");

    decline(&mut f.conn, &f.gate, ids[0], f.user).expect("declined");

    assert_eq!(entry_content(&mut f.conn, entry_id), "What the app says.");
    assert_eq!(revision_count(&mut f.conn, entry_id), 0);
    assert!(pending(&mut f.conn, &f.gate).expect("listed").is_empty());
}

/// One acceptance per proposal. Two reviewers pressing accept, or a retried
/// request, must not write the same text twice into an entry's history.
#[test]
fn a_change_cannot_be_accepted_twice() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "Once.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        }],
    )
    .expect("recorded");

    accept(&mut f.conn, &f.gate, ids[0], f.user).expect("accepted");
    let second = accept(&mut f.conn, &f.gate, ids[0], f.user);

    assert!(matches!(second, Err(IncomingError::NotPending)));
    assert_eq!(revision_count(&mut f.conn, entry_id), 1);
}

/// A proposal belongs to the connection that detected it. An id from
/// another world's review surface finds nothing here, so authority over one
/// world never becomes authority over another's lore.
#[test]
fn a_change_from_another_connection_is_not_acceptable() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "Not yours.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        }],
    )
    .expect("recorded");

    let other_world = insert_test_world(&mut f.conn, f.user);
    let other_row = connection(other_world, f.user, true);
    diesel::insert_into(crate::schema::lore_repository_connections::table)
        .values(other_row.clone())
        .execute(&mut f.conn)
        .expect("a second connection");
    let other_gate = gate_for(&other_row);

    assert!(matches!(
        accept(&mut f.conn, &other_gate, ids[0], f.user),
        Err(IncomingError::NotPending),
    ));
    assert!(matches!(
        decline(&mut f.conn, &other_gate, ids[0], f.user),
        Err(IncomingError::NotPending),
    ));
    assert_eq!(entry_content(&mut f.conn, entry_id), "");
}

/// FR-026. Confirming a deletion records the confirmation and deletes
/// nothing — the entry is still there, and the caller is handed the
/// obligation to remove it through the world's own deletion path, which
/// knows about children, permissions and images.
#[test]
fn accepting_a_deletion_confirms_it_and_deletes_nothing() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::Deletion {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
        }],
    )
    .expect("recorded");

    assert_eq!(
        accept(&mut f.conn, &f.gate, ids[0], f.user).expect("confirmed"),
        Acceptance::DeletionConfirmed {
            lore_entry_id: entry_id
        },
    );

    let still_there: i64 = world_lore_entries::table
        .filter(world_lore_entries::id.eq(entry_id))
        .count()
        .get_result(&mut f.conn)
        .expect("counted");
    assert_eq!(
        still_there, 1,
        "confirming a deletion deleted the entry from inside lore_sync",
    );
}

/// FR-026's second half: a declined deletion is reversed on the next
/// synchronisation.
///
/// Exercised as what actually happens rather than as an intention — the
/// ordinary export pass, given the exported-entry record and a plan that
/// still contains the entry, writes the file back into a subtree it is
/// missing from. Nothing in this module is involved, which is the claim:
/// declining needs no restoration code because the entry never left.
#[test]
fn a_declined_deletion_is_restored_by_the_next_export_pass() {
    use crate::lore_sync::apply;
    use crate::lore_sync::plan::{Plan, PlannedFile};

    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::Deletion {
            lore_entry_id: entry_id,
            repository_path: "westeros/the-red-keep.md".to_string(),
        }],
    )
    .expect("recorded");
    decline(&mut f.conn, &f.gate, ids[0], f.user).expect("declined");

    let subtree = std::env::temp_dir().join(format!("tf-incoming-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&subtree).expect("a subtree");

    let plan = Plan {
        files: vec![PlannedFile {
            entry_id,
            path: "westeros/the-red-keep.md".to_string(),
            contents: "A castle above the bay.".to_string(),
        }],
        images: Vec::new(),
        notes: Vec::new(),
    };
    let previously_written = HashMap::from([(entry_id, "westeros/the-red-keep.md".to_string())]);

    apply::apply(&subtree, &plan, &previously_written, &|_| None).expect("the pass applies");

    assert_eq!(
        std::fs::read_to_string(subtree.join("westeros/the-red-keep.md"))
            .expect("the file was restored"),
        "A castle above the bay.",
    );
    std::fs::remove_dir_all(&subtree).ok();
}

/// T058 and FR-027, through to acceptance. A proposal that matched nothing
/// creates a NEW entry with a new identifier, and the entry that happened
/// to share its path and title is untouched.
#[test]
fn accepting_a_proposed_new_entry_creates_one_and_touches_no_existing_entry() {
    let mut f = fixture(true);
    let existing = insert_test_lore_entry(&mut f.conn, f.world, f.user);

    let ids = record(
        &mut f.conn,
        &f.gate,
        &[DetectedChange::NewEntry {
            repository_path: "westeros/the-salt-road.md".to_string(),
            proposed_title: "The Salt Road".to_string(),
            incoming_body: "It runs east.".to_string(),
        }],
    )
    .expect("recorded");

    let outcome = accept(&mut f.conn, &f.gate, ids[0], f.user).expect("accepted");
    let Acceptance::Created {
        lore_entry_id,
        revision_id,
    } = outcome
    else {
        panic!("expected a creation, got {outcome:?}");
    };

    assert_ne!(lore_entry_id, existing);
    assert_eq!(entry_content(&mut f.conn, existing), "");
    assert_eq!(revision_count(&mut f.conn, existing), 0);

    let (title, world_id, content): (String, Uuid, String) = world_lore_entries::table
        .filter(world_lore_entries::id.eq(lore_entry_id))
        .select((
            world_lore_entries::title,
            world_lore_entries::world_id,
            world_lore_entries::content,
        ))
        .first(&mut f.conn)
        .expect("the new entry exists");
    assert_eq!(title, "The Salt Road");
    assert_eq!(world_id, f.world);
    assert_eq!(content, "It runs east.");
    assert!(is_repository_originated(&mut f.conn, revision_id).expect("checked"));

    let created: Option<Uuid> = lore_pending_incoming_changes::table
        .filter(lore_pending_incoming_changes::id.eq(ids[0]))
        .select(lore_pending_incoming_changes::created_entry_id)
        .first(&mut f.conn)
        .expect("the proposal exists");
    assert_eq!(
        created,
        Some(lore_entry_id),
        "the proposal does not record which entry it created, so FR-027's \
         'never matched to an existing entry' cannot be audited afterwards",
    );

    let matched: Option<Uuid> = lore_pending_incoming_changes::table
        .filter(lore_pending_incoming_changes::id.eq(ids[0]))
        .select(lore_pending_incoming_changes::lore_entry_id)
        .first(&mut f.conn)
        .expect("the proposal exists");
    assert_eq!(
        matched, None,
        "a proposed new entry ended up naming an entry"
    );
}

/// A repeated polling pass observing the same divergence updates the
/// existing proposal rather than stacking a second one. Two accept buttons
/// for one entry means pressing both writes the older text last.
#[test]
fn a_second_detection_pass_does_not_stack_a_second_proposal() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let change = |body: &str| DetectedChange::Update {
        lore_entry_id: entry_id,
        repository_path: "lore/entry.md".to_string(),
        incoming_body: body.to_string(),
        base_revision_id: None,
        app_revision_id: None,
        also_changed_in_app: false,
    };

    let first = record(&mut f.conn, &f.gate, &[change("First.")]).expect("recorded");
    let second = record(&mut f.conn, &f.gate, &[change("Second.")]).expect("recorded again");

    assert_eq!(first, second, "a second pass created a second proposal");
    let rows = pending(&mut f.conn, &f.gate).expect("listed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].incoming_body.as_deref(), Some("Second."));
}

// ---------------------------------------------------------------------
// The schema's own guarantees
// ---------------------------------------------------------------------

/// FR-023 in the database. A row claiming to have written a revision
/// without being accepted would mean lore had changed with nobody agreeing
/// to it — the one thing this story must not allow — so the constraint is
/// exercised rather than assumed.
#[test]
fn an_applied_revision_cannot_be_recorded_without_an_acceptance() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let revision_id = write_revision(
        &mut f.conn,
        entry_id,
        "Text.",
        f.user,
        Utc::now().naive_utc(),
    )
    .expect("a revision");

    let mut row = row_for(
        &f.gate,
        &DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "Text.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        },
        Utc::now().naive_utc(),
    );
    row.applied_revision_id = Some(revision_id);

    assert!(
        diesel::insert_into(lore_pending_incoming_changes::table)
            .values(row)
            .execute(&mut f.conn)
            .is_err(),
        "a pending row claimed to have applied a revision",
    );
}

/// A decision with no decider and no time is not a decision. Without this
/// constraint "accepted by nobody" is representable and the audit trail
/// FR-023 rests on has a hole in it.
#[test]
fn a_decision_without_a_decider_is_refused() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let mut row = row_for(
        &f.gate,
        &DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "Text.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        },
        Utc::now().naive_utc(),
    );
    row.status = STATUS_ACCEPTED.to_string();

    assert!(
        diesel::insert_into(lore_pending_incoming_changes::table)
            .values(row)
            .execute(&mut f.conn)
            .is_err(),
        "a change was accepted by nobody at no time",
    );
}

/// FR-027 in the database. A proposal for a new entry that also names an
/// existing one is the exact shape of the mistake FR-027 forbids, and it is
/// refused by the schema rather than by whichever code path remembered.
#[test]
fn a_new_entry_proposal_cannot_name_an_existing_entry() {
    let mut f = fixture(true);
    let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
    let mut row = row_for(
        &f.gate,
        &DetectedChange::NewEntry {
            repository_path: "westeros/the-salt-road.md".to_string(),
            proposed_title: "The Salt Road".to_string(),
            incoming_body: "It runs east.".to_string(),
        },
        Utc::now().naive_utc(),
    );
    row.lore_entry_id = Some(entry_id);

    assert!(
        diesel::insert_into(lore_pending_incoming_changes::table)
            .values(row)
            .execute(&mut f.conn)
            .is_err(),
        "a file was allowed to be a new entry and an existing entry at once",
    );
}
