//! FR-001a / FR-001b: what may not enter a collection, and what is withheld
//! from one after the fact.
//!
//! # What "restricted to a subset of a world's members" actually means here
//!
//! The spec's clarification asked about "a lore entry restricted to only some
//! of a world's members". **That category does not exist for lore**, and the
//! answer matters enough to write down rather than discover twice.
//!
//! The permission ladder — `Viewer`/`Editor`/`Owner`, resolved by
//! `auth::permissioned_entities` — **cannot express a restriction**. `Viewer`
//! is both its floor and its default: `queries/lore.rs` states outright that
//! "every caller — member or not — defaults to `Viewer` when no explicit row
//! exists". A grant row therefore *elevates* one member; it never withholds
//! from the others. `permissioned_entities.rs` says the same thing from the
//! other side, warning at length that visibility is a separate axis and that
//! the macro "must never gain a visibility parameter 'for symmetry'".
//!
//! So the real restriction axes, verified against the schema rather than
//! assumed, are these:
//!
//! | Member type | Axis | Meaning |
//! |---|---|---|
//! | ability | `world_abilities.gm_only` | Only the world's DMs may see it — **a restriction** |
//! | scene | `scenes.hidden` | Not yet revealed to players — **staging, not a restriction**; see below |
//! | item | *none* | Every member sees every item |
//! | lore | *none* | Every member sees every lore entry |
//! | actor | *none* | `world_actors.is_public` exists but gates nothing — it is never read as a visibility check anywhere in the server |
//!
//! Four of the five types are therefore vacuous today. **That is not a reason
//! to check only one.** This function is exhaustive over `MEMBER_TYPES` with
//! an explicit arm per type, so adding a restriction axis to items or lore
//! later lands in a function that already has somewhere to put it, next to a
//! test that already covers the type. A check written only for the type that
//! needs it today is a check the others silently escape.
//!
//! # Why `scenes.hidden` is not treated as a restriction
//!
//! This was implemented as a refusal first, and a test caught it:
//! `hidden BOOLEAN NOT NULL DEFAULT true`. **Every scene is hidden when it is
//! created.** Refusing hidden scenes would have refused very nearly every
//! scene in the product, which defeats FR-002 — scenes are in scope precisely
//! because the flagship case is sharing a *place*, and a haunted manor without
//! its rooms is a list of nouns.
//!
//! The deeper point is that the two flags are not the same kind of thing, even
//! though `world_abilities`' migration says `gm_only` "mirrors scenes.hidden".
//! `gm_only` defaults to **false**: setting it is a deliberate act meaning
//! *this is secret from my players*. `hidden` defaults to **true**: it is
//! play-staging state meaning *I have not revealed this room yet*, and clearing
//! it is the deliberate act. A flag that is on until someone turns it off is
//! not a statement anybody made.
//!
//! Treating it as a restriction would also force a worse outcome than it
//! prevents: to share a scene, an owner would first have to unhide it **in
//! their own world**, revealing it to their players mid-campaign as a side
//! effect of sharing it with someone else. The feature would be asking them to
//! spoil their own game.
//!
//! What makes this safe is that adding a scene to a collection and sharing
//! that collection are both deliberate acts by someone with authority over the
//! world. The owner is choosing to publish it. That is the same standard every
//! other member type is held to.

