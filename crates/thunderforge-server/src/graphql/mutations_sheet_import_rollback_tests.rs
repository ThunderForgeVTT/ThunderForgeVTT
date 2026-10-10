//! Spec 048 T074 (User Story 5): the GM rolls a bad import back. The sheet's
//! fields and links return to what they were before it; what happened at
//! the table since stays (FR-044a), and only the GM may do it (FR-044b).

use super::*;
use crate::compendium::origin::ContentOrigin;
use crate::sheet_import::error::SheetImportError;
use crate::sheet_import::rollback::roll_back_actor_impl;
use crate::sheet_import::{ActorImport, ActorImportKind};

impl Table {
    async fn roll_back(&self, user: Uuid, to: Uuid) -> Result<ActorImport, SheetImportError> {
        roll_back_actor_impl(&self.state, &systems_dir(), user, false, self.actor, to).await
    }

    fn label_and_origin(&self) -> (String, ContentOrigin) {
        use crate::schema::world_actors;
        let mut conn = self.state.db_pool.get().expect("conn");
        world_actors::table
            .find(self.actor)
            .select((world_actors::label, world_actors::origin))
            .first(&mut conn)
            .unwrap()
    }

    fn rolled_back_events(&self) -> Vec<Value> {
        use crate::schema::world_events;
        let mut conn = self.state.db_pool.get().expect("conn");
        world_events::table
            .filter(world_events::world_id.eq(self.world))
            .filter(world_events::event_code.eq(crate::world_events::EVENT_CODE_ACTOR_ROLLED_BACK))
            .select(world_events::token_event)
            .load::<Option<Value>>(&mut conn)
            .unwrap()
            .into_iter()
            .flatten()
            .collect()
    }
}

#[tokio::test]
async fn only_the_world_s_gm_rolls_a_character_back() {
    let t = table();
    let owner = t.claimant().await;
    let first = t.bring(owner, FIGHTER_WIZARD, &[]).await;
    let trusted = t.member("TrustedPlayer");
    t.grant(trusted, "Editor");
    for (who, user) in [("the owner", owner), ("a Trusted Player", trusted)] {
        let refused = t.roll_back(user, first.id).await.unwrap_err();
        assert_eq!(refused.code(), "FORBIDDEN", "{who}: {refused:?}");
    }
    assert!(
        t.rolled_back_events().is_empty(),
        "a refusal writes nothing"
    );

    // Another actor's import is not this actor's to roll back to.
    let other = {
        let mut conn = t.state.db_pool.get().expect("conn");
        let scene = crate::schema::world_actors::table
            .find(t.actor)
            .select(crate::schema::world_actors::scene_id)
            .first(&mut conn)
            .unwrap();
        insert_test_actor(&mut conn, t.world, scene, t.gm)
    };
    let refused = roll_back_actor_impl(&t.state, &systems_dir(), t.gm, false, other, first.id)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), "VALIDATION_FAILED", "{refused:?}");

    t.roll_back(t.gm, first.id)
        .await
        .expect("the GM rolls back");
}

#[tokio::test]
async fn a_rollback_restores_the_sheet_and_keeps_the_table_s_play() {
    let t = table();
    let player = t.claimant().await;
    t.bring(player, FIGHTER_WIZARD, &[]).await;
    let links_at_five = t.ability_links();
    let max_hp_at_five = t.resource("max_hp");
    let label_at_five = t.label_and_origin().0;

    // The bad import, then a session played on it.
    let bad = t.bring(player, LEVEL_SIX, &[]).await;
    assert_ne!(t.ability_links(), links_at_five, "level six links more");
    assert_ne!(t.resource("max_hp"), max_hp_at_five);
    t.play();

    let rollback = t.roll_back(t.gm, bad.id).await.expect("rolled back");
    assert_eq!(rollback.kind, ActorImportKind::Rollback);
    assert_eq!(rollback.restored_from, Some(bad.id));
    assert_eq!(rollback.version_id, None);

    assert_eq!(
        t.ability_links(),
        links_at_five,
        "the links are level five's"
    );
    assert_eq!(
        t.resource("max_hp"),
        max_hp_at_five,
        "the sheet field is back"
    );
    let (label, origin) = t.label_and_origin();
    assert_eq!(label, label_at_five);
    assert_eq!(origin, ContentOrigin::Uploaded, "origin stays Uploaded");

    // The table's play survives.
    assert_eq!(t.resource("current_hp"), json!(7));
    assert_eq!(t.resource("death_save_failures"), json!(1));
    assert_eq!(t.resource("spell_slots_used"), json!({ "level_1": 2 }));

    let history = actor_imports_impl(&t.state, t.gm, false, t.actor)
        .await
        .unwrap();
    let kinds: Vec<_> = history.iter().map(|h| h.import.kind).collect();
    assert_eq!(
        kinds,
        [
            ActorImportKind::Rollback,
            ActorImportKind::Import,
            ActorImportKind::Import
        ]
    );
    assert_eq!(history[0].version_no, None);
    assert!(!history[0].file_available, "a rollback has no file");

    let events = t.rolled_back_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["actorId"], json!(t.actor));
    assert_eq!(events[0]["importId"], json!(rollback.id));
}

#[tokio::test]
async fn rolling_back_to_the_first_import_restores_the_actor_before_any() {
    let t = table();
    let before = (
        t.label_and_origin().0,
        t.ability_links(),
        t.resource("max_hp"),
    );
    let player = t.claimant().await;
    let first = t.bring(player, FIGHTER_WIZARD, &[]).await;
    t.bring(player, LEVEL_SIX, &[]).await;
    assert_ne!(t.ability_links(), before.1);

    t.roll_back(t.gm, first.id).await.expect("rolled back");
    let after = (
        t.label_and_origin().0,
        t.ability_links(),
        t.resource("max_hp"),
    );
    assert_eq!(after, before, "the actor as it was before any import");
    assert_eq!(t.label_and_origin().1, ContentOrigin::Uploaded);
}
