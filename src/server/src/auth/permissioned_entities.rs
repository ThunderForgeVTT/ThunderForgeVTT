//! Spec 027 US5: one declaration governing permission resolution and
//! member-removal cleanup for every permissioned content type.
//!
//! # Why this exists
//!
//! Four modules — `actor_permissions`, `item_permissions`, `lore_permissions`,
//! `ability_permissions` — each carried a near-verbatim `effective_*` /
//! `require_*` pair. All four resolved identically: DM of the owning world →
//! `Owner` (implicit, un-removable); else an explicit grant row; else
//! `Viewer`. Only the table and the noun differed. The duplication was
//! acknowledged in the source (`item_permissions` called itself "a direct
//! structural mirror" of `actor_permissions`, and `ability_permissions` a
//! mirror of `item_permissions`) and never resolved.
//!
//! It cost a live privilege leak. `remove_member_impl` cleaned up a removed
//! member's grants in three hand-written blocks, and spec 025 added a fourth
//! content type without adding a fourth block — so a removed member kept their
//! ability grants, and re-adding them silently restored Editor or Owner
//! rights.
//!
//! Because a single invocation lists every type, it can emit both the
//! resolvers and an aggregate [`purge_member_grants`] that walks all of them.
//! There is no second list to keep in sync, which is the precise mechanism by
//! which that omission happened.
//!
//! # Why a macro rather than a trait
//!
//! Diesel gives every table its own generated type. A function generic over
//! "any permissions table" needs bounds on `Table`, `Column`,
//! `SelectableExpression`, `QueryFragment`, `AppearsOnTable` and the query DSL
//! types for each `filter`/`select` in the body — a bound list longer than the
//! four bodies it would replace, failing with errors that name Diesel
//! internals rather than anything here. See ADR-050, which also records why
//! collapsing the four tables into one polymorphic table was rejected: each
//! declares `ON DELETE CASCADE` to its content, and a polymorphic table cannot
//! carry that FK.
//!
//! # What this deliberately does NOT generate
//!
//! `is_ability_visible_to` stays hand-written in `ability_permissions`.
//! Visibility is a **separate axis** from the permission ladder: `Viewer` is
//! both the ladder's floor and its default, so the ladder structurally cannot
//! express "hidden" — that is what `world_abilities.gm_only` is for. This
//! macro must never gain a visibility parameter "for symmetry"; doing so
//! invites the next content type to express hidden-ness as a permission level,
//! which is the confusion spec 025 documented at length.

use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;