use async_graphql::{Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::state::AppState;

/// Why this artifact may not be shared, in the words shown to the person
/// trying to share it. `None` means it may.
///
/// A **refusal**, not a warning (FR-001a). A collection is read by anyone
/// holding its link — ADR-070 makes that anyone at all, signed out — so a
/// restricted artifact placed in one is published to strangers, and that is
/// the single failure in this feature its owner cannot undo. Nothing is
/// forced by refusing: an owner who wants to share something GM-only may
/// clear that flag first, which is a deliberate act rather than a side effect
/// of adding to a list.
pub async fn restriction_reason(
    state: &AppState,
    member_type: &str,
    member_id: Uuid,
) -> GraphQLResult<Option<String>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let member_type = member_type.to_string();

    tokio::task::spawn_blocking(move || {
        match member_type.as_str() {
            "ability" => {
                use crate::schema::world_abilities;
                let gm_only: Option<bool> = world_abilities::table
                    .filter(world_abilities::id.eq(member_id))
                    .select(world_abilities::gm_only)
                    .first::<bool>(&mut conn)
                    .optional()
                    .map_err(|e| e.to_string())?;
                Ok(gm_only.and_then(|flag| {
                    flag.then(|| {
                        "This ability is visible only to the Game Master, so it \
                         cannot be shared in a collection. Anyone holding the \
                         collection's link would be able to read it. Clear the \
                         GM-only setting first if you mean to share it."
                            .to_string()
                    })
                }))
            }
            // Scenes are NOT gated on `hidden`. It defaults to true, so
            // gating on it would refuse nearly every scene, and it is
            // play-staging state rather than a statement about who may see the
            // content. The module documentation carries the full argument —
            // read it before "fixing" this arm.
            "scene" => Ok(None),
            // The three vacuous arms, written out rather than folded into a
            // catch-all. Every member of a world can already see every item,
            // lore entry and actor in it, so there is no subset to publish
            // past. If that ever changes, the change lands here.
            "item" | "lore" | "actor" => Ok(None),
            other => Err(format!("Unknown member type: {other}")),
        }
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    /// SC-003a says "verified across every artifact type rather than sampled",
    /// so this walks all five rather than the two that can currently refuse.
    #[tokio::test]
    async fn every_member_type_is_answered_and_unrestricted_content_passes() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");

        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);

        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        let lore_id = insert_test_lore_entry(&mut conn, world_id, owner_id);

        for (member_type, id) in [
            ("actor", actor_id),
            ("item", item_id),
            ("ability", ability_id),
            ("lore", lore_id),
            ("scene", scene_id),
        ] {
            let reason = restriction_reason(&state, member_type, id)
                .await
                .unwrap_or_else(|e| panic!("{member_type} must be answered, not error: {e:?}"));
            assert!(
                reason.is_none(),
                "unrestricted {member_type} must be shareable, got {reason:?}"
            );
        }
    }

    /// The ability axis: `gm_only` is a genuine restriction to the DMs.
    #[tokio::test]
    async fn a_gm_only_ability_is_refused_with_a_reason() {
        use crate::schema::world_abilities;

        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");

        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set(world_abilities::gm_only.eq(true))
            .execute(&mut conn)
            .expect("hide the ability");

        let reason = restriction_reason(&state, "ability", ability_id)
            .await
            .expect("resolves")
            .expect("a GM-only ability must be refused");
        assert!(
            reason.contains("Game Master"),
            "the refusal must say why, got: {reason}"
        );
    }

    /// A hidden scene is shareable, and this test is the reason the module
    /// documentation argues the point at length.
    ///
    /// `scenes.hidden` defaults to **true**, so gating on it would refuse
    /// nearly every scene in the product and force an owner to reveal a room
    /// to their own players in order to share it with someone else. The first
    /// implementation did gate on it; this test is what found that.
    #[tokio::test]
    async fn a_hidden_scene_is_still_shareable_because_hidden_is_staging_state() {
        use crate::schema::scenes;

        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");

        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);

        let hidden: bool = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::hidden)
            .first(&mut conn)
            .expect("load the scene");
        assert!(
            hidden,
            "a freshly created scene is hidden by default — if this ever \
             changes, the argument in this module needs revisiting"
        );

        assert!(
            restriction_reason(&state, "scene", scene_id)
                .await
                .expect("resolves")
                .is_none(),
            "a hidden scene must remain shareable"
        );
    }

    /// An unknown type is an error, not a silent pass. A typo in a member type
    /// must not become "nothing restricts this".
    #[tokio::test]
    async fn an_unknown_member_type_errors_rather_than_permitting() {
        let state = test_app_state();
        let result = restriction_reason(&state, "spaceship", Uuid::now_v7()).await;
        assert!(result.is_err(), "an unknown member type must not pass");
    }

    /// A member that no longer exists is not "restricted" — it is gone, which
    /// is `resolve`'s question rather than this one's. Asserted so that a
    /// future change making a missing row refuse here does not silently turn
    /// a deleted artifact into an unexplained share failure.
    #[tokio::test]
    async fn a_missing_artifact_is_not_reported_as_restricted() {
        let state = test_app_state();
        let reason = restriction_reason(&state, "ability", Uuid::now_v7())
            .await
            .expect("resolves");
        assert!(
            reason.is_none(),
            "a missing artifact is gone, not restricted"
        );
    }
}
