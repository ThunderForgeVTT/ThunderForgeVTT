//! Spec 048 T071 (User Story 5): a character comes back changed. The same
//! invented fighter-wizard at level 5 and then at level 6, brought onto the
//! actor that holds the first import.

use super::*;
use thunderforge_sheet_import::{ImportPlan, PlanCertainty, Resolution};

const LEVEL_SIX: &[u8] =
    include_bytes!("../../../../packs/systems/dnd5e/sheet/tests/fixtures/fighter3-wizard2-l6.pdf");

impl Table {
    async fn bring(
        &self,
        user: Uuid,
        bytes: &[u8],
        overwrite: &[&str],
    ) -> crate::sheet_import::ActorImport {
        let plan = self.plan_of(user, bytes, None).await.expect("a plan");
        apply_sheet_import_impl(
            &self.state,
            &systems_dir(),
            user,
            false,
            ApplyInput {
                actor_id: self.actor,
                bytes: bytes.to_vec(),
                corrections: None,
                overwrite_play_state: overwrite.iter().map(|t| t.to_string()).collect(),
                plan_hash: plan_hash(&plan),
            },
        )
        .await
        .unwrap_or_else(|e| panic!("applied: {} {e:?}", e.code()))
    }

    /// The table plays: the character takes damage and spends a slot.
    fn play(&self) {
        use crate::schema::world_actor_system_data as data;
        let mut conn = self.state.db_pool.get().expect("conn");
        let (resources, spells): (Option<Value>, Option<Value>) = data::table
            .filter(data::actor_id.eq(self.actor))
            .select((data::resource_data, data::spell_data))
            .first(&mut conn)
            .unwrap();
        let mut resources = resources.unwrap_or_else(|| json!({}));
        resources["current_hp"] = json!(7);
        resources["death_save_failures"] = json!(1);
        let mut spells = spells.unwrap_or_else(|| json!({}));
        spells["spell_slots_used"] = json!({ "level_1": 2 });
        diesel::update(data::table.filter(data::actor_id.eq(self.actor)))
            .set((
                data::resource_data.eq(Some(resources)),
                data::spell_data.eq(Some(spells)),
            ))
            .execute(&mut conn)
            .expect("play written");
    }