/// Generates, per declared entity: `effective_*_permission`,
/// `require_*_permission`, and a per-type member-grant purge. Generates once,
/// over all of them: [`purge_member_grants`].
///
/// Function names are supplied explicitly rather than synthesised from the
/// entity name. That keeps every generated symbol greppable — searching for
/// `effective_actor_permission` finds the declaration below — which matters
/// more here than brevity, since macro-generated items have no
/// go-to-definition.
macro_rules! permissioned_entities {
    (
        $(
            $entity:ident {
                effective: $effective:ident,
                require: $require:ident,
                purge: $purge:ident,
                grants: $grants:ident,
                content_fk: $content_fk:ident,
                user_fk: $user_fk:ident,
                parent: $parent:ident,
                noun: $noun:literal,
                noun_capitalized: $noun_cap:literal,
            }
        ),* $(,)?
    ) => {
        $(
            #[doc = concat!(
                "Resolves the caller's effective permission on one ", $noun, ": \
                 DM of its world → always `Owner` (implicit, un-removable); \
                 else their explicit `", stringify!($grants), "` row; \
                 else `Viewer`."
            )]
            pub async fn $effective(
                state: &AppState,
                user_id: Uuid,
                is_admin: bool,
                content_id: Uuid,
            ) -> GraphQLResult<ActorPermissionLevel> {
                use crate::schema::{$grants, $parent};

                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;

                let world_id = tokio::task::spawn_blocking(move || {
                    $parent::table
                        .filter($parent::id.eq(content_id))
                        .select($parent::world_id)
                        .first::<Uuid>(&mut conn)
                        .optional()
                })
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(|_| Error::new(concat!("Failed to load ", $noun)))?
                .ok_or_else(|| Error::new(concat!($noun_cap, " not found")))?;

                if is_dm_of_world(state, user_id, is_admin, world_id).await? {
                    return Ok(ActorPermissionLevel::Owner);
                }

                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;

                let level = tokio::task::spawn_blocking(move || {
                    $grants::table
                        .filter($grants::$content_fk.eq(content_id))
                        .filter($grants::$user_fk.eq(user_id))
                        .select($grants::level)
                        .first::<String>(&mut conn)
                        .optional()
                })
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(|_| Error::new(concat!("Failed to load ", $noun, " permission")))?;

                // An unparseable level falls back to Viewer rather than
                // erroring. Preserved behaviour: a malformed row must not lock
                // a world out of its own content.
                Ok(level
                    .and_then(|value| ActorPermissionLevel::from_db_str(&value))
                    .unwrap_or(ActorPermissionLevel::Viewer))
            }

            #[doc = concat!(
                "Rejects the caller unless their effective permission on this ",
                $noun, " is at least `minimum`."
            )]
            pub async fn $require(
                state: &AppState,
                user_id: Uuid,
                is_admin: bool,
                content_id: Uuid,
                minimum: ActorPermissionLevel,
            ) -> GraphQLResult<()> {
                let level = $effective(state, user_id, is_admin, content_id).await?;

                if level.rank() >= minimum.rank() {
                    Ok(())
                } else {
                    Err(Error::new(concat!(
                        "You do not have sufficient permission on this ",
                        $noun
                    ))
                    .extend_with(|_, ext| ext.set("code", "FORBIDDEN")))
                }
            }

            #[doc = concat!(
                "Deletes every explicit ", $noun, " grant `user_id` holds \
                 within `world_id`. Called only via [`purge_member_grants`]."
            )]
            fn $purge(
                conn: &mut PgConnection,
                world_id: Uuid,
                user_id: Uuid,
            ) -> QueryResult<usize> {
                use crate::schema::{$grants, $parent};

                diesel::delete(
                    $grants::table
                        .filter($grants::$user_fk.eq(user_id))
                        .filter(
                            $grants::$content_fk.eq_any(
                                $parent::table
                                    .filter($parent::world_id.eq(world_id))
                                    .select($parent::id),
                            ),
                        ),
                )
                .execute(conn)
            }
        )*

        /// Deletes every explicit grant `user_id` holds on **every**
        /// permissioned content type within `world_id`. Returns the total rows
        /// removed.
        ///
        /// # This is the point of the whole module
        ///
        /// The set of types walked here *is* the declaration above. A content
        /// type cannot be declared and then forgotten by cleanup, which is
        /// exactly what happened when `world_ability_permissions` shipped
        /// against three hand-written blocks.
        ///
        /// Removal is the only path that needs this. Deleting the content or
        /// the user account is already handled by `ON DELETE CASCADE`; there
        /// is no FK from `world_members` to the grant tables, because the
        /// relationship runs through `world_id` on the parent content table.
        pub fn purge_member_grants(
            conn: &mut PgConnection,
            world_id: Uuid,
            user_id: Uuid,
        ) -> QueryResult<usize> {
            let mut removed = 0usize;
            $(
                removed += $purge(conn, world_id, user_id)?;
            )*
            Ok(removed)
        }

        /// The declared entity names, for tests that must assert over the
        /// declaration rather than restating it.
        ///
        /// A cleanup test that hardcodes four type names cannot catch a fifth
        /// type being declared and skipped — it would merely restate the bug
        /// it exists to prevent. Asserting against this list makes the test
        /// fail when the declaration grows and something is left behind.
        #[cfg(test)]
        pub const DECLARED_ENTITIES: &[&str] = &[$(stringify!($entity)),*];
    };
}

// ============================================================================
// The declaration. Adding a permissioned content type means adding one entry
// here — and nothing anywhere else.
//
// `world_lore_permissions` names its user column `world_member_user_id` rather
// than `user_id`, unlike the other three. That asymmetry is real and is
// absorbed here as a parameter rather than migrated: renaming it would touch
// live data for cosmetic uniformity.
// ============================================================================

