//! One declaration governing the GraphQL "ownership block" surface for every
//! permissioned content type.
//!
//! # Why this exists, and why it is separate from its sibling
//!
//! [`crate::auth::permissioned_entities`] unified the *resolution* half of
//! this problem — `effective_*_permission`, `require_*_permission`, and the
//! member-grant purge — and its module doc records the privilege leak the
//! duplication cost. It stopped at the auth boundary. The GraphQL half stayed
//! as four modules of 277–390 lines that each carried the same six items:
//!
//! * `Set*PermissionInput`, three fields, identical but for the content id;
//! * a private `require_dm_of_*_world` that reads the parent row's `world_id`
//!   and hands it to `is_dm_of_world`;
//! * a list, an upsert and a delete, each re-checking that gate first;
//! * a `*PermissionQuery` and a `*PermissionMutation` wrapping those three.
//!
//! Three of the four said so in their own headers — `item` called itself "a
//! direct structural mirror" of `actor`, and `ability` a mirror of `item`.
//!
//! # Why it is two declarations and not one
//!
//! Ideally a content type is declared once. It cannot be: `macro_rules!`
//! expands within one module, and these two macros emit into different ones —
//! the auth functions belong beside the other auth code, and these resolver
//! structs must live where the schema roots can reach them. The parameter sets
//! barely overlap besides.
//!
//! So the drift risk that caused the original leak is answered by a test
//! instead of by construction: [`DECLARED_RESOLVER_ENTITIES`] is compared
//! against `permissioned_entities::DECLARED_ENTITIES`, and declaring a fifth
//! content type in one list and not the other fails the build. That is the
//! exact omission that shipped `world_ability_permissions` against three
//! hand-written cleanup blocks.
//!
//! # What this deliberately does NOT generate
//!
//! Nothing about visibility. As with the auth sibling: the ownership block
//! governs **edit rights only**, and its lowest level (`Viewer`) is also its
//! default for a member with no row, so the ladder structurally cannot express
//! "hidden". An ability is hidden by `world_abilities.gm_only`, through its own
//! DM-gated mutation. This macro must never gain a visibility parameter for
//! symmetry.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::graphql::{app_state, authenticated_user};
use crate::state::AppState;

