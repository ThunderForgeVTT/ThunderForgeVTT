//! Spec 044 T087: contracts B7 and B8.
//!
//! B7 — a stored spec is an object, at most 4 KB, and passes
//! `HERO_SPEC_SCHEMA`; a refusal names the problem and writes neither the spec
//! nor the image. B8 — an image written without a spec has none, so a file
//! replacing a built image clears that role's spec in the same write.

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use super::spec_schema::{
    HERO_SPEC_SCHEMA_JSON, HeroSpecRefusal, MAX_HERO_SPEC_BYTES, check_hero_spec,
};
use crate::graphql::mutations_actor_images::{
    ROLE_PORTRAIT, ROLE_TOKEN, UploadActorImageError, actor_images_impl, upload_actor_image_impl,
};
use crate::graphql::mutations_actors::{CreateActorInput, create_actor_impl};
use crate::schema::world_actor_images;
use crate::state::AppState;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, test_app_state, tiny_png_bytes,
};

fn sir_pip() -> Value {
    json!({
        "name": "Sir Pip",
        "title": "the Stalwart Knight",
        "skin": "#f1c6a0",
        "hair": "short",
        "hairColor": "#c98a3c",
        "headgear": "helm",
        "emblem": "cross",
        "prop": "sword",
        "tusks": false,
        "size": "small"
    })
}

fn problems_of(spec: &Value) -> Vec<String> {
    match check_hero_spec(spec) {
        Err(HeroSpecRefusal::Invalid(problems)) => problems,
        other => panic!("expected a schema refusal for {spec}, got {other:?}"),
    }
}

// --- B7, without a database -------------------------------------------------

#[test]
fn the_vendored_schema_refuses_fields_it_does_not_name() {
    let schema: Value = serde_json::from_str(HERO_SPEC_SCHEMA_JSON).expect("JSON");
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(schema["required"], json!(["name"]));
    assert!(
        schema["properties"].get("race").is_none(),
        "the race a roll used is never a hero field (B5a)"
    );
}

#[test]
fn a_valid_spec_passes() {
    assert_eq!(check_hero_spec(&sir_pip()), Ok(()));
    assert_eq!(
        check_hero_spec(&json!({ "name": "Mira", "title": "" })),
        Ok(())
    );
}

#[test]
fn anything_but_an_object_is_refused_by_kind() {
    for (value, kind) in [
        (json!([sir_pip()]), "an array"),
        (json!("Sir Pip"), "a string"),
        (json!(7), "a number"),
        (json!(true), "a boolean"),
        (Value::Null, "null"),
    ] {
        let refusal = check_hero_spec(&value).expect_err("not an object");
        assert_eq!(refusal, HeroSpecRefusal::NotAnObject(kind));
        assert!(refusal.to_string().contains(kind), "{refusal}");
    }
}

#[test]
fn a_spec_over_the_cap_is_refused_with_its_size() {
    let mut spec = sir_pip();
    // Over the cap by weight alone: the schema would also refuse the field,
    // but the size is what is reported, and it is checked first.
    spec["padding"] = json!("x".repeat(MAX_HERO_SPEC_BYTES));
    let refusal = check_hero_spec(&spec).expect_err("too large");
    let HeroSpecRefusal::TooLarge { actual, max } = refusal.clone() else {
        panic!("expected TooLarge, got {refusal:?}");
    };
    assert!(actual > max);
    assert_eq!(max, MAX_HERO_SPEC_BYTES);
    assert!(refusal.to_string().contains("4096"), "{refusal}");
}

#[test]
fn an_unknown_field_is_refused_by_name() {
    let mut spec = sir_pip();
    spec["race"] = json!("elf");
    let problems = problems_of(&spec);
    assert!(
        problems.iter().any(|p| p.contains("race")),
        "the refusal names the field: {problems:?}"
    );
}

#[test]
fn each_problem_names_its_field() {
    let problems = problems_of(&json!({
        "name": "Mira",
        "headgear": "crown",
        "skin": "red",
        "beard": "yes"
    }));
    for field in ["headgear", "skin", "beard"] {
        assert!(
            problems.iter().any(|p| p.starts_with(&format!("{field}:"))),
            "{field} is named in {problems:?}"
        );
    }
}

#[test]
fn a_missing_or_blank_name_is_refused() {
    assert!(!problems_of(&json!({ "hair": "long" })).is_empty());
    assert!(!problems_of(&json!({ "name": "" })).is_empty());
    assert!(!problems_of(&json!({ "name": "   " })).is_empty());
    assert!(!problems_of(&json!({ "name": "Mira", "title": "  " })).is_empty());
}