permissioned_entities! {
    actor {
        effective: effective_actor_permission,
        require: require_actor_permission,
        purge: purge_actor_grants_for_member,
        grants: world_actor_permissions,
        content_fk: actor_id,
        user_fk: user_id,
        parent: world_actors,
        noun: "actor",
        noun_capitalized: "Actor",
    },
    item {
        effective: effective_item_permission,
        require: require_item_permission,
        purge: purge_item_grants_for_member,
        grants: world_item_permissions,
        content_fk: item_id,
        user_fk: user_id,
        parent: world_items,
        noun: "item",
        noun_capitalized: "Item",
    },
    lore {
        effective: effective_lore_permission,
        require: require_lore_permission,
        purge: purge_lore_grants_for_member,
        grants: world_lore_permissions,
        content_fk: lore_entry_id,
        user_fk: world_member_user_id,
        parent: world_lore_entries,
        noun: "lore entry",
        noun_capitalized: "Lore entry",
    },
    ability {
        effective: effective_ability_permission,
        require: require_ability_permission,
        purge: purge_ability_grants_for_member,
        grants: world_ability_permissions,
        content_fk: ability_id,
        user_fk: user_id,
        parent: world_abilities,
        noun: "ability",
        noun_capitalized: "Ability",
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        count_content_permissions, grant_all_content_permissions, insert_test_ability,
        insert_test_actor, insert_test_item, insert_test_lore_entry, insert_test_scene,
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// One content row of each type in a fresh world, plus a member.
    /// Returns `(world_id, owner_id, member_id, [actor, item, lore, ability])`.
    fn world_with_one_of_everything(
        conn: &mut PgConnection,
    ) -> (Uuid, Uuid, Uuid, [Uuid; 4]) {
        let owner_id = insert_test_user(conn);
        let world_id = insert_test_world(conn, owner_id);
        let scene_id = insert_test_scene(conn, world_id, owner_id);

        let ids = [
            insert_test_actor(conn, world_id, scene_id, owner_id),
            insert_test_item(conn, world_id, owner_id),
            insert_test_lore_entry(conn, world_id, owner_id),
            insert_test_ability(conn, world_id, owner_id),
        ];

        let member_id = insert_test_user(conn);
        insert_test_world_member(conn, world_id, member_id, "Player");

        (world_id, owner_id, member_id, ids)
    }

    /// Resolves effective permission for each of the four types, so parity can
    /// be asserted without repeating the call shape four times per test.
    async fn all_four_levels(
        state: &AppState,
        user_id: Uuid,
        is_admin: bool,
        ids: [Uuid; 4],
    ) -> [ActorPermissionLevel; 4] {
        [
            effective_actor_permission(state, user_id, is_admin, ids[0])
                .await
                .expect("actor resolution"),
            effective_item_permission(state, user_id, is_admin, ids[1])
                .await
                .expect("item resolution"),
            effective_lore_permission(state, user_id, is_admin, ids[2])
                .await
                .expect("lore resolution"),
            effective_ability_permission(state, user_id, is_admin, ids[3])
                .await
                .expect("ability resolution"),
        ]
    }

    /// FR-015 / US5-1: identical conditions produce identical answers on every
    /// content type. This is the property the consolidation exists to
    /// guarantee, and the one that four hand-maintained copies could not.
    #[tokio::test]
    async fn all_four_types_resolve_identically_under_identical_conditions() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (_world_id, _owner_id, member_id, ids) = world_with_one_of_everything(&mut conn);
        drop(conn);

        // No explicit grants anywhere → Viewer on all four.
        let levels = all_four_levels(&state, member_id, false, ids).await;
        assert_eq!(
            levels,
            [ActorPermissionLevel::Viewer; 4],
            "a member with no grants must default to Viewer on every type"
        );

        // Editor on all four → Editor on all four.
        let mut conn = state.db_pool.get().unwrap();
        grant_all_content_permissions(
            &mut conn, member_id, ids[0], ids[1], ids[2], ids[3], "Editor",
        );
        drop(conn);

        let levels = all_four_levels(&state, member_id, false, ids).await;
        assert_eq!(
            levels,
            [ActorPermissionLevel::Editor; 4],
            "an explicit grant must resolve the same way on every type"
        );
    }

    /// FR-015 / US5-2: the DM holds the top level implicitly, everywhere, with
    /// no rows — and it cannot be taken away by a lower explicit grant.
    #[tokio::test]
    async fn the_dm_is_owner_everywhere_and_cannot_be_demoted() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (world_id, owner_id, _member_id, ids) = world_with_one_of_everything(&mut conn);
        drop(conn);

        let levels = all_four_levels(&state, owner_id, false, ids).await;
        assert_eq!(
            levels,
            [ActorPermissionLevel::Owner; 4],
            "the DM must hold Owner on every type with zero explicit rows"
        );

        // A GM-role member is equally a DM.
        let mut conn = state.db_pool.get().unwrap();
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        // Explicitly granting the GM only Viewer must not demote them.
        grant_all_content_permissions(
            &mut conn, gm_id, ids[0], ids[1], ids[2], ids[3], "Viewer",
        );
        drop(conn);

        let levels = all_four_levels(&state, gm_id, false, ids).await;
        assert_eq!(
            levels,
            [ActorPermissionLevel::Owner; 4],
            "a DM's implicit Owner must outrank an explicit lower grant"
        );
    }

    /// FR-021: behaviours the four hand-written copies had, which the
    /// generalization must not quietly "clean up".
    #[tokio::test]
    async fn preserved_edge_behaviours() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (_world_id, _owner_id, member_id, ids) = world_with_one_of_everything(&mut conn);
        drop(conn);

        // The unparseable-level fallback is defence in depth, not a reachable
        // path: every grant table declares
        // `CHECK (level IN ('Viewer', 'Editor', 'Owner'))`, so a malformed
        // value cannot be stored at all — attempting it fails with a
        // CheckViolation before any resolver sees it.
        //
        // The fallback is therefore asserted where it can actually be
        // exercised: on the parsing function itself. It must yield Viewer
        // rather than erroring, so that if the constraint were ever dropped, a
        // malformed row would deny access rather than crash the resolver or
        // grant it.
        assert_eq!(
            ActorPermissionLevel::from_db_str("Sorcerer-Supreme"),
            None,
            "an unrecognised level must not parse to something permissive"
        );

        // `is_admin` short-circuits to Owner without any membership at all.
        let outsider = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        let levels = all_four_levels(&state, outsider, true, ids).await;
        assert_eq!(
            levels,
            [ActorPermissionLevel::Owner; 4],
            "an admin must short-circuit to Owner on every type"
        );

        // A missing content row errors; a missing grant row does not.
        let ghost = Uuid::now_v7();
        assert!(
            effective_actor_permission(&state, member_id, false, ghost)
                .await
                .is_err(),
            "a nonexistent content row must error"
        );
        assert_eq!(
            effective_item_permission(&state, member_id, false, ids[1])
                .await
                .expect("a missing grant row must not error"),
            ActorPermissionLevel::Viewer
        );
    }

    /// Clarification carried from spec 010: Owner is uncapped — several
    /// simultaneous Owners are all accepted.
    #[tokio::test]
    async fn multiple_simultaneous_owners_are_all_accepted() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (world_id, _owner_id, member_a, ids) = world_with_one_of_everything(&mut conn);
        let member_b = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, member_b, "Player");

        for user in [member_a, member_b] {
            grant_all_content_permissions(
                &mut conn, user, ids[0], ids[1], ids[2], ids[3], "Owner",
            );
        }
        drop(conn);

        for user in [member_a, member_b] {
            require_actor_permission(&state, user, false, ids[0], ActorPermissionLevel::Owner)
                .await
                .expect("both simultaneous Owners must be accepted");
        }
    }

    /// FR-018 / SC-002 — the structural check.
    ///
    /// Asserts over [`DECLARED_ENTITIES`] rather than a hardcoded list of
    /// four. A test restating "actor, item, lore, ability" cannot catch a
    /// fifth type being declared and skipped by cleanup; that is precisely the
    /// bug this module exists to make impossible, so the test must be tied to
    /// the declaration itself.
    #[tokio::test]
    async fn purge_covers_every_declared_entity_type() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (world_id, _owner_id, member_id, ids) = world_with_one_of_everything(&mut conn);
        grant_all_content_permissions(
            &mut conn, member_id, ids[0], ids[1], ids[2], ids[3], "Editor",
        );

        let before = count_content_permissions(&mut conn, world_id, member_id);
        assert_eq!(before, (1, 1, 1, 1), "setup: one grant of each type");

        let removed = purge_member_grants(&mut conn, world_id, member_id)
            .expect("purge should succeed");

        // One row per declared type — derived from the declaration, so adding
        // a fifth type without wiring its cleanup fails right here.
        assert_eq!(
            removed,
            DECLARED_ENTITIES.len(),
            "purge must remove one grant per declared entity type; \
             declared: {DECLARED_ENTITIES:?}"
        );

        let after = count_content_permissions(&mut conn, world_id, member_id);
        assert_eq!(after, (0, 0, 0, 0), "no grant of any type may survive");

        // Idempotent: purging again removes nothing and does not error.
        let again = purge_member_grants(&mut conn, world_id, member_id)
            .expect("purging an empty set must succeed quietly");
        assert_eq!(again, 0);
    }

    /// The declaration and the counting helper must stay in step. If a fifth
    /// type is added, `count_content_permissions` needs widening too — this
    /// fails loudly rather than letting the coverage silently narrow.
    #[test]
    fn the_declaration_matches_what_the_test_helpers_count() {
        assert_eq!(
            DECLARED_ENTITIES.len(),
            4,
            "count_content_permissions returns a 4-tuple; widen it alongside \
             the declaration, then update this assertion"
        );
    }
}
