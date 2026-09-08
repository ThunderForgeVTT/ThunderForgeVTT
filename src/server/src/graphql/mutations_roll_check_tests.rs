//! Spec 036 US3b (FR-035 to FR-037), and the guard that keeps the shape.
//!
//! Two halves. The substitution itself is tested without a database, because
//! "the server chose these numbers" is the claim the whole feature rests on
//! and it should be assertable directly. Everything about refusal is tested
//! against a real one, because a refusal that leaves a row behind is the
//! failure worth catching.

use super::*;

use thunderforge_canvas_core::system_rules::{
    DeclaredValue, DeclaredValueKind, DeclaredValues, Origin,
};

use crate::schema::{world_actor_system_data, world_roll_records};
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state,
};

/// The repository root, so a test reads the packs that actually ship rather
/// than a fixture written to agree with them.
fn repo_root() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root should exist")
        .to_string_lossy()
        .into_owned()
}

fn packs() -> String {
    std::path::Path::new(&repo_root())
        .join("packs/systems")
        .to_string_lossy()
        .into_owned()
}

/// `test_app_state` points its directories at a temp dir, which has no packs
/// in it. A check is content, so a test about checks needs the real ones.
fn state_with_real_packs() -> AppState {
    let mut state = test_app_state();
    state.directories = crate::config::Directories::from(repo_root());
    state
}

/// Deterministic RNG, same shape as `mutations_roll.rs`'s — a real roll comes
/// only from the resolver method, never from a test.
struct StepRng(u64);

impl rand::TryRng for StepRng {
    type Error = std::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        self.0 = self.0.wrapping_add(1);
        Ok(self.0 as u32)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.try_next_u32()? as u64)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for b in dest.iter_mut() {
            *b = self.try_next_u32()? as u8;
        }
        Ok(())
    }
}

fn check(id: &str, formula: &str, bindings: &[(&str, &str)]) -> CheckDeclaration {
    CheckDeclaration {
        id: id.to_string(),
        label: id.to_string(),
        group: None,
        formula: formula.to_string(),
        bindings: bindings
            .iter()
            .map(|(placeholder, value_id)| {
                (
                    placeholder.to_string(),
                    CheckBinding::Value {
                        id: value_id.to_string(),
                    },
                )
            })
            .collect(),
    }
}

fn value(id: &str, kind: DeclaredValueKind) -> DeclaredValue {
    DeclaredValue {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        value: kind,
        group: None,
        group_label: None,
        headline: false,
        origin: Origin::Derived,
    }
}

// ---------------------------------------------------------------------------
// The substitution, without a database
// ---------------------------------------------------------------------------

/// FR-035: the number in the roll came off the actor, and the caller never
/// named it.
#[test]
fn a_binding_takes_its_number_from_the_actor() {
    let values = DeclaredValues::new(vec![value("armsMod", DeclaredValueKind::Integer(3))]);
    let bindings = bindings_for_check(
        &check("arms", "1d20 + MODIFIER", &[("MODIFIER", "armsMod")]),
        &values,
    )
    .expect("a declared value is bindable");

    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].name, "MODIFIER");
    assert_eq!(bindings[0].value, 3.0);
}

/// A negative modifier is a number like any other — the case a "treat absence
/// as zero" shortcut gets right by accident and gets wrong the moment a
/// character is actually bad at something.
#[test]
fn a_negative_modifier_survives_intact() {
    let values = DeclaredValues::new(vec![value("armsMod", DeclaredValueKind::Integer(-2))]);
    let bindings = bindings_for_check(
        &check("arms", "1d20 + MODIFIER", &[("MODIFIER", "armsMod")]),
        &values,
    )
    .unwrap();
    assert_eq!(bindings[0].value, -2.0);
}

/// An unfilled sheet is the absence of a number, not the number nought. The
/// refusal names the identifier so a Game Master can see which square is empty.
#[test]
fn a_missing_value_refuses_rather_than_rolling_against_zero() {
    let err = bindings_for_check(
        &check("arms", "1d20 + MODIFIER", &[("MODIFIER", "armsMod")]),
        &DeclaredValues::default(),
    )
    .expect_err("a check whose input is absent cannot be rolled");
    assert!(err.contains("armsMod"), "the refusal should name it: {err}");
}