/// `validateHero` measures `String.length`; an astral character is two of
/// those and one code point. The server measures the client's way.
#[test]
fn text_is_measured_as_the_client_measures_it() {
    let astral = "\u{1F409}".repeat(41); // 41 code points, 82 UTF-16 units
    let problems = problems_of(&json!({ "name": astral }));
    assert_eq!(problems, vec!["name: at most 80 characters".to_string()]);

    // JavaScript's `trim()` counts the byte-order mark as whitespace.
    let bom = problems_of(&json!({ "name": "\u{feff}" }));
    assert!(
        bom.contains(&"name: needs some text".to_string()),
        "{bom:?}"
    );

    assert_eq!(check_hero_spec(&json!({ "name": "x".repeat(80) })), Ok(()));
}

// --- B7 and B8 at the mutation ----------------------------------------------

struct Fixture {
    state: AppState,
    owner_id: Uuid,
    actor_id: Uuid,
}

async fn fixture() -> Fixture {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);
    let actor_id = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Sir Pip".to_string(),
            is_npc: true,
            actor_type: None,
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("world owner may create an actor")
    .id;
    Fixture {
        state,
        owner_id,
        actor_id,
    }
}

async fn upload(
    f: &Fixture,
    role: &str,
    spec: Option<Value>,
) -> Result<crate::models::WorldActorImage, UploadActorImageError> {
    upload_actor_image_impl(
        &f.state,
        f.owner_id,
        false,
        f.actor_id,
        role.to_string(),
        tiny_png_bytes(),
        spec,
    )
    .await
}

fn image_rows(f: &Fixture) -> Vec<(String, Uuid, Option<Value>)> {
    let mut conn = f.state.db_pool.get().unwrap();
    world_actor_images::table
        .filter(world_actor_images::actor_id.eq(f.actor_id))
        .order(world_actor_images::role.asc())
        .select((
            world_actor_images::role,
            world_actor_images::asset_id,
            world_actor_images::hero_spec,
        ))
        .load(&mut conn)
        .unwrap()
}

#[tokio::test]
async fn a_valid_spec_is_stored_with_its_image_and_read_back() {
    let f = fixture().await;
    let stored = upload(&f, ROLE_PORTRAIT, Some(sir_pip()))
        .await
        .expect("stored");
    assert_eq!(stored.hero_spec, Some(sir_pip()));

    let images = actor_images_impl(&f.state, f.actor_id).await.unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].hero_spec, Some(sir_pip()));
    let exposed = crate::graphql::types::GraphQLActorImage::from(images[0].clone());
    assert_eq!(exposed.hero_spec.map(|j| j.0), Some(sir_pip()));
}

#[tokio::test]
async fn a_refused_spec_writes_no_image_row() {
    let f = fixture().await;
    for bad in [
        json!([sir_pip()]),
        json!({ "name": "Sir Pip", "race": "halfling" }),
        json!({ "name": "Sir Pip", "notes": "y".repeat(MAX_HERO_SPEC_BYTES) }),
    ] {
        let refusal = upload(&f, ROLE_PORTRAIT, Some(bad.clone()))
            .await
            .expect_err("refused");
        assert!(
            matches!(refusal, UploadActorImageError::HeroSpec(_)),
            "{bad} refused by B7, got {refusal:?}"
        );
    }
    assert!(image_rows(&f).is_empty(), "neither the spec nor the image");
}

#[tokio::test]
async fn a_refused_spec_leaves_the_stored_image_and_spec_alone() {
    let f = fixture().await;
    let first = upload(&f, ROLE_PORTRAIT, Some(sir_pip())).await.unwrap();
    upload(
        &f,
        ROLE_PORTRAIT,
        Some(json!({ "name": "Sir Pip", "hair": "mohawk" })),
    )
    .await
    .expect_err("refused");
    assert_eq!(
        image_rows(&f),
        vec![(ROLE_PORTRAIT.to_string(), first.asset_id, Some(sir_pip()))]
    );
}

#[tokio::test]
async fn an_upload_without_a_spec_clears_that_roles_spec_only() {
    let f = fixture().await;
    upload(&f, ROLE_PORTRAIT, Some(sir_pip())).await.unwrap();
    upload(&f, ROLE_TOKEN, Some(sir_pip())).await.unwrap();

    let replaced = upload(&f, ROLE_TOKEN, None).await.expect("a plain file");
    assert_eq!(replaced.hero_spec, None);

    let rows = image_rows(&f);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, ROLE_PORTRAIT);
    assert_eq!(rows[0].2, Some(sir_pip()), "the portrait keeps its spec");
    assert_eq!(rows[1].0, ROLE_TOKEN);
    assert_eq!(
        rows[1].2, None,
        "the token no longer comes from a built hero"
    );
}
