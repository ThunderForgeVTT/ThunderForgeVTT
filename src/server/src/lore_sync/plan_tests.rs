//! What a world says its repository should contain, asserted against a real
//! world rather than a mock.
//!
//! These need a database because the plan is a statement about a world's
//! actual rows — its tree, its tags, its moderation state. What they do *not*
//! need is a remote, a clone, a credential, or a repository host, which is the
//! whole reason planning is a separate module from the pass that applies it.

use diesel::prelude::*;
use uuid::Uuid;

use super::*;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn insert_entry(
    conn: &mut PgConnection,
    world_id: Uuid,
    created_by: Uuid,
    title: &str,
    content: &str,
    parent: Option<Uuid>,
) -> Uuid {
    use crate::schema::world_lore_entries;
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_lore_entries::table)
        .values((
            world_lore_entries::id.eq(id),
            world_lore_entries::world_id.eq(world_id),
            world_lore_entries::title.eq(title),
            world_lore_entries::slug.eq(format!("s-{}", id.simple())),
            world_lore_entries::content.eq(content),
            world_lore_entries::created_by.eq(created_by),
            world_lore_entries::created_at.eq(now),
            world_lore_entries::updated_at.eq(now),
            world_lore_entries::parent_id.eq(parent),
        ))
        .execute(conn)
        .expect("insert lore entry");
    id
}

fn disable_entry(conn: &mut PgConnection, world_id: Uuid, entry_id: Uuid) {
    use crate::schema::content_moderation_actions;
    diesel::insert_into(content_moderation_actions::table)
        .values((
            content_moderation_actions::id.eq(Uuid::now_v7()),
            content_moderation_actions::case_id.eq(Uuid::now_v7()),
            content_moderation_actions::action_type.eq("content_disabled"),
            content_moderation_actions::entity_type.eq("lore_entry"),
            content_moderation_actions::entity_id.eq(entry_id),
            content_moderation_actions::world_id.eq(world_id),
            content_moderation_actions::claimant_name.eq("A Claimant"),
            content_moderation_actions::claimant_contact.eq("claimant@example.test"),
            content_moderation_actions::copyrighted_work_description.eq("A work"),
            content_moderation_actions::infringing_material_location.eq("somewhere"),
            content_moderation_actions::good_faith_statement.eq(true),
            content_moderation_actions::accuracy_statement.eq(true),
            content_moderation_actions::signature.eq("A Claimant"),
        ))
        .execute(conn)
        .expect("insert moderation action");
}

fn world_with_owner() -> (crate::AppState, Uuid, Uuid) {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    (state, world, owner)
}

/// FR-007 and FR-008: one file per entry, in directories mirroring the tree.
#[tokio::test]
async fn the_tree_becomes_directories_and_one_file_per_entry() {
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");

    let parent = insert_entry(&mut conn, world, owner, "Westeros", "A continent.", None);
    insert_entry(
        &mut conn,
        world,
        owner,
        "The Red Keep",
        "A castle.",
        Some(parent),
    );

    let plan = plan_world(&state, world).await.expect("a plan");

    assert_eq!(plan.files.len(), 2, "expected one file per entry");
    let paths: Vec<&str> = plan.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.iter().any(|p| p.contains("westeros")));
    assert!(
        paths.iter().any(|p| p.contains("westeros/")),
        "the child did not land under its parent: {paths:?}",
    );
}

/// FR-015 and SC-009. The interesting half is the second assertion: the rest
/// of the world must still synchronise, so this filters rather than fails.
#[tokio::test]
async fn a_disabled_entry_is_absent_and_does_not_stop_the_others() {
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");

    let kept = insert_entry(&mut conn, world, owner, "Kept Entry", "Fine.", None);
    let disabled = insert_entry(&mut conn, world, owner, "Taken Down", "Not fine.", None);
    disable_entry(&mut conn, world, disabled);

    let plan = plan_world(&state, world).await.expect("a plan");

    let ids: Vec<Uuid> = plan.files.iter().map(|f| f.entry_id).collect();
    assert!(
        ids.contains(&kept),
        "an unrelated entry stopped synchronising"
    );
    assert!(!ids.contains(&disabled), "a disabled entry was mirrored");
}

/// The ordering that matters. Assigning paths first and filtering afterwards
/// would leave a survivor carrying a disambiguation suffix earned against a
/// sibling that is no longer there — a takedown on one entry renaming another,
/// producing a commit for an entry nobody touched.
#[tokio::test]
async fn disabling_a_colliding_sibling_does_not_rename_the_survivor() {
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");

    let survivor = insert_entry(&mut conn, world, owner, "The Keep", "One.", None);
    let doomed = insert_entry(&mut conn, world, owner, "the keep", "Two.", None);

    let before = plan_world(&state, world).await.expect("a plan");
    let survivor_path_before = before
        .files
        .iter()
        .find(|f| f.entry_id == survivor)
        .expect("survivor present")
        .path
        .clone();

    disable_entry(&mut conn, world, doomed);

    let after = plan_world(&state, world).await.expect("a plan");
    let survivor_path_after = &after
        .files
        .iter()
        .find(|f| f.entry_id == survivor)
        .expect("survivor still present")
        .path;

    assert_eq!(
        &survivor_path_before, survivor_path_after,
        "a takedown on one entry renamed another",
    );
}

