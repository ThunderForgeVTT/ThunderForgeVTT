//! Spec 031 (T072, US8/FR-038): lore stops being a flat list.
//!
//! Two operations, both about where an entry belongs rather than what it says:
//! `moveLoreEntry` sets an entry's parent, and `addLoreTag`/`removeLoreTag`
//! attach labels that cut across the tree. The client finds an entry by
//! either.
//!
//! # Why a cycle is refused here and not in the browser
//!
//! An entry that is its own ancestor is a subtree with no root: it never
//! appears in a tree render, and every walk of it either loops forever or
//! stops on a step counter. The client can and does hide the choice — a
//! move-target list that offers a descendant is a bad list — but hiding is
//! not refusing. Two Game Masters moving A under B and B under A in the same
//! second each see a legal move; only the second write is illegal, and only
//! the database ever sees both. Constitution Principle III puts the answer
//! here.
//!
//! # Why an advisory lock rather than a recursive check alone
//!
//! Walking the new parent's ancestors is the check, but on its own it is not
//! the answer: under Postgres's default READ COMMITTED both transactions read
//! a chain that predates the other's write, both pass, and the pair commits a
//! cycle neither one could have created alone. Locking the *world* for the
//! duration of a move serialises them, so the loser's walk sees the winner's
//! write and refuses. The rejected alternative was `SELECT ... FOR UPDATE` on
//! the ancestor chain: it locks the rows the walk actually visits, but the
//! chain a competing move is about may not intersect it at all until the very
//! write that creates the cycle. A per-world lock is coarse and correct;
//! moves are rare and human-paced, so the contention it costs is theoretical.
//!
//! # Why tags are rows and not a column
//!
//! `world_lore_tags` is unique on (entry, tag), so "tag it twice" is settled
//! by the database rather than by a read-then-write. A text array column
//! would have made a duplicate a race and "which entries carry this tag" a
//! scan.

use async_graphql::{Context, Error, ErrorExtensions, InputObject, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::auth::lore_permissions::require_lore_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLLoreEntry};
use crate::graphql::{app_state, authenticated_user};
use crate::models::LoreEntry;
use crate::schema::{world_lore_entries, world_lore_tags};
use crate::state::AppState;

/// The extension code a refused move carries, keyed on by
/// `GraphQLRequestError.hasCode()` in `apps/web/src/api/graphqlClient.ts`.
///
/// Distinguishable in the way `ALREADY_TAKEN` is in `mutations_pickup.rs`: a
/// Game Master whose move lost a race is told the move would have made a
/// loop, and the client can say so and leave the tree as it was. A generic
/// failure would have them retry the one move that can never work.
pub const LORE_CYCLE: &str = "LORE_CYCLE";

/// How far up the tree a cycle check will walk before giving up.
///
/// Not a depth limit on lore — it is a termination guarantee. If a cycle ever
/// reaches the table by some path this module does not own (a hand-run SQL
/// statement, a future bulk import), the walk that meets it must refuse
/// rather than spin. A hierarchy of prose deeper than this is not a shape
/// anybody navigates.
const MAX_ANCESTOR_WALK: usize = 256;

/// Longest tag we store. Long enough for a real phrase, short enough that a
/// tag stays a label rather than becoming a sentence pasted into the box.
const MAX_TAG_LENGTH: usize = 64;

#[derive(InputObject, Debug, Clone)]
pub struct MoveLoreEntryInput {
    pub lore_entry_id: Uuid,
    /// The entry to sit under. `None` makes this a root — the same value a
    /// root already holds, so "move to top level" needs no separate verb.
    pub parent_id: Option<Uuid>,
}