/// A Fate ladder rung is text and a Cypher damage track is a position. Neither
/// is a number, and neither is silently coerced into one.
#[test]
fn a_value_that_is_not_a_number_refuses() {
    let values = DeclaredValues::new(vec![value(
        "rung",
        DeclaredValueKind::Text("Great".to_string()),
    )]);
    let err = bindings_for_check(
        &check("ladder", "4dF + MODIFIER", &[("MODIFIER", "rung")]),
        &values,
    )
    .expect_err("text is not a modifier");
    assert!(err.contains("rung"));
}

/// A pool's number is its current value, which `as_integer` already settled;
/// this check confirms the check path inherits that answer rather than
/// inventing a second one.
#[test]
fn a_pool_binds_to_its_current_value() {
    let values = DeclaredValues::new(vec![value(
        "stress",
        DeclaredValueKind::Fraction {
            current: 2,
            max: Some(6),
        },
    )]);
    let bindings = bindings_for_check(
        &check("s", "2d6 + MODIFIER", &[("MODIFIER", "stress")]),
        &values,
    )
    .unwrap();
    assert_eq!(bindings[0].value, 2.0);
}

/// A flat check binds nothing and is complete.
#[test]
fn a_check_with_no_bindings_needs_nothing_from_the_actor() {
    let bindings =
        bindings_for_check(&check("flat", "2d6", &[]), &DeclaredValues::default()).unwrap();
    assert!(bindings.is_empty());
}

// ---------------------------------------------------------------------------
// The shipped packs
// ---------------------------------------------------------------------------

/// FR-037, measured against the packs themselves rather than a list here: at
/// least one system declares checks and at least one declares none, and both
/// are read without complaint.
#[test]
fn every_shipped_pack_either_declares_usable_checks_or_none_at_all() {
    let systems: Vec<String> = std::fs::read_dir(packs())
        .expect("the bundled packs should be readable")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(systems.len() > 1, "there should be several bundled packs");

    let mut with_checks = 0;
    let mut without_checks = 0;
    for system in &systems {
        let checks = checks_for_system(&packs(), system);
        if checks.is_empty() {
            without_checks += 1;
            continue;
        }
        with_checks += 1;

        let mut seen = std::collections::BTreeSet::new();
        for check in &checks {
            assert!(
                seen.insert(check.id.clone()),
                "'{}' declares '{}' twice",
                system,
                check.id
            );
            assert!(!check.label.is_empty(), "a check needs a label to offer");

            // Every declared formula must parse, and every placeholder in it
            // must be bound. This is the check the canvas-core crate cannot
            // make — it does not depend on the dice crate — and it is the one
            // that would catch a pack shipping a button nobody can press.
            let formula = thunderforge_dice::DiceFormula::parse(&check.formula)
                .unwrap_or_else(|e| panic!("'{}' check '{}': {e}", system, check.id));

            let bindings: std::collections::HashMap<String, f64> = check
                .bindings
                .keys()
                .map(|name| (name.clone(), 1.0))
                .collect();
            let mut rng = StepRng(7);
            thunderforge_dice::resolve(&formula, &bindings, &mut rng).unwrap_or_else(|e| {
                panic!(
                    "'{}' check '{}' does not resolve with its own bindings: {e}",
                    system, check.id
                )
            });
        }
    }

    assert!(with_checks >= 1, "at least one pack should declare checks");
    assert!(
        without_checks >= 1,
        "a pack declaring none must remain valid — FR-037"
    );
}

/// The 5e block is generated from the abilities and skills the pack already
/// declares, and every binding names a value that pack's own rules publish.
/// A transcription error here is a button that refuses at a table.
#[tokio::test]
async fn the_5e_checks_bind_only_to_values_that_system_actually_publishes() {
    let checks = checks_for_system(&packs(), "dnd5e");
    assert_eq!(checks.len(), 24, "six abilities and eighteen skills");

    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({
            "strength": 10, "dexterity": 16, "constitution": 14,
            "intelligence": 8, "wisdom": 12, "charisma": 7
        })),
        resource_data: Some(serde_json::json!({ "current_hp": 22, "max_hp": 38 })),
        proficiency_data: Some(serde_json::json!({
            "skill_proficiencies": ["stealth"],
            "saving_throw_proficiencies": ["dexterity"]
        })),
        trait_data: Some(serde_json::json!({ "level": 5 })),
    };
    let values = DeclaredValues::new(declared_values_for_actor(&packs(), "dnd5e", &slots));

    for check in &checks {
        bindings_for_check(check, &values)
            .unwrap_or_else(|e| panic!("check '{}' cannot be rolled: {e}", check.id));
    }

    // And the number really is the one the ruleset derived, not a guess:
    // dexterity 16 is +3, and the sheet never said so.
    let dexterity = checks.iter().find(|c| c.id == "dexterity").unwrap();
    assert_eq!(
        bindings_for_check(dexterity, &values).unwrap()[0].value,
        3.0
    );
}

