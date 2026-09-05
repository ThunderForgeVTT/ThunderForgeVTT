use super::*;
use crate::test_support::*;

fn ability_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
    CreateAbilityInput {
        world_id,
        name: name.to_string(),
        description: None,
        classification: "spell".to_string(),
        grade: None,
        gm_only: None,
    }
}

fn effect_input(formula: &str, target: &str) -> AbilityEffectInput {
    AbilityEffectInput {
        effect_type: AbilityEffectType::Damage,
        formula: formula.to_string(),
        target: target.to_string(),
        trigger_kind: None,
        sort_order: None,
    }
}

/// FR-018: an empty/whitespace-only formula errors before any write.
#[tokio::test]
async fn add_ability_effect_rejects_empty_formula() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Bolt"))
        .await
        .unwrap();

    let err = add_ability_effect_impl(
        &state,
        owner_id,
        false,
        ability.id,
        effect_input("   ", "Hit Points"),
    )
    .await
    .expect_err("a whitespace-only formula must be rejected");
    assert!(err.message.contains("must not be empty"));

    // A formula with no alphanumeric content is also structurally invalid.
    let err = add_ability_effect_impl(
        &state,
        owner_id,
        false,
        ability.id,
        effect_input("+++", "Hit Points"),
    )
    .await
    .expect_err("a formula with no letters or digits must be rejected");
    assert!(err.message.contains("at least one letter or digit"));

    // Nothing was persisted by either rejection.
    assert!(
        load_ability_effects(&state, ability.id)
            .await
            .unwrap()
            .is_empty(),
        "a rejected effect must not be written"
    );

    // An empty target is rejected too.
    add_ability_effect_impl(
        &state,
        owner_id,
        false,
        ability.id,
        effect_input("3d6", "  "),
    )
    .await
    .expect_err("an empty target must be rejected");
}

/// FR-017: effects are independent — editing or removing one leaves the
/// others untouched.
#[tokio::test]
async fn ability_can_carry_multiple_effects() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Fireball"))
        .await
        .unwrap();

    let mut first = effect_input("3d6", "Hit Points");
    first.sort_order = Some(0);
    let first = add_ability_effect_impl(&state, owner_id, false, ability.id, first)
        .await
        .unwrap();

    let mut second = effect_input("1d20 + STAT", "Attack Roll");
    second.effect_type = AbilityEffectType::AttackRoll;
    second.sort_order = Some(1);
    let second = add_ability_effect_impl(&state, owner_id, false, ability.id, second)
        .await
        .unwrap();

    assert_eq!(
        load_ability_effects(&state, ability.id)
            .await
            .unwrap()
            .len(),
        2
    );

    // Editing the first must not disturb the second.
    let mut edited = effect_input("4d6", "Hit Points");
    edited.sort_order = Some(0);
    update_ability_effect_impl(&state, owner_id, false, first.id, edited)
        .await
        .unwrap();

    let reloaded = load_ability_effects(&state, ability.id).await.unwrap();
    assert_eq!(reloaded.len(), 2);
    let untouched = reloaded.iter().find(|e| e.id == second.id).unwrap();
    assert_eq!(untouched.formula, "1d20 + STAT");
    assert_eq!(untouched.target, "Attack Roll");

    // Removing one leaves the other.
    assert!(
        remove_ability_effect_impl(&state, owner_id, false, first.id)
            .await
            .unwrap()
    );
    let remaining = load_ability_effects(&state, ability.id).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, second.id);
}

/// FR-019: effects are inert authored data. Nothing here resolves, rolls,
/// or evaluates a formula — it round-trips byte-for-byte, including
/// notation this spec deliberately does not understand.
#[tokio::test]
async fn ability_effect_formula_is_not_evaluated() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Odd"))
        .await
        .unwrap();

    // Ruleset-specific notation this spec has no opinion about.
    let exotic = "2d8kh1 + PROF - resistance(fire)";
    let created = add_ability_effect_impl(
        &state,
        owner_id,
        false,
        ability.id,
        effect_input(exotic, "Mana"),
    )
    .await
    .expect("structurally valid notation must be accepted as-authored");

    assert_eq!(
        created.formula, exotic,
        "the formula must be stored verbatim"
    );
    assert_eq!(
        created.target, "Mana",
        "a target naming a resource this system lacks is still accepted"
    );

    let reloaded = load_ability_effects(&state, ability.id).await.unwrap();
    assert_eq!(
        reloaded[0].formula, exotic,
        "and it must round-trip unchanged"
    );
    // FR-020: trigger_kind is scaffolded but nothing sets or evaluates it.
    assert_eq!(reloaded[0].trigger_kind, None);
}

/// Effect edits require Editor on the parent ability, not on the effect
/// row — a Viewer must not be able to rewrite an ability's mechanics.
#[tokio::test]
async fn effect_edits_require_editor_on_the_parent_ability() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let member_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_world_member(&mut conn, world_id, member_id, "Player");
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Ward"))
        .await
        .unwrap();

    add_ability_effect_impl(
        &state,
        member_id,
        false,
        ability.id,
        effect_input("2d6", "Hit Points"),
    )
    .await
    .expect_err("a Viewer must not add effects");
}

/// FR-031: deleting an ability must not be blocked by lore linking to it;
/// the link row survives with a null FK and renders unresolved.
#[tokio::test]
async fn deleting_an_ability_nulls_referencing_lore_links_instead_of_blocking() {
    use crate::schema::{world_lore_entries, world_lore_links};
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Linked"))
        .await
        .unwrap();

    let mut conn = state.db_pool.get().unwrap();
    let entry_id = Uuid::now_v7();
    diesel::insert_into(world_lore_entries::table)
        .values((
            world_lore_entries::id.eq(entry_id),
            world_lore_entries::world_id.eq(world_id),
            world_lore_entries::title.eq("Source Entry"),
            world_lore_entries::slug.eq(format!("source-{}", entry_id.simple())),
            world_lore_entries::content.eq("Refers to [[Linked]]."),
            world_lore_entries::created_by.eq(owner_id),
        ))
        .execute(&mut conn)
        .expect("insert lore entry");
    let link_id = Uuid::now_v7();
    diesel::insert_into(world_lore_links::table)
        .values((
            world_lore_links::id.eq(link_id),
            world_lore_links::source_lore_entry_id.eq(entry_id),
            world_lore_links::raw_title.eq("Linked"),
            world_lore_links::target_kind.eq("ability"),
            world_lore_links::target_ability_id.eq(ability.id),
        ))
        .execute(&mut conn)
        .expect("insert lore link");
    drop(conn);

    assert!(
        delete_ability_impl(&state, owner_id, false, ability.id)
            .await
            .unwrap(),
        "deletion must not be blocked by an inbound lore link"
    );

    let mut conn = state.db_pool.get().unwrap();
    let (surviving, target): (Uuid, Option<Uuid>) = world_lore_links::table
        .filter(world_lore_links::id.eq(link_id))
        .select((world_lore_links::id, world_lore_links::target_ability_id))
        .first(&mut conn)
        .expect("the link row must survive");
    assert_eq!(surviving, link_id);
    assert_eq!(target, None, "its FK is nulled, so it renders unresolved");

    // The source entry itself is untouched.
    let title: String = world_lore_entries::table
        .filter(world_lore_entries::id.eq(entry_id))
        .select(world_lore_entries::title)
        .first(&mut conn)
        .expect("source entry must be untouched");
    assert_eq!(title, "Source Entry");
}