/// Generates, per declared entity: the input object, the DM gate, the three
/// testable `*_impl` cores, and the `Query`/`Mutation` resolver pair.
///
/// Every generated symbol is named explicitly rather than synthesised, for the
/// reason the auth sibling gives: macro-generated items have no
/// go-to-definition, so `set_actor_permission_impl` must be greppable to the
/// line that declares it.
macro_rules! permissioned_entity_resolvers {
    (
        $(
            $entity:ident {
                input: $input:ident,
                input_field: $input_field:ident,
                gate: $gate:ident,
                list_impl: $list_impl:ident,
                set_impl: $set_impl:ident,
                remove_impl: $remove_impl:ident,
                query_type: $query_type:ident,
                mutation_type: $mutation_type:ident,
                list_field: $list_field:ident,
                set_field: $set_field:ident,
                remove_field: $remove_field:ident,
                row: $row:ty,
                new_row: $new_row:ident,
                gql: $gql:ident,
                grants: $grants:ident,
                content_fk: $content_fk:ident,
                user_fk: $user_fk:ident,
                parent: $parent:ident,
                noun: $noun:expr,
                noun_capitalized: $noun_capitalized:expr,
                short_noun: $short_noun:expr,
                article: $article:expr,
                list_doc: $list_doc:expr,
            }
        ),* $(,)?
    ) => {
        $(
            #[derive(InputObject, Debug, Clone)]
            pub struct $input {
                pub $input_field: Uuid,
                pub user_id: Uuid,
                pub level: ActorPermissionLevel,
            }

            #[doc = concat!(
                "DM-only. Reading *and* writing ", $article, " ", $noun,
                "'s ownership block requires DM status — Editor, and even \
                 content-level Owner, is deliberately not sufficient, because \
                 the block is what confers those levels in the first place."
            )]
            async fn $gate(
                state: &AppState,
                caller_id: Uuid,
                is_admin: bool,
                content_id: Uuid,
            ) -> GraphQLResult<()> {
                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;

                let world_id = tokio::task::spawn_blocking(move || {
                    crate::schema::$parent::table
                        .filter(crate::schema::$parent::id.eq(content_id))
                        .select(crate::schema::$parent::world_id)
                        .first::<Uuid>(&mut conn)
                        .optional()
                })
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(|_| Error::new(concat!("Failed to load ", $noun)))?
                .ok_or_else(|| Error::new(concat!($noun_capitalized, " not found")))?;

                if is_dm_of_world(state, caller_id, is_admin, world_id).await? {
                    Ok(())
                } else {
                    Err(Error::new(concat!(
                        "Only the DM (Owner or GM) may view or change ",
                        $article, " ", $noun, "'s ownership block"
                    )))
                }
            }

            #[doc = concat!(
                "Testable core of the `", stringify!($list_field), "` query. \
                 Returns only explicit rows — a member with none defaults to \
                 `Viewer`, which the client renders by combining this with the \
                 world-member roster."
            )]
            pub async fn $list_impl(
                state: &AppState,
                caller_id: Uuid,
                is_admin: bool,
                content_id: Uuid,
            ) -> GraphQLResult<Vec<$row>> {
                $gate(state, caller_id, is_admin, content_id).await?;

                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;

                tokio::task::spawn_blocking(move || {
                    crate::schema::$grants::table
                        .filter(crate::schema::$grants::$content_fk.eq(content_id))
                        .select(<$row>::as_select())
                        .load::<$row>(&mut conn)
                })
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(|_| Error::new(concat!("Failed to load ", $noun, " permissions")))
            }

            #[doc = concat!(
                "Testable core of the `", stringify!($set_field), "` mutation. \
                 DM-only. UPSERT on `(", stringify!($content_fk), ", ",
                stringify!($user_fk), ")`, so re-granting a level a member \
                 already holds is not an error."
            )]
            pub async fn $set_impl(
                state: &AppState,
                caller_id: Uuid,
                is_admin: bool,
                input: $input,
            ) -> GraphQLResult<$row> {
                $gate(state, caller_id, is_admin, input.$input_field).await?;

                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;

                let content_id = input.$input_field;
                let target_user_id = input.user_id;
                let level = input.level.as_db_str().to_string();

                tokio::task::spawn_blocking(move || {
                    let new_row = crate::models::$new_row {
                        id: Uuid::now_v7(),
                        $content_fk: content_id,
                        $user_fk: target_user_id,
                        level: level.clone(),
                    };

                    diesel::insert_into(crate::schema::$grants::table)
                        .values(&new_row)
                        .on_conflict((
                            crate::schema::$grants::$content_fk,
                            crate::schema::$grants::$user_fk,
                        ))
                        .do_update()
                        .set((
                            crate::schema::$grants::level.eq(level),
                            crate::schema::$grants::updated_at
                                .eq(chrono::Utc::now().naive_utc()),
                        ))
                        .returning(<$row>::as_returning())
                        .get_result::<$row>(&mut conn)
                        .map_err(|e| {
                            format!(concat!("Failed to set ", $short_noun, " permission: {}"), e)
                        })
                })
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(Error::new)
            }

            #[doc = concat!(
                "Testable core of the `", stringify!($remove_field), "` \
                 mutation. DM-only, and idempotent: it resets the member to \
                 the implicit `Viewer` default.\n\n\
                 Returns whether a row was actually deleted — `false` means \
                 there was nothing to remove, which is a no-op and not an \
                 error. See this module's declaration comment for why all four \
                 report it this way."
            )]
            pub async fn $remove_impl(
                state: &AppState,
                caller_id: Uuid,
                is_admin: bool,
                content_id: Uuid,
                user_id: Uuid,
            ) -> GraphQLResult<bool> {
                $gate(state, caller_id, is_admin, content_id).await?;

                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;

                tokio::task::spawn_blocking(move || {
                    diesel::delete(
                        crate::schema::$grants::table
                            .filter(crate::schema::$grants::$content_fk.eq(content_id))
                            .filter(crate::schema::$grants::$user_fk.eq(user_id)),
                    )
                    .execute(&mut conn)
                    .map(|rows| rows > 0)
                    .map_err(|e| {
                        format!(concat!("Failed to remove ", $short_noun, " permission: {}"), e)
                    })
                })
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(Error::new)
            }

            #[derive(Default)]
            pub struct $query_type;

            #[async_graphql::Object]
            impl $query_type {
                #[doc = $list_doc]
                async fn $list_field(
                    &self,
                    ctx: &Context<'_>,
                    $input_field: Uuid,
                ) -> GraphQLResult<Vec<crate::graphql::types::$gql>> {
                    let state = app_state(ctx)?;
                    let auth_user = authenticated_user(ctx)?;
                    let rows = $list_impl(
                        state,
                        auth_user.user_id,
                        auth_user.is_admin,
                        $input_field,
                    )
                    .await?;
                    Ok(rows
                        .into_iter()
                        .map(crate::graphql::types::$gql::from)
                        .collect())
                }
            }

            #[derive(Default)]
            pub struct $mutation_type;

            #[async_graphql::Object]
            impl $mutation_type {
                #[doc = concat!(
                    "DM-only. Grants ", $article, " ", $noun, "-level \
                     permission to one world member."
                )]
                async fn $set_field(
                    &self,
                    ctx: &Context<'_>,
                    input: $input,
                ) -> GraphQLResult<crate::graphql::types::$gql> {
                    let state = app_state(ctx)?;
                    let auth_user = authenticated_user(ctx)?;
                    $set_impl(state, auth_user.user_id, auth_user.is_admin, input)
                        .await
                        .map(crate::graphql::types::$gql::from)
                }

                #[doc = concat!(
                    "DM-only. Clears a member's explicit grant, returning them \
                     to the implicit `Viewer` default. `false` means there was \
                     no grant to clear."
                )]
                async fn $remove_field(
                    &self,
                    ctx: &Context<'_>,
                    $input_field: Uuid,
                    user_id: Uuid,
                ) -> GraphQLResult<bool> {
                    let state = app_state(ctx)?;
                    let auth_user = authenticated_user(ctx)?;
                    $remove_impl(
                        state,
                        auth_user.user_id,
                        auth_user.is_admin,
                        $input_field,
                        user_id,
                    )
                    .await
                }
            }
        )*

        /// The entity names this declaration covers.
        ///
        /// Compared against `permissioned_entities::DECLARED_ENTITIES` by the
        /// test below. Two lists exist because two macros expand into two
        /// modules; this is what stops them diverging.
        #[cfg(test)]
        pub const DECLARED_RESOLVER_ENTITIES: &[&str] = &[$(stringify!($entity)),*];
    };
}