/// Two passes over an unchanged world must produce byte-identical plans, or a
/// pass commits on every run and the history becomes noise.
#[tokio::test]
async fn planning_twice_produces_the_same_thing() {
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");
    insert_entry(&mut conn, world, owner, "Stable", "Unchanged.", None);
    insert_entry(
        &mut conn,
        world,
        owner,
        "Also Stable",
        "Unchanged too.",
        None,
    );

    let first = plan_world(&state, world).await.expect("a plan");
    let second = plan_world(&state, world).await.expect("a plan");

    let render = |p: &Plan| {
        p.files
            .iter()
            .map(|f| format!("{}\n{}", f.path, f.contents))
            .collect::<Vec<_>>()
            .join("\n---\n")
    };
    assert_eq!(render(&first), render(&second));
}

/// FR-011: the body is the entry's markdown as authored. A plan that
/// reformatted would fail SC-008's round trip, but it would fail it far from
/// here — this catches it at the source.
#[tokio::test]
async fn the_body_is_not_reformatted() {
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");
    let awkward = "Heading\n=======\n\n*  ragged bullet\n+   another\n\n\n\ntrailing   ";
    insert_entry(&mut conn, world, owner, "Awkward", awkward, None);

    let plan = plan_world(&state, world).await.expect("a plan");
    let file = &plan.files[0];

    assert!(
        file.contents.ends_with(awkward),
        "the body was altered on the way out",
    );
}

/// FR-014: the uploaded original only. A derived rendition in the repository
/// would multiply an image-heavy world's clone for nothing a reader notices.
#[tokio::test]
async fn only_the_uploaded_original_is_planned_for_an_image() {
    use crate::schema::world_lore_image_assets;
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");
    let entry = insert_entry(&mut conn, world, owner, "Illustrated", "See below.", None);

    let asset = Uuid::now_v7();
    diesel::insert_into(world_lore_image_assets::table)
        .values((
            world_lore_image_assets::id.eq(asset),
            world_lore_image_assets::lore_entry_id.eq(entry),
            world_lore_image_assets::uploaded_by.eq(owner),
            world_lore_image_assets::content_type.eq("image/webp"),
            world_lore_image_assets::byte_size.eq(1024i64),
        ))
        .execute(&mut conn)
        .expect("insert image asset");

    let plan = plan_world(&state, world).await.expect("a plan");

    assert_eq!(plan.images.len(), 1);
    assert_eq!(plan.images[0].path, format!("{IMAGE_DIR}/{asset}.webp"));
    assert!(
        !plan.images[0].object_key.contains("thumb"),
        "a derived rendition was planned",
    );
}

/// FR-013: a link the app resolves to something that is not lore stays
/// readable and is recorded. Silently dropping it, or turning it into a
/// broken lore link, are the two failures this rules out.
///
/// The actor has to actually exist. An earlier version of this test wrote the
/// link without creating one and asserted a note anyway — and the code was
/// right to refuse it, for the reason the next test states.
#[tokio::test]
async fn a_non_lore_link_is_recorded_rather_than_dropped() {
    use crate::schema::world_actors;
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");

    let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
    let actor = crate::test_support::insert_test_actor(&mut conn, world, scene, owner);
    diesel::update(world_actors::table.find(actor))
        .set(world_actors::label.eq("Ser Willem"))
        .execute(&mut conn)
        .expect("name the actor");

    insert_entry(
        &mut conn,
        world,
        owner,
        "Court",
        "Attended by [[Ser Willem]].",
        None,
    );

    let plan = plan_world(&state, world).await.expect("a plan");

    assert!(
        plan.files[0].contents.contains("Ser Willem"),
        "the link text was dropped from the body",
    );
    assert!(
        plan.notes
            .iter()
            .any(|n| n.kind == "unresolvable_cross_link"),
        "no fidelity note recorded the loss",
    );
}

/// The other side of FR-013, and the reason the test above needs a real actor.
///
/// A `[[link]]` that resolves to nothing in the app has lost no fidelity by
/// being mirrored — it was already going nowhere. Recording a note for it
/// would assert a loss that did not occur, and would fill a Game Master's
/// fidelity list with their own typos, which is how a list meant to be read
/// stops being read.
#[tokio::test]
async fn a_link_that_resolves_to_nothing_is_not_reported_as_a_loss() {
    let (state, world, owner) = world_with_owner();
    let mut conn = state.db_pool.get().expect("a connection");
    insert_entry(
        &mut conn,
        world,
        owner,
        "Court",
        "Attended by [[Nobody At All]].",
        None,
    );

    let plan = plan_world(&state, world).await.expect("a plan");

    assert!(
        plan.files[0].contents.contains("Nobody At All"),
        "a broken link was altered rather than left alone",
    );
    assert!(
        !plan
            .notes
            .iter()
            .any(|n| n.kind == "unresolvable_cross_link"),
        "a link that went nowhere was reported as a fidelity loss",
    );
}