// ---------------------------------------------------------------------------
// The mutation, against a database
// ---------------------------------------------------------------------------

/// Sets up a world playing `system_id`, with one actor whose sheet is filled
/// in. Returns the owner, the world and the actor.
fn world_with_actor(state: &AppState, system_id: &str) -> (Uuid, Uuid, Uuid) {
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    diesel::update(crate::schema::worlds::table.filter(crate::schema::worlds::id.eq(world_id)))
        .set(crate::schema::worlds::game_system_id.eq(system_id))
        .execute(&mut conn)
        .unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);

    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actor_system_data::table)
        .values((
            world_actor_system_data::id.eq(Uuid::now_v7()),
            world_actor_system_data::actor_id.eq(actor_id),
            world_actor_system_data::game_system_id.eq(system_id),
            world_actor_system_data::ability_data.eq(Some(serde_json::json!({
                "strength": 10, "dexterity": 16, "constitution": 14,
                "intelligence": 8, "wisdom": 12, "charisma": 7
            }))),
            world_actor_system_data::proficiency_data.eq(Some(serde_json::json!({
                "skill_proficiencies": ["stealth"]
            }))),
            world_actor_system_data::trait_data.eq(Some(serde_json::json!({ "level": 5 }))),
            world_actor_system_data::created_by.eq(owner_id),
            world_actor_system_data::updated_by.eq(owner_id),
            world_actor_system_data::created_at.eq(now),
            world_actor_system_data::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .unwrap();

    (owner_id, world_id, actor_id)
}

fn roll_count(state: &AppState, world_id: Uuid) -> i64 {
    let mut conn = state.db_pool.get().unwrap();
    world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap()
}

/// FR-036: the record a sheet-rolled check leaves is an ordinary roll record —
/// same table, same triggering user, carrying the formula the *system*
/// declared and the bindings the *server* resolved.
#[tokio::test]
async fn a_check_rolled_from_a_sheet_is_recorded_exactly_as_a_roll_is() {
    let state = state_with_real_packs();
    let (owner_id, world_id, actor_id) = world_with_actor(&state, "dnd5e");

    let mut rng = StepRng(11);
    let resolution = roll_check_impl(
        &state,
        owner_id,
        false,
        world_id,
        actor_id,
        "dexterity".to_string(),
        &mut rng,
    )
    .await
    .expect("the actor's owner may roll their check");

    assert_eq!(resolution.dice.len(), 1, "5e checks roll one d20");

    let mut conn = state.db_pool.get().unwrap();
    let records: Vec<crate::models::RollRecord> = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .load(&mut conn)
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].triggered_by, owner_id);
    assert!(
        records[0].formula.contains("d20"),
        "the recorded formula is the system's: {}",
        records[0].formula
    );
    // Dexterity 16 is +3, and no client said so.
    let bindings = records[0].bindings.clone().expect("bindings are recorded");
    assert_eq!(bindings["MODIFIER"], serde_json::json!(3.0));
}

/// An unknown id is refused, and — the point of the test — it is refused as an
/// id rather than evaluated as a formula. `"1d20+100"` is a valid formula and
/// a client passing it must get nothing.
#[tokio::test]
async fn an_unknown_check_id_is_never_treated_as_a_formula() {
    let state = state_with_real_packs();
    let (owner_id, world_id, actor_id) = world_with_actor(&state, "dnd5e");

    for candidate in ["1d20+100", "not_a_check", "", "dexterity "] {
        let mut rng = StepRng(3);
        let err = roll_check_impl(
            &state,
            owner_id,
            false,
            world_id,
            actor_id,
            candidate.to_string(),
            &mut rng,
        )
        .await
        .expect_err("only a declared check id may be rolled");
        assert!(
            err.message.contains("no such check"),
            "refused as an id, not parsed: {}",
            err.message
        );
    }

    assert_eq!(
        roll_count(&state, world_id),
        0,
        "a refused check leaves no record anywhere"
    );
}

