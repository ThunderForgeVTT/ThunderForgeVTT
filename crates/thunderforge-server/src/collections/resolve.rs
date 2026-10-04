//! Resolving one member of a collection, freshly, on every read.
//!
//! # Why nothing here is cached
//!
//! `world_collection_members` carries no `disabled` and no `restricted`
//! column, and this module is why. A cached status is stale in **both**
//! directions: it can serve an artifact that has since been taken down, and it
//! can withhold one that FR-025 says should have returned when a counter-notice
//! period elapsed. `moderation::effective_status` already restores lazily at
//! read time, so asking it every time is both simpler and more correct than
//! any invalidation scheme.
//!
//! # Three ways a member can be absent, one way it is shown
//!
//! A member may be moderated (FR-021), restricted after the fact (FR-001b), or
//! simply gone — its artifact or its whole world deleted. All three are
//! withheld, and all three are shown the same way: *something* has been
//! withheld, never *what* (FR-022). Reproducing the name of a taken-down
//! artifact in the sentence explaining that it was taken down would defeat the
//! takedown.

use async_graphql::Result as GraphQLResult;
use diesel::prelude::*;
use uuid::Uuid;

use crate::collections::{membership, moderation_entity_type};
use crate::models::CollectionMember;
use crate::state::AppState;

/// What a member turned out to be, at the moment it was asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberResolution {
    /// Present and shareable. Carries the display name for the preview.
    Visible { name: String },
    /// Present but not shareable — moderated, or restricted after being added.
    Withheld,
    /// The artifact no longer exists, or its world does not.
    ///
    /// Distinct from `Withheld` internally so the copy path can tell a
    /// deliberate withholding from a dangling reference when it writes a
    /// fidelity note. **Not distinct to a viewer**: both read as "something has
    /// been withheld".
    Gone,
}

impl MemberResolution {
    pub fn is_visible(&self) -> bool {
        matches!(self, MemberResolution::Visible { .. })
    }
}

/// Resolve one member: does it exist, is it moderated, is it restricted now?
pub async fn resolve_member(
    state: &AppState,
    member: &CollectionMember,
) -> GraphQLResult<MemberResolution> {
    // 1. Does it still exist, and what is it called?
    let Some(name) = load_name(state, &member.member_type, member.member_id).await? else {
        return Ok(MemberResolution::Gone);
    };

    // 2. Has moderation disabled it? A share must never become a moderation
    //    bypass — the property spec 025's shares guard with the same call, and
    //    the one ADR-069's determination leans on hardest.
    if let Some(entity_type) = moderation_entity_type(&member.member_type)
        && crate::moderation::effective_status(state, entity_type, member.member_id)
            .await?
            .is_some()
    {
        return Ok(MemberResolution::Withheld);
    }

    // 3. FR-001b: has it become restricted since it was added? A check that
    //    ran only at add time would make FR-001a a gate with a way around it.
    if membership::restriction_reason(state, &member.member_type, member.member_id)
        .await?
        .is_some()
    {
        return Ok(MemberResolution::Withheld);
    }

    Ok(MemberResolution::Visible { name })
}

/// The artifact's display name, or `None` if it no longer exists.
async fn load_name(
    state: &AppState,
    member_type: &str,
    member_id: Uuid,
) -> GraphQLResult<Option<String>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

    let member_type = member_type.to_string();

    tokio::task::spawn_blocking(move || {
        let found: Option<String> = match member_type.as_str() {
            "actor" => {
                use crate::schema::world_actors;
                world_actors::table
                    .filter(world_actors::id.eq(member_id))
                    .select(world_actors::label)
                    .first::<String>(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "item" => {
                use crate::schema::world_items;
                world_items::table
                    .filter(world_items::id.eq(member_id))
                    .select(world_items::name)
                    .first::<String>(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "ability" => {
                use crate::schema::world_abilities;
                world_abilities::table
                    .filter(world_abilities::id.eq(member_id))
                    .select(world_abilities::name)
                    .first::<String>(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "lore" => {
                use crate::schema::world_lore_entries;
                world_lore_entries::table
                    .filter(world_lore_entries::id.eq(member_id))
                    .select(world_lore_entries::title)
                    .first::<String>(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            "scene" => {
                use crate::schema::scenes;
                scenes::table
                    .filter(scenes::scene_id.eq(member_id))
                    .select(scenes::name)
                    .first::<String>(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?
            }
            other => return Err(format!("Unknown member type: {other}")),
        };
        Ok::<_, String>(found)
    })
    .await
    .map_err(|_| async_graphql::Error::new("Failed to spawn blocking task"))?
    .map_err(async_graphql::Error::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn member_row(collection_id: Uuid, member_type: &str, member_id: Uuid) -> CollectionMember {
        CollectionMember {
            id: Uuid::now_v7(),
            collection_id,
            member_type: member_type.to_string(),
            member_id,
            sort_order: 0,
            added_by: Uuid::now_v7(),
            created_at: chrono::Utc::now().naive_utc(),
        }
    }

    #[tokio::test]
    async fn a_present_unmoderated_member_resolves_visible() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);

        let resolution = resolve_member(&state, &member_row(Uuid::now_v7(), "item", item_id))
            .await
            .expect("resolves");
        assert!(resolution.is_visible(), "got {resolution:?}");
    }

    /// The edge case the schema's missing foreign key exists for: a member
    /// deleted from its world must not make the collection unopenable.
    #[tokio::test]
    async fn a_deleted_artifact_resolves_gone_rather_than_erroring() {
        let state = test_app_state();
        let resolution =
            resolve_member(&state, &member_row(Uuid::now_v7(), "item", Uuid::now_v7()))
                .await
                .expect("a dangling member must resolve, not error");
        assert_eq!(resolution, MemberResolution::Gone);
    }

    /// FR-001b: restricting an artifact *after* it was added withholds it.
    /// This is the half that fails if the restriction check runs only at add
    /// time — which is the obvious way to write it.
    #[tokio::test]
    async fn an_ability_restricted_after_being_added_is_withheld() {
        use crate::schema::world_abilities;

        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

        let member = member_row(Uuid::now_v7(), "ability", ability_id);
        assert!(
            resolve_member(&state, &member)
                .await
                .expect("resolves")
                .is_visible(),
            "unrestricted to begin with"
        );

        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set(world_abilities::gm_only.eq(true))
            .execute(&mut conn)
            .expect("restrict it");

        assert_eq!(
            resolve_member(&state, &member).await.expect("resolves"),
            MemberResolution::Withheld,
            "an artifact restricted after being added must be withheld (FR-001b)"
        );

        // And the reverse, which FR-025's sibling edge case requires: lifting
        // the restriction returns the member without the owner rebuilding
        // anything.
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set(world_abilities::gm_only.eq(false))
            .execute(&mut conn)
            .expect("unrestrict it");

        assert!(
            resolve_member(&state, &member)
                .await
                .expect("resolves")
                .is_visible(),
            "lifting a restriction must return the member"
        );
    }
}