#[derive(InputObject, Debug, Clone)]
pub struct LoreTagInput {
    pub lore_entry_id: Uuid,
    pub tag: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LoreTreeError {
    #[error("database error: {0}")]
    Database(String),
    #[error("lore entry not found")]
    NotFound,
    #[error("that move would put the entry inside itself")]
    Cycle,
    #[error("a lore entry can only sit under another entry in the same world")]
    OtherWorld,
    #[error("a tag needs at least one character and at most {MAX_TAG_LENGTH}")]
    BadTag,
}

impl From<DieselError> for LoreTreeError {
    fn from(e: DieselError) -> Self {
        LoreTreeError::Database(e.to_string())
    }
}

pub fn to_graphql_error(e: LoreTreeError) -> Error {
    let msg = e.to_string();
    match e {
        LoreTreeError::Cycle => Error::new(msg).extend_with(|_, ext| ext.set("code", LORE_CYCLE)),
        LoreTreeError::NotFound => {
            Error::new(msg).extend_with(|_, ext| ext.set("code", "NOT_FOUND"))
        }
        _ => Error::new(msg),
    }
}

/// Puts a tag into the one form comparison happens in.
///
/// "Ancient Ruins", "ancient ruins" and " ANCIENT  RUINS " are one tag, and
/// the unique constraint on (entry, tag) only means that if every writer
/// agrees on the spelling before the row is written. Normalising on read
/// instead would leave three rows in the table and three chips on the screen.
pub fn normalise_tag(raw: &str) -> Option<String> {
    let normalised = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalised = normalised.to_lowercase();
    if normalised.is_empty() || normalised.chars().count() > MAX_TAG_LENGTH {
        return None;
    }
    Some(normalised)
}

/// Takes the world-wide lore-tree lock for the rest of this transaction.
///
/// Released by COMMIT or ROLLBACK, never by hand — an `xact` lock cannot be
/// leaked by an early return, which is exactly why it is the transaction-
/// scoped variant and not `pg_advisory_lock`.
pub(crate) fn lock_world_tree(conn: &mut PgConnection, world_id: Uuid) -> Result<(), DieselError> {
    diesel::sql_query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind::<diesel::sql_types::Text, _>(format!("lore-tree:{world_id}"))
        .execute(conn)?;
    Ok(())
}

/// True when `candidate_parent` is `entry_id` or sits below it.
///
/// Walks upward from the candidate rather than downward from the entry: a
/// subtree can be wide, but the chain from any node to its root is as long as
/// the tree is deep and no longer.
fn would_cycle(
    conn: &mut PgConnection,
    entry_id: Uuid,
    candidate_parent: Uuid,
) -> Result<bool, DieselError> {
    let mut cursor = Some(candidate_parent);
    for _ in 0..MAX_ANCESTOR_WALK {
        let Some(current) = cursor else {
            return Ok(false);
        };
        if current == entry_id {
            return Ok(true);
        }
        cursor = world_lore_entries::table
            .filter(world_lore_entries::id.eq(current))
            .select(world_lore_entries::parent_id)
            .first::<Option<Uuid>>(conn)
            .optional()?
            .flatten();
    }
    // The walk ran out of patience, which can only mean the chain already
    // loops. Refusing is the only safe answer: this move cannot make that
    // better and might make it worse.
    Ok(true)
}

/// Testable core of `LoreTreeMutation::move_lore_entry`.
///
/// Editor on the entry being moved, the same level `updateLoreEntry` asks for
/// — where a page sits is part of editing it. Deliberately *not* a permission
/// check on the destination as well: a tree whose branches each need their
/// own grant is a tree nobody can tidy, and every entry in it is already
/// visible to the same world.
pub async fn move_lore_entry_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: MoveLoreEntryInput,
) -> Result<LoreEntry, LoreTreeError> {
    require_lore_permission(
        state,
        user_id,
        is_admin,
        input.lore_entry_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(|e| LoreTreeError::Database(e.message))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreTreeError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<LoreEntry, LoreTreeError, _>(|conn| {
            let entry = world_lore_entries::table
                .filter(world_lore_entries::id.eq(input.lore_entry_id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)
                .optional()?
                .ok_or(LoreTreeError::NotFound)?;

            // Before the check, not after: everything read below has to be
            // read while no competing move can be writing it.
            lock_world_tree(conn, entry.world_id)?;

            if let Some(parent_id) = input.parent_id {
                let parent_world = world_lore_entries::table
                    .filter(world_lore_entries::id.eq(parent_id))
                    .select(world_lore_entries::world_id)
                    .first::<Uuid>(conn)
                    .optional()?
                    .ok_or(LoreTreeError::NotFound)?;
                if parent_world != entry.world_id {
                    return Err(LoreTreeError::OtherWorld);
                }
                if would_cycle(conn, entry.id, parent_id)? {
                    return Err(LoreTreeError::Cycle);
                }
            }

            diesel::update(world_lore_entries::table.filter(world_lore_entries::id.eq(entry.id)))
                .set((
                    world_lore_entries::parent_id.eq(input.parent_id),
                    world_lore_entries::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)?;

            world_lore_entries::table
                .filter(world_lore_entries::id.eq(entry.id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)
                .map_err(LoreTreeError::from)
        })
    })
    .await
    .map_err(|_| LoreTreeError::Database("Failed to spawn blocking task".to_string()))?
}

/// Every tag on one entry, in the order a reader expects to see them.
pub fn tags_for_entry(conn: &mut PgConnection, entry_id: Uuid) -> Result<Vec<String>, DieselError> {
    world_lore_tags::table
        .filter(world_lore_tags::lore_entry_id.eq(entry_id))
        .order(world_lore_tags::tag.asc())
        .select(world_lore_tags::tag)
        .load::<String>(conn)
}

/// Testable core of `LoreTreeMutation::add_lore_tag`. Editor, as for a move.
pub async fn add_lore_tag_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: LoreTagInput,
) -> Result<Vec<String>, LoreTreeError> {
    let tag = normalise_tag(&input.tag).ok_or(LoreTreeError::BadTag)?;

    require_lore_permission(
        state,
        user_id,
        is_admin,
        input.lore_entry_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(|e| LoreTreeError::Database(e.message))?;

    let entry_id = input.lore_entry_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreTreeError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        // `DO NOTHING` rather than a read-then-insert: tagging something that
        // is already tagged is not an error a person should be shown, and the
        // unique constraint is what makes the second attempt harmless without
        // anyone having to check first.
        diesel::insert_into(world_lore_tags::table)
            .values((
                world_lore_tags::id.eq(Uuid::now_v7()),
                world_lore_tags::lore_entry_id.eq(entry_id),
                world_lore_tags::tag.eq(&tag),
                world_lore_tags::created_by.eq(user_id),
                world_lore_tags::created_at.eq(Utc::now().naive_utc()),
            ))
            .on_conflict((world_lore_tags::lore_entry_id, world_lore_tags::tag))
            .do_nothing()
            .execute(&mut conn)?;
        tags_for_entry(&mut conn, entry_id)
    })
    .await
    .map_err(|_| LoreTreeError::Database("Failed to spawn blocking task".to_string()))?
    .map_err(LoreTreeError::from)
}