// ============================================================================
// The declaration. Adding a permissioned content type means adding one entry
// here and one in `auth::permissioned_entities` — and nothing anywhere else.
//
// Two asymmetries are absorbed as parameters rather than migrated away:
//
// * `world_lore_permissions` names its user column `world_member_user_id`,
//   unlike the other three. Renaming it would touch live data for cosmetic
//   uniformity.
// * A lore entry's long noun ("lore entry") is not its short one ("lore").
//   The two appear in different messages, and both were already in the code.
//
// One divergence was NOT preserved, because unifying forced a choice.
// `removeAbilityPermission` returned whether a row was actually deleted; the
// other three returned `true` unconditionally. Every one of the four is
// generated with the ability semantics now, so removing a grant that does not
// exist reports `false` on all of them. It is the more informative of the two —
// always-true cannot distinguish a removal from a no-op — and it was the only
// one of the pair with a test asserting it. No caller reads the value: all four
// `*OwnershipBlock.tsx` components await the mutation and discard it.
// ============================================================================

permissioned_entity_resolvers! {
    actor {
        input: SetActorPermissionInput,
        input_field: actor_id,
        gate: require_dm_of_actors_world,
        list_impl: actor_permissions_impl,
        set_impl: set_actor_permission_impl,
        remove_impl: remove_actor_permission_impl,
        query_type: ActorPermissionQuery,
        mutation_type: ActorPermissionMutation,
        list_field: actor_permissions,
        set_field: set_actor_permission,
        remove_field: remove_actor_permission,
        row: crate::models::ActorPermission,
        new_row: NewActorPermission,
        gql: GraphQLActorPermission,
        grants: world_actor_permissions,
        content_fk: actor_id,
        user_fk: user_id,
        parent: world_actors,
        noun: "actor",
        noun_capitalized: "Actor",
        short_noun: "actor",
        article: "an",
        list_doc: "FR-014: only the DM may view or change the ownership block. \
                   Returns only explicit rows — members with no row default to \
                   Viewer, which the client renders itself by combining this \
                   with the full `worldMembers` roster \
                   (contracts/actor-permissions.md).",
    },
    item {
        input: SetItemPermissionInput,
        input_field: item_id,
        gate: require_dm_of_items_world,
        list_impl: item_permissions_impl,
        set_impl: set_item_permission_impl,
        remove_impl: remove_item_permission_impl,
        query_type: ItemPermissionQuery,
        mutation_type: ItemPermissionMutation,
        list_field: item_permissions,
        set_field: set_item_permission,
        remove_field: remove_item_permission,
        row: crate::models::ItemPermission,
        new_row: NewItemPermission,
        gql: GraphQLItemPermission,
        grants: world_item_permissions,
        content_fk: item_id,
        user_fk: user_id,
        parent: world_items,
        noun: "item",
        noun_capitalized: "Item",
        short_noun: "item",
        article: "an",
        list_doc: "FR-003: only the DM may view or change the ownership block. \
                   Returns only explicit rows — members with no row default to \
                   Viewer.",
    },
    lore {
        input: SetLorePermissionInput,
        input_field: lore_entry_id,
        gate: require_dm_of_entrys_world,
        list_impl: lore_entry_permissions_impl,
        set_impl: set_lore_permission_impl,
        remove_impl: remove_lore_permission_impl,
        query_type: LorePermissionQuery,
        mutation_type: LorePermissionMutation,
        list_field: lore_entry_permissions,
        set_field: set_lore_permission,
        remove_field: remove_lore_permission,
        row: crate::models::LorePermission,
        new_row: NewLorePermission,
        gql: GraphQLLorePermission,
        grants: world_lore_permissions,
        content_fk: lore_entry_id,
        user_fk: world_member_user_id,
        parent: world_lore_entries,
        noun: "lore entry",
        noun_capitalized: "Lore entry",
        short_noun: "lore",
        article: "a",
        list_doc: "FR-003: only the DM may view or change the ownership block. \
                   Returns only explicit rows — members with no row default to \
                   Viewer, which the client renders itself by combining this \
                   with the full world-member roster \
                   (contracts/lore-permissions.md).",
    },
    ability {
        input: SetAbilityPermissionInput,
        input_field: ability_id,
        gate: require_dm_of_abilitys_world,
        list_impl: ability_permissions_impl,
        set_impl: set_ability_permission_impl,
        remove_impl: remove_ability_permission_impl,
        query_type: AbilityPermissionQuery,
        mutation_type: AbilityPermissionMutation,
        list_field: ability_permissions,
        set_field: set_ability_permission,
        remove_field: remove_ability_permission,
        row: crate::models::AbilityPermission,
        new_row: NewAbilityPermission,
        gql: GraphQLAbilityPermission,
        grants: world_ability_permissions,
        content_fk: ability_id,
        user_fk: user_id,
        parent: world_abilities,
        noun: "ability",
        noun_capitalized: "Ability",
        short_noun: "ability",
        article: "an",
        list_doc: "FR-026: the ownership block is DM-only to read *and* write. \
                   This governs EDIT RIGHTS ONLY — visibility is \
                   `world_abilities.gm_only`, changed through its own DM-gated \
                   mutation, because the ladder's lowest level (`Viewer`) is \
                   also its default and so cannot express \"hidden\".",
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_ability, insert_test_actor, insert_test_item, insert_test_lore_entry,
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    /// Removing a grant that is not there reports `false`, on **all four**.
    ///
    /// This is the one behaviour unification changed rather than preserved.
    /// `removeAbilityPermission` reported whether a row was actually deleted;
    /// the other three returned `true` whatever happened, so a caller could
    /// not tell a removal from a no-op. Generating all four from one body
    /// forced a choice, and this asserts which one was made — for every type,
    /// so the answer cannot quietly become three-and-one again.
    ///
    /// Both halves matter: `true` the first time proves the row really was
    /// there and really was deleted, so `false` the second time means "nothing
    /// to remove" and not "removal is broken".
    #[tokio::test]
    async fn removing_a_grant_that_is_not_there_reports_false_on_every_type() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();

        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let lore_entry_id = insert_test_lore_entry(&mut conn, world_id, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        let member_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        drop(conn);

        // Grant on every type, then remove twice on every type. Written out
        // per type rather than looped because the four take four different
        // input structs — the point of the macro is that the bodies are one,
        // not that the call sites are.
        set_actor_permission_impl(
            &state,
            owner_id,
            false,
            SetActorPermissionInput {
                actor_id,
                user_id: member_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .unwrap();
        set_item_permission_impl(
            &state,
            owner_id,
            false,
            SetItemPermissionInput {
                item_id,
                user_id: member_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .unwrap();
        set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput {
                lore_entry_id,
                user_id: member_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .unwrap();
        set_ability_permission_impl(
            &state,
            owner_id,
            false,
            SetAbilityPermissionInput {
                ability_id,
                user_id: member_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .unwrap();

        let first = [
            remove_actor_permission_impl(&state, owner_id, false, actor_id, member_id)
                .await
                .unwrap(),
            remove_item_permission_impl(&state, owner_id, false, item_id, member_id)
                .await
                .unwrap(),
            remove_lore_permission_impl(&state, owner_id, false, lore_entry_id, member_id)
                .await
                .unwrap(),
            remove_ability_permission_impl(&state, owner_id, false, ability_id, member_id)
                .await
                .unwrap(),
        ];
        assert_eq!(
            first, [true; 4],
            "removing a grant that exists must report true on every type"
        );

        let second = [
            remove_actor_permission_impl(&state, owner_id, false, actor_id, member_id)
                .await
                .unwrap(),
            remove_item_permission_impl(&state, owner_id, false, item_id, member_id)
                .await
                .unwrap(),
            remove_lore_permission_impl(&state, owner_id, false, lore_entry_id, member_id)
                .await
                .unwrap(),
            remove_ability_permission_impl(&state, owner_id, false, ability_id, member_id)
                .await
                .unwrap(),
        ];
        assert_eq!(
            second, [false; 4],
            "removing a grant that is not there must report false on every type, \
             not true — and idempotently, without erroring"
        );
    }

    /// Every field the client already calls is still on the schema, spelled
    /// the same way.
    ///
    /// This is the guard that makes the refactor safe to believe. The twelve
    /// fields below were hand-written a moment ago and are macro-generated
    /// now; a macro that silently renamed one — `loreEntryPermissions` to
    /// `lorePermissions`, say, since the impl is called
    /// `lore_entry_permissions_impl` and every other type's two names agree —
    /// would compile, pass every behavioural test, and break the first Game
    /// Master who opened an ownership block.
    ///
    /// The argument names are asserted too, because a generated resolver takes
    /// its argument name from the parameter ident: `$input_field` is the same
    /// token that names the struct field, and getting that wrong renames a
    /// GraphQL argument without touching a type.
    #[test]
    fn every_ownership_block_field_keeps_the_name_the_client_uses() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        for field in [
            // The read half, per type.
            "actorPermissions(actorId: UUID!",
            "itemPermissions(itemId: UUID!",
            "loreEntryPermissions(loreEntryId: UUID!",
            "abilityPermissions(abilityId: UUID!",
            // The write half.
            "setActorPermission(input: SetActorPermissionInput!",
            "setItemPermission(input: SetItemPermissionInput!",
            "setLorePermission(input: SetLorePermissionInput!",
            "setAbilityPermission(input: SetAbilityPermissionInput!",
            // And the clear-back-to-default half, which takes two arguments
            // rather than an input object.
            "removeActorPermission(actorId: UUID!, userId: UUID!)",
            "removeItemPermission(itemId: UUID!, userId: UUID!)",
            "removeLorePermission(loreEntryId: UUID!, userId: UUID!)",
            "removeAbilityPermission(abilityId: UUID!, userId: UUID!)",
        ] {
            assert!(
                sdl.contains(field),
                "`{field}` is missing from the schema — the generated surface \
                 does not match what the client calls"
            );
        }

        // The input objects keep their three fields under their own names.
        for input in [
            "input SetActorPermissionInput",
            "input SetItemPermissionInput",
            "input SetLorePermissionInput",
            "input SetAbilityPermissionInput",
        ] {
            assert!(sdl.contains(input), "`{input}` must still be declared");
        }
    }

    /// The two declarations must cover the same content types.
    ///
    /// This is the whole reason it is safe for there to be two. A fifth type
    /// added to one list and not the other is precisely the omission that
    /// shipped `world_ability_permissions` against three hand-written cleanup
    /// blocks and left removed members holding their ability grants — the
    /// defect `auth::permissioned_entities` was built to make impossible. It
    /// would be a poor trade to reintroduce it one layer up.
    #[test]
    fn every_permissioned_entity_has_both_halves() {
        let mut auth = crate::auth::permissioned_entities::DECLARED_ENTITIES.to_vec();
        let mut resolvers = super::DECLARED_RESOLVER_ENTITIES.to_vec();
        auth.sort_unstable();
        resolvers.sort_unstable();
        assert_eq!(
            auth, resolvers,
            "auth::permissioned_entities and graphql::permissioned_entity_resolvers \
             declare different content types — a type with only one half has \
             either no ownership block or no permission resolution"
        );
    }
}