/// A sheet is not a permission bypass: a world member with no permission on
/// this actor is refused by the same function that refuses them anywhere else,
/// and nothing is rolled.
#[tokio::test]
async fn a_member_without_permission_on_the_actor_is_refused() {
    let state = state_with_real_packs();
    let (_owner_id, world_id, actor_id) = world_with_actor(&state, "dnd5e");

    let mut conn = state.db_pool.get().unwrap();
    let bystander = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, bystander, "Player");
    drop(conn);

    let mut rng = StepRng(5);
    let result = roll_check_impl(
        &state,
        bystander,
        false,
        world_id,
        actor_id,
        "dexterity".to_string(),
        &mut rng,
    )
    .await;

    assert!(result.is_err(), "a Viewer may not roll this actor's check");
    assert_eq!(roll_count(&state, world_id), 0);
}

/// An actor id from another world cannot be rolled into a world the caller
/// happens to belong to.
#[tokio::test]
async fn an_actor_from_another_world_is_refused() {
    let state = state_with_real_packs();
    let (owner_id, world_id, _actor_id) = world_with_actor(&state, "dnd5e");
    let (_other_owner, _other_world, other_actor) = world_with_actor(&state, "dnd5e");

    let mut rng = StepRng(9);
    let err = roll_check_impl(
        &state,
        owner_id,
        false,
        world_id,
        other_actor,
        "dexterity".to_string(),
        &mut rng,
    )
    .await
    .expect_err("an actor belongs to one world");
    assert!(err.message.contains("not in this world"));
    assert_eq!(roll_count(&state, world_id), 0);
}

/// FR-037 through the mutation: a world whose system declares no checks offers
/// none and rolls none.
#[tokio::test]
async fn a_system_declaring_no_checks_offers_none_through_the_mutation() {
    let state = state_with_real_packs();
    let (owner_id, world_id, actor_id) = world_with_actor(&state, "fate_core");

    assert!(
        checks_for_system(&packs(), "fate_core").is_empty(),
        "this test is only meaningful while that pack declares none"
    );

    let mut rng = StepRng(13);
    let err = roll_check_impl(
        &state,
        owner_id,
        false,
        world_id,
        actor_id,
        "ladder".to_string(),
        &mut rng,
    )
    .await
    .expect_err("a system with no checks has none to roll");
    assert!(err.message.contains("no such check"));
    assert_eq!(roll_count(&state, world_id), 0);
}

// ---------------------------------------------------------------------------
// The schema guard
// ---------------------------------------------------------------------------

/// The shape of the API is the enforcement, so it is asserted against the
/// rendered schema rather than trusted to review — same guard as
/// `mutations_instance_access.rs`'s.
///
/// A field taking a formula would make every other rule in this module
/// decorative: a client that can name a formula has decided what the check is.
/// There is no argument by which it could, and this is what says so.
#[test]
fn the_schema_offers_a_check_by_name_and_no_way_to_name_a_roll() {
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish();
    let sdl = schema.sdl();

    for field in [
        "rollCheck(worldId: UUID!, actorId: UUID!, checkId: String!)",
        "systemChecks(worldId: UUID!)",
    ] {
        assert!(
            sdl.contains(field),
            "`{field}` must be reachable from the root"
        );
    }

    // Matched against the field *declaration* — SDL indents fields with a tab
    // — rather than anywhere in the text, because the doc comment above this
    // field says in prose that there is no formula argument and async_graphql
    // emits doc comments as descriptions.
    let declaration = sdl
        .lines()
        .find(|line| line.trim_start().starts_with("rollCheck("))
        .expect("rollCheck must be declared");
    for forbidden in ["formula", "bindings", "value", "result", "total"] {
        assert!(
            !declaration.to_ascii_lowercase().contains(forbidden),
            "`rollCheck` must take no {forbidden} argument: {declaration}"
        );
    }

    // And the checks a sheet is offered carry no formula either — a client
    // that can read the roll is one step from performing it.
    assert!(
        !sdl.contains("type SystemCheck") || !sdl.contains("\tformula: String!"),
        "a published check names itself and does not publish its dice"
    );
}