    fn resource(&self, key: &str) -> Value {
        use crate::schema::world_actor_system_data as data;
        let mut conn = self.state.db_pool.get().expect("conn");
        let (resources, spells): (Option<Value>, Option<Value>) = data::table
            .filter(data::actor_id.eq(self.actor))
            .select((data::resource_data, data::spell_data))
            .first(&mut conn)
            .unwrap();
        let resources = resources.unwrap_or_default();
        let spells = spells.unwrap_or_default();
        resources
            .get(key)
            .or_else(|| spells.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn ability_links(&self) -> Vec<String> {
        use crate::schema::world_actor_abilities as links;
        let mut conn = self.state.db_pool.get().expect("conn");
        let mut names: Vec<String> = links::table
            .filter(links::actor_id.eq(self.actor))
            .select(links::ability_name_snapshot)
            .load(&mut conn)
            .unwrap();
        names.sort();
        names
    }

    /// The staged spell of that name (a Shield is also a fighter's item).
    fn staged_spell(&self, name: &str) -> Uuid {
        use crate::schema::world_staged_content as staged;
        let mut conn = self.state.db_pool.get().expect("conn");
        staged::table
            .filter(staged::world_id.eq(self.world))
            .filter(staged::name.eq(name))
            .filter(staged::kind.ne("item"))
            .select(staged::id)
            .first(&mut conn)
            .unwrap_or_else(|e| panic!("{name} is staged: {e}"))
    }
}

fn changed(plan: &ImportPlan) -> Vec<&str> {
    plan.fields.iter().map(|f| f.path.as_str()).collect()
}

#[tokio::test]
async fn a_reimport_changes_only_what_differs_into_the_next_version() {
    use crate::schema::sheet_import_versions as versions;
    let t = table();
    let player = t.claimant().await;
    let first = t.bring(player, FIGHTER_WIZARD, &[]).await;

    let plan = t.plan_of(player, LEVEL_SIX, None).await.expect("a plan");
    assert!(plan.is_reimport);
    // A value the reader is sure of is listed only when it changes; one it
    // doubts or could not read stays, so the person can correct it.
    for field in plan
        .fields
        .iter()
        .filter(|f| f.certainty == PlanCertainty::Read)
    {
        assert_ne!(field.old, field.new, "{} is no change", field.path);
    }
    let paths = changed(&plan);
    assert!(paths.iter().any(|p| p.starts_with("classes")), "{paths:?}");
    assert!(paths.iter().any(|p| p.contains("hp_max")), "{paths:?}");
    assert!(
        !paths.iter().any(|p| p.starts_with("abilities.")),
        "{paths:?}"
    );
    assert!(plan.identical.iter().any(|p| p == "abilities.str"));

    let second = t.bring(player, LEVEL_SIX, &[]).await;
    let mut conn = t.state.db_pool.get().expect("conn");
    let rows: Vec<(Uuid, i32)> = versions::table
        .filter(versions::id.eq_any([first.version_id.unwrap(), second.version_id.unwrap()]))
        .select((versions::character_id, versions::version_no))
        .order(versions::version_no.asc())
        .load(&mut conn)
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, rows[1].0, "the same brought character");
    assert_eq!((rows[0].1, rows[1].1), (1, 2));
    drop(conn);
    let history = actor_imports_impl(&t.state, player, false, t.actor)
        .await
        .unwrap();
    let mut numbers: Vec<_> = history.iter().map(|h| h.version_no).collect();
    numbers.sort();
    assert_eq!(numbers, vec![Some(1), Some(2)]);
}

#[tokio::test]
async fn play_state_is_kept_unless_the_person_names_it() {
    let t = table();
    let player = t.claimant().await;
    t.bring(player, FIGHTER_WIZARD, &[]).await;
    t.play();

    let plan = t.plan_of(player, LEVEL_SIX, None).await.expect("a plan");
    let kept: Vec<&str> = plan
        .kept_in_play
        .iter()
        .map(|k| k.target.as_str())
        .collect();
    assert!(
        kept.contains(&"resource_data.death_save_failures"),
        "{kept:?}"
    );
    t.bring(player, LEVEL_SIX, &[]).await;
    assert_eq!(
        t.resource("current_hp"),
        json!(7),
        "the table's hit points survive"
    );
    assert_eq!(t.resource("death_save_failures"), json!(1));
    assert_eq!(t.resource("spell_slots_used"), json!({ "level_1": 2 }));

    // Named, it is the sheet's again, and only what was named.
    t.bring(
        player,
        FIGHTER_WIZARD,
        &["resource_data.death_save_failures"],
    )
    .await;
    assert_eq!(t.resource("death_save_failures"), json!(0));
    assert_eq!(t.resource("current_hp"), json!(7), "only what was named");
}

#[tokio::test]
async fn content_adopted_since_uses_the_world_s_and_what_left_the_sheet_goes_on_accept() {
    let t = table();
    let player = t.claimant().await;
    t.bring(player, LEVEL_SIX, &[]).await;
    let shield = t.staged_spell("Shield");
    crate::staged_content::decide::adopt(&mut t.state.db_pool.get().unwrap(), t.gm, shield)
        .expect("the GM adopts Shield");

    // Back to level 5: the second-level spells are no longer on the sheet.
    let plan = t
        .plan_of(player, FIGHTER_WIZARD, None)
        .await
        .expect("a plan");
    let shield_change = plan
        .content
        .iter()
        .find(|c| c.kind == "spell" && c.name == "Shield")
        .expect("Shield is planned");
    assert!(
        matches!(shield_change.resolution, Resolution::World { .. }),
        "{:?}",
        shield_change.resolution
    );
    let removed: Vec<&str> = plan
        .content
        .iter()
        .filter(|c| c.removed)
        .map(|c| c.name.as_str())
        .collect();
    assert!(!removed.is_empty(), "the level-six spells leave");
    let before = t.ability_links();
    for name in &removed {
        assert!(
            before.iter().any(|l| l == name),
            "{name} is linked until accepted"
        );
    }

    // Declined: a preview unlinks nothing.
    assert_eq!(t.ability_links(), before);
    t.bring(player, FIGHTER_WIZARD, &[]).await;
    let after = t.ability_links();
    for name in &removed {
        assert!(
            !after.iter().any(|l| l == name),
            "{name} is unlinked on accept"
        );
    }
    assert!(after.iter().any(|l| l == "Shield"));
}
