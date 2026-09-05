//! Spec 026: authoring a collection — create, rename, delete, add and remove
//! members. Sharing and copying live in `mutations_collection_shares.rs`.
//!
//! Governed by ADR-069 (the DMCA determination) and ADR-070 (the anonymous
//! read path). Two of ADR-069's conditions live in this file and must stay
//! true:
//!
//! * **No enumeration.** `world_collections(worldId)` returns one world's
//!   collections to a caller with authority over that world, and there is
//!   deliberately no query that reaches further. Adding one re-opens the
//!   determination.
//! * **Nothing restricted enters a collection** (FR-001a), because a
//!   collection is read by anyone holding its link — signed out, under
//!   ADR-070 — and publishing restricted content is the one failure in this
//!   feature its owner cannot undo.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::collections::{MAX_MEMBERS, is_known_member_type, membership};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{Collection, CollectionMember, NewCollection, NewCollectionMember};
use crate::schema::{world_collection_members, world_collections};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CreateCollectionInput {
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateCollectionInput {
    pub collection_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct AddCollectionMemberInput {
    pub collection_id: Uuid,
    pub member_type: String,
    pub member_id: Uuid,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLCollection {
    pub id: Uuid,
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub member_count: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLCollectionMember {
    pub id: Uuid,
    pub collection_id: Uuid,
    pub member_type: String,
    pub member_id: Uuid,
    pub sort_order: i32,
}

impl From<CollectionMember> for GraphQLCollectionMember {
    fn from(row: CollectionMember) -> Self {
        Self {
            id: row.id,
            collection_id: row.collection_id,
            member_type: row.member_type,
            member_id: row.member_id,
            sort_order: row.sort_order,
        }
    }
}

/// The collection's world, and an authority check over it in one step.
///
/// Every mutation here funnels through this rather than checking authority
/// each in its own way. A second way to answer "may this caller touch this
/// collection" is a second place for the answer to drift.
async fn require_collection_authority(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    collection_id: Uuid,
) -> GraphQLResult<Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = tokio::task::spawn_blocking(move || {
        world_collections::table
            .filter(world_collections::id.eq(collection_id))
            .select(world_collections::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load collection"))?
    // "Not found" rather than "not permitted": a caller with no authority over
    // the world must not learn that a collection with this id exists.
    .ok_or_else(|| Error::new("Collection not found"))?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Collection not found"));
    }

    Ok(world_id)
}

/// Testable core of `createCollection` (FR-001).
pub async fn create_collection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CreateCollectionInput,
) -> GraphQLResult<Collection> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new(
            "You must be the DM (Owner or GM) of this world to create a collection in it",
        ));
    }

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(Error::new("A collection needs a name"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_collection = NewCollection {
        id: Uuid::now_v7(),
        world_id: input.world_id,
        name,
        description: input.description,
        created_by: user_id,
        updated_by: user_id,
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_collections::table)
            .values(&new_collection)
            .returning(Collection::as_returning())
            .get_result::<Collection>(&mut conn)
            .map_err(|e| format!("Failed to create collection: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `updateCollection` (FR-005).
pub async fn update_collection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateCollectionInput,
) -> GraphQLResult<Collection> {
    require_collection_authority(state, user_id, is_admin, input.collection_id).await?;

    if let Some(name) = &input.name
        && name.trim().is_empty()
    {
        return Err(Error::new("A collection needs a name"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::update(
            world_collections::table.filter(world_collections::id.eq(input.collection_id)),
        )
        .set((
            input
                .name
                .map(|n| world_collections::name.eq(n.trim().to_string())),
            input
                .description
                .map(|d| world_collections::description.eq(d)),
            world_collections::updated_by.eq(user_id),
            world_collections::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .returning(Collection::as_returning())
        .get_result::<Collection>(&mut conn)
        .map_err(|e| format!("Failed to update collection: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `deleteCollection` (US2 scenario 4).
///
/// Deletes the collection, its membership rows and its share rows — and **no
/// artifacts** (FR-004). The share row going with it is what makes the link
/// stop resolving; FR-009d requires that to be indistinguishable from a code
/// that never existed, which it is, because there is nothing left to find.
pub async fn delete_collection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    collection_id: Uuid,
) -> GraphQLResult<bool> {
    require_collection_authority(state, user_id, is_admin, collection_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(world_collections::table.filter(world_collections::id.eq(collection_id)))
            .execute(&mut conn)
            .map(|rows| rows > 0)
            .map_err(|e| format!("Failed to delete collection: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `addCollectionMember` (FR-001a, FR-003, FR-005a).
pub async fn add_collection_member_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AddCollectionMemberInput,
) -> GraphQLResult<CollectionMember> {
    let world_id =
        require_collection_authority(state, user_id, is_admin, input.collection_id).await?;

    if !is_known_member_type(&input.member_type) {
        return Err(Error::new(format!(
            "A collection cannot hold a {}",
            input.member_type
        )));
    }

    // FR-003: the artifact belongs to this collection's world. Checked before
    // anything else that could reveal something about it.
    let member_world = artifact_world(state, &input.member_type, input.member_id).await?;
    match member_world {
        None => return Err(Error::new("That content could not be found")),
        Some(id) if id != world_id => {
            return Err(Error::new(
                "A collection can only hold content from its own world",
            ));
        }
        Some(_) => {}
    }

    // FR-001a: refuse restricted content, and say why.
    if let Some(reason) =
        membership::restriction_reason(state, &input.member_type, input.member_id).await?
    {
        return Err(Error::new(reason));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let collection_id = input.collection_id;
    let member_type = input.member_type.clone();
    let member_id = input.member_id;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // FR-005a, inside the transaction: two concurrent adds must not
            // both see 99 and both succeed.
            let current: i64 = world_collection_members::table
                .filter(world_collection_members::collection_id.eq(collection_id))
                .count()
                .get_result(conn)?;

            if current >= MAX_MEMBERS {
                // Refused at add time with the limit named — never silently
                // truncated, and never accepted here to fail at copy time.
                return Err(diesel::result::Error::RollbackTransaction);
            }

            let next_sort: i32 = current as i32;

            diesel::insert_into(world_collection_members::table)
                .values(&NewCollectionMember {
                    id: Uuid::now_v7(),
                    collection_id,
                    member_type,
                    member_id,
                    sort_order: next_sort,
                    added_by: user_id,
                })
                .returning(CollectionMember::as_returning())
                .get_result::<CollectionMember>(conn)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| match e {
        diesel::result::Error::RollbackTransaction => Error::new(format!(
            "This collection already holds the maximum of {MAX_MEMBERS} items. \
             Remove something before adding more."
        )),
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        ) => Error::new("That content is already in this collection"),
        other => Error::new(format!("Failed to add to collection: {other}")),
    })
}

/// Testable core of `removeCollectionMember` (FR-004).
///
/// Deletes the membership row and nothing else. The artifact stays where it is.
pub async fn remove_collection_member_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    collection_id: Uuid,
    member_id: Uuid,
) -> GraphQLResult<bool> {
    require_collection_authority(state, user_id, is_admin, collection_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_collection_members::table
                .filter(world_collection_members::collection_id.eq(collection_id))
                .filter(world_collection_members::member_id.eq(member_id)),
        )
        .execute(&mut conn)
        .map(|rows| rows > 0)
        .map_err(|e| format!("Failed to remove from collection: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Which world an artifact belongs to, or `None` if it does not exist.
pub async fn artifact_world(
    state: &AppState,
    member_type: &str,
    member_id: Uuid,
) -> GraphQLResult<Option<Uuid>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let member_type = member_type.to_string();

    tokio::task::spawn_blocking(move || {
        let found: Option<Uuid> = match member_type.as_str() {
            "actor" => {
                use crate::schema::world_actors;
                world_actors::table
                    .filter(world_actors::id.eq(member_id))
                    .select(world_actors::world_id)
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "item" => {
                use crate::schema::world_items;
                world_items::table
                    .filter(world_items::id.eq(member_id))
                    .select(world_items::world_id)
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "ability" => {
                use crate::schema::world_abilities;
                world_abilities::table
                    .filter(world_abilities::id.eq(member_id))
                    .select(world_abilities::world_id)
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "lore" => {
                use crate::schema::world_lore_entries;
                world_lore_entries::table
                    .filter(world_lore_entries::id.eq(member_id))
                    .select(world_lore_entries::world_id)
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "scene" => {
                use crate::schema::scenes;
                scenes::table
                    .filter(scenes::scene_id.eq(member_id))
                    .select(scenes::world_id)
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            other => return Err(format!("Unknown member type: {other}")),
        };
        Ok::<_, String>(found)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `worldCollections` (FR-020's permitted exception).
///
/// One world, to a caller with authority over it. **This is the only listing
/// surface in the feature**, and it must stay that way: ADR-069's
/// determination that a link-shared collection is not a repository rests on
/// there being nothing to enumerate.
pub async fn world_collections_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<Vec<GraphQLCollection>> {
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "You must be the DM (Owner or GM) of this world to see its collections",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let rows = world_collections::table
            .filter(world_collections::world_id.eq(world_id))
            .order(world_collections::created_at.desc())
            .select(Collection::as_select())
            .load::<Collection>(&mut conn)
            .map_err(|e| e.to_string())?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let count: i64 = world_collection_members::table
                .filter(world_collection_members::collection_id.eq(row.id))
                .count()
                .get_result(&mut conn)
                .map_err(|e| e.to_string())?;
            out.push(GraphQLCollection {
                id: row.id,
                world_id: row.world_id,
                name: row.name,
                description: row.description,
                member_count: count as i32,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }
        Ok::<_, String>(out)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `collectionMembers`.
pub async fn collection_members_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    collection_id: Uuid,
) -> GraphQLResult<Vec<GraphQLCollectionMember>> {
    require_collection_authority(state, user_id, is_admin, collection_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_collection_members::table
            .filter(world_collection_members::collection_id.eq(collection_id))
            .order(world_collection_members::sort_order.asc())
            .select(CollectionMember::as_select())
            .load::<CollectionMember>(&mut conn)
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[derive(Default)]
pub struct CollectionQuery;

#[async_graphql::Object]
impl CollectionQuery {
    async fn world_collections(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLCollection>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        world_collections_impl(state, user.user_id, user.is_admin, world_id).await
    }

    async fn collection_members(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLCollectionMember>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        collection_members_impl(state, user.user_id, user.is_admin, collection_id).await
    }
}

#[derive(Default)]
pub struct CollectionMutation;

#[async_graphql::Object]
impl CollectionMutation {
    async fn create_collection(
        &self,
        ctx: &Context<'_>,
        input: CreateCollectionInput,
    ) -> GraphQLResult<GraphQLCollection> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let row = create_collection_impl(state, user.user_id, user.is_admin, input).await?;
        Ok(GraphQLCollection {
            id: row.id,
            world_id: row.world_id,
            name: row.name,
            description: row.description,
            member_count: 0,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn update_collection(
        &self,
        ctx: &Context<'_>,
        input: UpdateCollectionInput,
    ) -> GraphQLResult<GraphQLCollection> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let collection_id = input.collection_id;
        let row = update_collection_impl(state, user.user_id, user.is_admin, input).await?;
        let members =
            collection_members_impl(state, user.user_id, user.is_admin, collection_id).await?;
        Ok(GraphQLCollection {
            id: row.id,
            world_id: row.world_id,
            name: row.name,
            description: row.description,
            member_count: members.len() as i32,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn delete_collection(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        delete_collection_impl(state, user.user_id, user.is_admin, collection_id).await
    }

    async fn add_collection_member(
        &self,
        ctx: &Context<'_>,
        input: AddCollectionMemberInput,
    ) -> GraphQLResult<GraphQLCollectionMember> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        add_collection_member_impl(state, user.user_id, user.is_admin, input)
            .await
            .map(Into::into)
    }

    async fn remove_collection_member(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
        member_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        remove_collection_member_impl(state, user.user_id, user.is_admin, collection_id, member_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    struct Fixture {
        state: AppState,
        owner_id: Uuid,
        player_id: Uuid,
        world_id: Uuid,
        scene_id: Uuid,
        actor_id: Uuid,
        item_id: Uuid,
        ability_id: Uuid,
        lore_id: Uuid,
    }

    /// One world with one of every member type, plus a non-DM member.
    fn fixture() -> Fixture {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");

        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");

        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        let lore_id = insert_test_lore_entry(&mut conn, world_id, owner_id);

        Fixture {
            state,
            owner_id,
            player_id,
            world_id,
            scene_id,
            actor_id,
            item_id,
            ability_id,
            lore_id,
        }
    }

    async fn a_collection(f: &Fixture, name: &str) -> Collection {
        create_collection_impl(
            &f.state,
            f.owner_id,
            false,
            CreateCollectionInput {
                world_id: f.world_id,
                name: name.to_string(),
                description: None,
            },
        )
        .await
        .expect("the DM may create a collection")
    }

    /// FR-001: a DM creates; a Player does not.
    #[tokio::test]
    async fn only_a_dm_may_create_a_collection() {
        let f = fixture();

        create_collection_impl(
            &f.state,
            f.player_id,
            false,
            CreateCollectionInput {
                world_id: f.world_id,
                name: "Player's attempt".to_string(),
                description: None,
            },
        )
        .await
        .expect_err("a Player must not create a collection");

        let collection = a_collection(&f, "The Haunted Manor").await;
        assert_eq!(collection.name, "The Haunted Manor");
        assert_eq!(collection.created_by, f.owner_id);
        assert_eq!(collection.updated_by, f.owner_id);
    }

    /// FR-002: all five member types go in.
    #[tokio::test]
    async fn a_collection_holds_every_member_type() {
        let f = fixture();
        let collection = a_collection(&f, "Everything").await;

        for (member_type, member_id) in [
            ("actor", f.actor_id),
            ("item", f.item_id),
            ("ability", f.ability_id),
            ("lore", f.lore_id),
            ("scene", f.scene_id),
        ] {
            add_collection_member_impl(
                &f.state,
                f.owner_id,
                false,
                AddCollectionMemberInput {
                    collection_id: collection.id,
                    member_type: member_type.to_string(),
                    member_id,
                },
            )
            .await
            .unwrap_or_else(|e| panic!("{member_type} must be addable: {e:?}"));
        }

        let members = collection_members_impl(&f.state, f.owner_id, false, collection.id)
            .await
            .expect("members list");
        assert_eq!(members.len(), 5, "the collection lists exactly its members");
    }

    /// FR-003: a collection holds artifacts from its own world only.
    #[tokio::test]
    async fn an_artifact_from_another_world_is_refused() {
        let f = fixture();
        let mut conn = f.state.db_pool.get().expect("connection");
        let other_world = insert_test_world(&mut conn, f.owner_id);
        let foreign_item = insert_test_item(&mut conn, other_world, f.owner_id);
        drop(conn);

        let collection = a_collection(&f, "Cross-world attempt").await;

        let error = add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "item".to_string(),
                member_id: foreign_item,
            },
        )
        .await
        .expect_err("a foreign artifact must be refused");
        assert!(
            error.message.contains("own world"),
            "the refusal must say why, got: {}",
            error.message
        );
    }

    /// FR-001a: a GM-only ability is refused, and the refusal explains itself
    /// rather than reading as a generic failure.
    #[tokio::test]
    async fn a_restricted_artifact_is_refused_with_its_reason() {
        use crate::schema::world_abilities;

        let f = fixture();
        let mut conn = f.state.db_pool.get().expect("connection");
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(f.ability_id)))
            .set(world_abilities::gm_only.eq(true))
            .execute(&mut conn)
            .expect("hide the ability");
        drop(conn);

        let collection = a_collection(&f, "Restricted attempt").await;

        let error = add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "ability".to_string(),
                member_id: f.ability_id,
            },
        )
        .await
        .expect_err("a GM-only ability must be refused");
        assert!(
            error.message.contains("Game Master"),
            "the refusal must carry the reason, got: {}",
            error.message
        );
    }

    /// FR-004: removing a member deletes the membership, never the artifact.
    #[tokio::test]
    async fn removing_a_member_leaves_the_artifact_alone() {
        use crate::schema::world_items;

        let f = fixture();
        let collection = a_collection(&f, "Removable").await;

        add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "item".to_string(),
                member_id: f.item_id,
            },
        )
        .await
        .expect("added");

        assert!(
            remove_collection_member_impl(&f.state, f.owner_id, false, collection.id, f.item_id)
                .await
                .expect("removed")
        );

        let mut conn = f.state.db_pool.get().expect("connection");
        let still_there: i64 = world_items::table
            .filter(world_items::id.eq(f.item_id))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(still_there, 1, "the artifact must survive being removed");
    }

    /// FR-005a: the 101st member is refused, with the limit named.
    ///
    /// Uses the same item repeatedly? No — the unique constraint forbids it,
    /// so this inserts real rows. It is slower and it is the only way to
    /// exercise the count the limit actually reads.
    #[tokio::test]
    async fn the_hundred_and_first_member_is_refused_with_the_limit_named() {
        let f = fixture();
        let collection = a_collection(&f, "At the limit").await;

        let mut conn = f.state.db_pool.get().expect("connection");
        let mut ids = Vec::new();
        for _ in 0..(MAX_MEMBERS + 1) {
            ids.push(insert_test_item(&mut conn, f.world_id, f.owner_id));
        }
        drop(conn);

        for (index, item_id) in ids.iter().enumerate().take(MAX_MEMBERS as usize) {
            add_collection_member_impl(
                &f.state,
                f.owner_id,
                false,
                AddCollectionMemberInput {
                    collection_id: collection.id,
                    member_type: "item".to_string(),
                    member_id: *item_id,
                },
            )
            .await
            .unwrap_or_else(|e| panic!("member {index} must be addable: {e:?}"));
        }

        let error = add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "item".to_string(),
                member_id: ids[MAX_MEMBERS as usize],
            },
        )
        .await
        .expect_err("the 101st must be refused");
        assert!(
            error.message.contains(&MAX_MEMBERS.to_string()),
            "the refusal must name the limit, got: {}",
            error.message
        );
    }

    /// Adding the same artifact twice is refused rather than duplicated.
    #[tokio::test]
    async fn the_same_artifact_cannot_be_added_twice() {
        let f = fixture();
        let collection = a_collection(&f, "No duplicates").await;

        let input = AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "item".to_string(),
            member_id: f.item_id,
        };

        add_collection_member_impl(&f.state, f.owner_id, false, input.clone())
            .await
            .expect("first add");
        let error = add_collection_member_impl(&f.state, f.owner_id, false, input)
            .await
            .expect_err("second add must be refused");
        assert!(error.message.contains("already"), "got: {}", error.message);
    }

    /// FR-020: a caller with no authority over the world learns nothing —
    /// including whether the collection exists. The refusal is "not found",
    /// not "not permitted", because the second answer is itself information.
    #[tokio::test]
    async fn a_non_dm_cannot_see_or_touch_a_collection() {
        let f = fixture();
        let collection = a_collection(&f, "Private").await;

        let error = collection_members_impl(&f.state, f.player_id, false, collection.id)
            .await
            .expect_err("a Player must not read a collection's members");
        assert!(
            error.message.contains("not found"),
            "the refusal must not confirm the collection exists, got: {}",
            error.message
        );

        world_collections_impl(&f.state, f.player_id, false, f.world_id)
            .await
            .expect_err("a Player must not list the world's collections");
    }

    /// US2 scenario 4: deleting a collection deletes no artifacts.
    #[tokio::test]
    async fn deleting_a_collection_deletes_no_artifacts() {
        use crate::schema::world_items;

        let f = fixture();
        let collection = a_collection(&f, "Doomed").await;
        add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "item".to_string(),
                member_id: f.item_id,
            },
        )
        .await
        .expect("added");

        assert!(
            delete_collection_impl(&f.state, f.owner_id, false, collection.id)
                .await
                .expect("deleted")
        );

        let mut conn = f.state.db_pool.get().expect("connection");
        let item_survives: i64 = world_items::table
            .filter(world_items::id.eq(f.item_id))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(item_survives, 1, "the artifact must survive the collection");
    }
}