/// Testable core of `LoreTreeMutation::remove_lore_tag`.
pub async fn remove_lore_tag_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: LoreTagInput,
) -> Result<Vec<String>, LoreTreeError> {
    let tag = normalise_tag(&input.tag).ok_or(LoreTreeError::BadTag)?;

    require_lore_permission(
        state,
        user_id,
        is_admin,
        input.lore_entry_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(|e| LoreTreeError::Database(e.message))?;

    let entry_id = input.lore_entry_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreTreeError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_lore_tags::table
                .filter(world_lore_tags::lore_entry_id.eq(entry_id))
                .filter(world_lore_tags::tag.eq(&tag)),
        )
        .execute(&mut conn)?;
        tags_for_entry(&mut conn, entry_id)
    })
    .await
    .map_err(|_| LoreTreeError::Database("Failed to spawn blocking task".to_string()))?
    .map_err(LoreTreeError::from)
}

#[derive(Default)]
pub struct LoreTreeMutation;

#[async_graphql::Object]
impl LoreTreeMutation {
    /// Move an entry under another, or to the top level with a null parent.
    async fn move_lore_entry(
        &self,
        ctx: &Context<'_>,
        input: MoveLoreEntryInput,
    ) -> GraphQLResult<GraphQLLoreEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        move_lore_entry_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLLoreEntry::from)
            .map_err(to_graphql_error)
    }

    /// Tag an entry. Returns the entry's full tag set, so a client never has
    /// to guess what the normalisation did to what it sent.
    async fn add_lore_tag(
        &self,
        ctx: &Context<'_>,
        input: LoreTagInput,
    ) -> GraphQLResult<Vec<String>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        add_lore_tag_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map_err(to_graphql_error)
    }

    /// Remove a tag. Idempotent — a tag that is not there is already gone.
    async fn remove_lore_tag(
        &self,
        ctx: &Context<'_>,
        input: LoreTagInput,
    ) -> GraphQLResult<Vec<String>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_lore_tag_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map_err(to_graphql_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_lore::{
        CreateLoreEntryInput, create_lore_entry_impl, delete_lore_entry_impl,
    };
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    /// Reads an entry's parent straight out of the table. Every assertion
    /// below is about a row, not about what a mutation chose to return.
    fn parent_of(conn: &mut PgConnection, entry_id: Uuid) -> Option<Uuid> {
        world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .select(world_lore_entries::parent_id)
            .first::<Option<Uuid>>(conn)
            .expect("failed to read parent_id")
    }

    fn entry_exists(conn: &mut PgConnection, entry_id: Uuid) -> bool {
        world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .count()
            .get_result::<i64>(conn)
            .expect("failed to count lore entries")
            > 0
    }

    async fn new_entry(state: &AppState, world_id: Uuid, owner_id: Uuid, title: &str) -> Uuid {
        create_lore_entry_impl(
            state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: title.to_string(),
                content: None,
            },
        )
        .await
        .expect("the world's owner may create a lore entry")
        .id
    }

    fn code_of(e: &Error) -> String {
        e.extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .map(|v| format!("{v:?}"))
            .unwrap_or_default()
    }

    /// The guard `mutations_party.rs` keeps for the same reason: a mutation
    /// that compiles but was never merged into the root fails for the first
    /// Game Master who tries to file a page, not for the suite.
    #[test]
    fn the_mutations_are_registered_under_the_names_the_client_uses() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        for name in ["moveLoreEntry(", "addLoreTag(", "removeLoreTag("] {
            assert!(sdl.contains(name), "{name} must be reachable from the root");
        }
        // The read half matters as much: a tree the client cannot see the
        // shape of is a tree it cannot draw.
        assert!(sdl.contains("parentId: UUID"));
    }

    /// FR-038: a move writes the parent, and moving to a null parent puts the
    /// entry back at the top level.
    #[tokio::test]
    async fn a_move_writes_the_parent_and_a_null_parent_returns_to_the_root() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let parent = new_entry(&state, world_id, owner_id, "Kingdoms").await;
        let child = new_entry(&state, world_id, owner_id, "Veldrath").await;

        move_lore_entry_impl(
            &state,
            owner_id,
            false,
            MoveLoreEntryInput {
                lore_entry_id: child,
                parent_id: Some(parent),
            },
        )
        .await
        .expect("an Owner may move their own entry");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(parent_of(&mut conn, child), Some(parent));
        drop(conn);

        move_lore_entry_impl(
            &state,
            owner_id,
            false,
            MoveLoreEntryInput {
                lore_entry_id: child,
                parent_id: None,
            },
        )
        .await
        .expect("moving back to the top level is the same operation");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(parent_of(&mut conn, child), None);
    }

    /// FR-038: an entry may not become its own ancestor, directly or through
    /// any number of steps — and the refusal is coded so the client can say
    /// what actually happened.
    #[tokio::test]
    async fn a_move_that_would_close_a_loop_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let a = new_entry(&state, world_id, owner_id, "A").await;
        let b = new_entry(&state, world_id, owner_id, "B").await;
        let c = new_entry(&state, world_id, owner_id, "C").await;

        for (child, parent) in [(b, a), (c, b)] {
            move_lore_entry_impl(
                &state,
                owner_id,
                false,
                MoveLoreEntryInput {
                    lore_entry_id: child,
                    parent_id: Some(parent),
                },
            )
            .await
            .expect("building the chain A > B > C must succeed");
        }

        let onto_itself = move_lore_entry_impl(
            &state,
            owner_id,
            false,
            MoveLoreEntryInput {
                lore_entry_id: a,
                parent_id: Some(a),
            },
        )
        .await
        .expect_err("an entry may not be its own parent");
        assert!(code_of(&to_graphql_error(onto_itself)).contains(LORE_CYCLE));

        let onto_grandchild = move_lore_entry_impl(
            &state,
            owner_id,
            false,
            MoveLoreEntryInput {
                lore_entry_id: a,
                parent_id: Some(c),
            },
        )
        .await
        .expect_err("A may not move under its own grandchild");
        assert!(code_of(&to_graphql_error(onto_grandchild)).contains(LORE_CYCLE));

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            parent_of(&mut conn, a),
            None,
            "a refused move must leave the tree exactly as it was"
        );
        assert_eq!(parent_of(&mut conn, b), Some(a));
        assert_eq!(parent_of(&mut conn, c), Some(b));
    }

    /// The case a client-side check cannot cover: two moves that are each
    /// legal against the tree they read, and a cycle if both land. Exactly
    /// one may commit.
    #[tokio::test]
    async fn two_concurrent_moves_cannot_form_a_cycle_between_them() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let a = new_entry(&state, world_id, owner_id, "A").await;
        let b = new_entry(&state, world_id, owner_id, "B").await;

        let attempts = [(a, b), (b, a)].map(|(child, parent)| {
            let state = state.clone();
            tokio::spawn(async move {
                move_lore_entry_impl(
                    &state,
                    owner_id,
                    false,
                    MoveLoreEntryInput {
                        lore_entry_id: child,
                        parent_id: Some(parent),
                    },
                )
                .await
            })
        });

        let mut winners = 0;
        let mut cycle_refusals = 0;
        for attempt in attempts {
            match attempt.await.expect("move task must not panic") {
                Ok(_) => winners += 1,
                Err(e) => {
                    assert!(
                        code_of(&to_graphql_error(e)).contains(LORE_CYCLE),
                        "the losing move must be told it would make a loop"
                    );
                    cycle_refusals += 1;
                }
            }
        }
        assert_eq!(winners, 1, "exactly one of two mutually-parenting moves");
        assert_eq!(cycle_refusals, 1);

        // The table, not the return values: exactly one of the two rows has a
        // parent, which is the only shape that is not a loop.
        let mut conn = state.db_pool.get().unwrap();
        let parents = [parent_of(&mut conn, a), parent_of(&mut conn, b)];
        assert_eq!(
            parents.iter().filter(|p| p.is_some()).count(),
            1,
            "two parented rows here would be the cycle we exist to prevent"
        );
    }

    /// FR-038: deleting a parent hands its children to their grandparent.
    #[tokio::test]
    async fn deleting_a_parent_reparents_its_children_to_the_grandparent() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let grandparent = new_entry(&state, world_id, owner_id, "Realms").await;
        let parent = new_entry(&state, world_id, owner_id, "Kingdoms").await;
        let child = new_entry(&state, world_id, owner_id, "Veldrath").await;
        let sibling = new_entry(&state, world_id, owner_id, "Tarn").await;
        for (child_id, parent_id) in [(parent, grandparent), (child, parent), (sibling, parent)] {
            move_lore_entry_impl(
                &state,
                owner_id,
                false,
                MoveLoreEntryInput {
                    lore_entry_id: child_id,
                    parent_id: Some(parent_id),
                },
            )
            .await
            .expect("building the tree must succeed");
        }

        delete_lore_entry_impl(&state, owner_id, false, parent)
            .await
            .expect("an Owner may delete their own entry");

        let mut conn = state.db_pool.get().unwrap();
        assert!(!entry_exists(&mut conn, parent));
        assert!(
            entry_exists(&mut conn, child) && entry_exists(&mut conn, sibling),
            "a deleted entry must never take its children with it"
        );
        assert_eq!(
            (parent_of(&mut conn, child), parent_of(&mut conn, sibling)),
            (Some(grandparent), Some(grandparent)),
            "children inherit the deleted entry's own parent"
        );
    }

    /// The root case of the same rule: there is no grandparent, so the
    /// children become roots rather than being stranded under a parent id
    /// that no longer names anything.
    #[tokio::test]
    async fn deleting_a_root_makes_its_children_roots() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let root = new_entry(&state, world_id, owner_id, "Realms").await;
        let child = new_entry(&state, world_id, owner_id, "Kingdoms").await;
        let grandchild = new_entry(&state, world_id, owner_id, "Veldrath").await;
        for (child_id, parent_id) in [(child, root), (grandchild, child)] {
            move_lore_entry_impl(
                &state,
                owner_id,
                false,
                MoveLoreEntryInput {
                    lore_entry_id: child_id,
                    parent_id: Some(parent_id),
                },
            )
            .await
            .expect("building the tree must succeed");
        }

        delete_lore_entry_impl(&state, owner_id, false, root)
            .await
            .expect("an Owner may delete their own root entry");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            parent_of(&mut conn, child),
            None,
            "a root's child is a root"
        );
        assert_eq!(
            parent_of(&mut conn, grandchild),
            Some(child),
            "only the deleted entry's own children move; the rest of the \
             subtree keeps its shape"
        );
    }

    /// Tags are normalised before they are compared, so one label written
    /// three ways is one row and one chip.
    #[tokio::test]
    async fn tags_are_normalised_and_a_repeat_tag_adds_nothing() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = new_entry(&state, world_id, owner_id, "Veldrath").await;
        for raw in ["Ancient Ruins", "  ancient   ruins ", "ANCIENT RUINS"] {
            add_lore_tag_impl(
                &state,
                owner_id,
                false,
                LoreTagInput {
                    lore_entry_id: entry,
                    tag: raw.to_string(),
                },
            )
            .await
            .expect("tagging must accept a tag it has already stored");
        }

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            tags_for_entry(&mut conn, entry).unwrap(),
            vec!["ancient ruins".to_string()],
            "three spellings of one tag are one row"
        );
        drop(conn);

        remove_lore_tag_impl(
            &state,
            owner_id,
            false,
            LoreTagInput {
                lore_entry_id: entry,
                tag: "Ancient Ruins".to_string(),
            },
        )
        .await
        .expect("removal normalises the same way adding does");

        let mut conn = state.db_pool.get().unwrap();
        assert!(tags_for_entry(&mut conn, entry).unwrap().is_empty());
    }

    /// A deleted entry's tags go with it — the row is meaningless without the
    /// entry, and the migration's `ON DELETE CASCADE` is what removes it.
    #[tokio::test]
    async fn deleting_an_entry_takes_its_tags() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = new_entry(&state, world_id, owner_id, "Veldrath").await;
        add_lore_tag_impl(
            &state,
            owner_id,
            false,
            LoreTagInput {
                lore_entry_id: entry,
                tag: "ruins".to_string(),
            },
        )
        .await
        .unwrap();

        delete_lore_entry_impl(&state, owner_id, false, entry)
            .await
            .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        let orphaned: i64 = world_lore_tags::table
            .filter(world_lore_tags::lore_entry_id.eq(entry))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(orphaned, 0);
    }
}
