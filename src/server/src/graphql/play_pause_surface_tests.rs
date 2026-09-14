//! Every root mutation and subscription refuses a paused world, or says why
//! it does not (spec 051 T027/T036, contracts/live-play-lock.md).
//!
//! # Why this exists
//!
//! There is no choke point a pause can hang on (research R2). The gate is
//! called resolver by resolver, beside each one's role check, so the only way
//! to know the list is whole is to make it closed. This is the third use of the
//! pattern `admin_surface_tests` and `disabled_surface_tests` established, with
//! the same two halves:
//!
//! 1. **Nothing is unclassified.** Every root `Mutation` and `Subscription`
//!    field, read from the schema, is in exactly one of [`GATED`],
//!    [`NOT_WORLD_SCOPED`] and [`OPERATOR`]. A field added to the schema and to
//!    no table fails the build; so does one in two.
//! 2. **The classification is honest.** Every [`GATED`] entry is *executed*
//!    against a paused world, as its Game Master and as a site admin who is a
//!    member, and must answer `WORLD_PLAY_PAUSED`.
//!
//! # Why the fixture lives in the table
//!
//! The plan names the weak point: a second, hand-kept list of requests that
//! drifts from the first. So there is no second list. A [`GATED`] entry *is*
//! its request, and a name cannot be gated without saying how it is called.
//! What can still drift is the request itself — a document calling some other
//! field, or failing validation — and `every_gated_request_calls_its_own_field`
//! and the assertion on the code catch both: a request that never reached its
//! resolver has no code.
//!
//! # Reads
//!
//! Queries are not in these tables. Read-only world queries stay answered
//! while paused (FR-024, *readable, not editable*), including `worldPlayState`,
//! which is how the notice learns a pause was lifted. The two queries that
//! *start play* — `worldSyncPlan` and `worldEventsSince` — are gated, and are
//! listed in [`GATED`] explicitly.

use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use async_graphql::Request;
use diesel::prelude::*;
use futures_util::StreamExt as _;
use serde_json::Value;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::play_pause::gate::WORLD_PLAY_PAUSED;
use crate::play_pause::{TriggerDetail, pause_world};
use crate::state::AppState;
use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member};

#[path = "play_pause_surface_tables.rs"]
mod tables;
use tables::*;

fn sdl() -> String {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl()
}

/// Every field of a root type, read from the SDL. Descriptions are stripped
/// first for the reason `admin_surface_tests::root_fields` gives: prose wraps
/// across lines and reads like a field declaration.
fn root_fields(sdl: &str, type_name: &str) -> Vec<String> {
    let mut stripped = String::with_capacity(sdl.len());
    let mut in_description = false;
    for line in sdl.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"\"\"") {
            let fenced = trimmed.len() > 3 && trimmed.ends_with("\"\"\"");
            if !fenced {
                in_description = !in_description;
            }
            continue;
        }
        if !in_description {
            stripped.push_str(line);
            stripped.push('\n');
        }
    }

    let Some(start) = stripped.find(&format!("type {type_name} {{")) else {
        return Vec::new();
    };
    let body = &stripped[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];

    // A field starts at depth zero: its name, then `(` or `:`. Arguments may
    // span lines, so depth is tracked across the whole body.
    let mut fields = Vec::new();
    let mut depth = 0usize;
    let mut name = String::new();
    for c in body.chars() {
        match c {
            '(' => {
                if depth == 0 && !name.is_empty() {
                    fields.push(std::mem::take(&mut name));
                }
                depth += 1;
            }
            ')' => depth -= 1,
            ':' if depth == 0 => {
                if !name.is_empty() {
                    fields.push(std::mem::take(&mut name));
                }
                // Skip the type: everything to the end of the line.
                name.clear();
            }
            c if depth == 0 && (c.is_alphanumeric() || c == '_') => name.push(c),
            '\n' if depth == 0 => name.clear(),
            _ if depth == 0 => name.clear(),
            _ => {}
        }
    }
    // The type after each colon was collected as a "name" and cleared at the
    // newline; the pushes above only ever fire on a name followed by `(` or
    // `:`, which a type never is.
    fields
}

/// Half one: every root mutation and subscription is in exactly one table.
#[test]
fn every_root_mutation_and_subscription_is_classified_once() {
    let sdl = sdl();
    let mut problems = Vec::new();

    let mutations = root_fields(&sdl, "MutationRoot");
    let subscriptions = root_fields(&sdl, "SubscriptionRoot");
    assert!(
        mutations.len() > 100 && subscriptions.len() > 3,
        "the SDL reader found {} mutations and {} subscriptions; it is reading the wrong thing",
        mutations.len(),
        subscriptions.len(),
    );

    for field in mutations.iter().chain(subscriptions.iter()) {
        let tables: Vec<&str> = [
            ("GATED", GATED.iter().any(|(name, _)| name == field)),
            (
                "NOT_WORLD_SCOPED",
                NOT_WORLD_SCOPED.contains(&field.as_str()),
            ),
            ("OPERATOR", OPERATOR.contains(&field.as_str())),
        ]
        .into_iter()
        .filter_map(|(table, listed)| listed.then_some(table))
        .collect();
        match tables.as_slice() {
            [_] => {}
            [] => problems.push(format!("{field}: in no table")),
            many => problems.push(format!("{field}: in {}", many.join(" and "))),
        }
    }

    for query in PLAY_STARTING_QUERIES {
        if !GATED.iter().any(|(name, _)| name == query) {
            problems.push(format!("{query}: starts play, and must be GATED"));
        }
    }

    assert!(
        problems.is_empty(),
        "every root mutation and subscription is gated against a paused world, \
         or listed in NOT_WORLD_SCOPED or OPERATOR with a reason:\n  {}",
        problems.join("\n  "),
    );
}

/// The tables name fields that exist, and nothing twice.
#[test]
fn the_tables_name_fields_that_exist() {
    let sdl = sdl();
    let mut all: HashSet<String> = root_fields(&sdl, "MutationRoot").into_iter().collect();
    all.extend(root_fields(&sdl, "SubscriptionRoot"));

    let mut seen = HashSet::new();
    for name in GATED
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !PLAY_STARTING_QUERIES.contains(name))
        .chain(NOT_WORLD_SCOPED.iter().copied())
        .chain(OPERATOR.iter().copied())
    {
        assert!(
            all.contains(name),
            "`{name}` is not a root mutation or subscription. Renamed or removed? \
             A stale entry tests nothing."
        );
        assert!(seen.insert(name), "`{name}` is listed twice");
    }
    for name in PLAY_STARTING_QUERIES {
        assert!(
            root_fields(&sdl, "QueryRoot").iter().any(|f| f == name),
            "`{name}` is not a root query"
        );
    }
}

/// A request that calls some other field would pass for the wrong reason.
#[test]
fn every_gated_request_calls_its_own_field() {
    for (name, document) in GATED {
        if let Some(upload) = document.strip_prefix("UPLOAD ") {
            assert_eq!(upload, *name, "the upload marker names its own field");
            continue;
        }
        let selection = &document[document.find('{').expect("a selection") + 1..];
        let called: String = selection
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect();
        assert_eq!(called, *name, "GATED `{name}`'s request calls `{called}`");
    }
}

/// An exception for a field that is not gated is a stale exception.
#[test]
fn every_caller_exception_names_a_gated_field() {
    for (name, callers, reason) in CALLED_AS {
        assert!(
            GATED.iter().any(|(field, _)| field == name),
            "CALLED_AS names `{name}`, which is not GATED"
        );
        assert!(!callers.is_empty() && !reason.is_empty());
    }
}

// --- half two -------------------------------------------------------------

fn schema(state: AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn caller(user_id: Uuid, is_admin: bool) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin,
        role: "User".to_string(),
        disabled: false,
    }
}

/// Run one document and return its first response as JSON. Subscriptions
/// answer their refusal at once; a stream that says nothing for five seconds
/// was opened, which is the finding.
async fn first_response(schema: &crate::graphql::AppSchema, request: Request) -> Value {
    let mut stream = schema.execute_stream(request);
    let response = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .map(|response| response.expect("one response"));
    match response {
        Ok(response) => serde_json::to_value(&response).unwrap(),
        Err(_) => serde_json::json!({ "timedOut": true }),
    }
}

/// Seed data for the owner, answering with the named field's JSON.
async fn seed_one(
    schema: &crate::graphql::AppSchema,
    owner: Uuid,
    document: &str,
    field: &str,
) -> Value {
    let response = first_response(schema, Request::new(document).data(caller(owner, false))).await;
    assert!(
        response.get("errors").is_none(),
        "seeding `{field}` failed: {response}"
    );
    response["data"][field].clone()
}

fn fill(template: &str, ids: &BTreeMap<&'static str, String>) -> String {
    let mut filled = template.to_string();
    for (key, value) in ids {
        filled = filled.replace(&format!("{{{key}}}"), value);
    }
    filled
}

/// A world with one of everything the gated requests name, made through the
/// schema where it can be, so each row is one the resolvers themselves wrote.
async fn seed(
    state: &AppState,
    schema: &crate::graphql::AppSchema,
    owner: Uuid,
    world: Uuid,
    player: Uuid,
) -> BTreeMap<&'static str, String> {
    let mut ids: BTreeMap<&'static str, String> = BTreeMap::new();
    ids.insert("world", world.to_string());
    ids.insert("player", player.to_string());

    // A system, for the resolvers that read one before anything else, and a
    // book on the owner's shelf running it, switched on below.
    {
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(crate::schema::worlds::table.filter(crate::schema::worlds::id.eq(world)))
            .set(crate::schema::worlds::game_system_id.eq(Some("test-system".to_string())))
            .execute(&mut conn)
            .unwrap();
        let book = crate::compendium::store::import_book(
            &mut conn,
            owner,
            crate::compendium::store::NewBook {
                book_title: "Paused Manual".to_string(),
                source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
                system_id: "test-system".to_string(),
                parser_version: "surface-test".to_string(),
                page_count: 1,
                silent_page_count: 0,
            },
            &[],
        )
        .expect("a book on the shelf");
        ids.insert("compendium", book.id.to_string());
    }

    macro_rules! make {
        ($key:literal, $field:literal, $doc:expr, $pick:expr) => {{
            let value = seed_one(schema, owner, &fill($doc, &ids), $field).await;
            let pick: fn(&Value) -> &Value = $pick;
            let id = pick(&value)
                .as_str()
                .unwrap_or_else(|| panic!("seeding {} gave {value}", $key))
                .to_string();
            ids.insert($key, id);
        }};
    }

    make!(
        "scene",
        "createScene",
        r#"mutation { createScene(input: { worldId: "{world}", name: "Seeded" }) { sceneId } }"#,
        |v| &v["sceneId"]
    );
    make!(
        "token",
        "createToken",
        r#"mutation { createToken(input: { sceneId: "{scene}", x: 0, y: 0 }) { tokenId } }"#,
        |v| &v["tokenId"]
    );
    make!(
        "wall",
        "createWall",
        r#"mutation { createWall(input: { sceneId: "{scene}", x1: 0, y1: 0, x2: 1, y2: 1 }) { wallId } }"#,
        |v| &v["wallId"]
    );
    make!(
        "door",
        "createWall",
        r#"mutation { createWall(input: { sceneId: "{scene}", x1: 2, y1: 0, x2: 3, y2: 1, doorState: CLOSED }) { wallId } }"#,
        |v| &v["wallId"]
    );
    make!(
        "light",
        "createLightSource",
        r#"mutation { createLightSource(input: { sceneId: "{scene}", x: 0, y: 0, radius: 1 }) { lightId } }"#,
        |v| &v["lightId"]
    );
    make!(
        "shape",
        "createShape",
        r#"mutation { createShape(input: { sceneId: "{scene}", kind: RECT, geometry: { x: 0, y: 0, w: 1, h: 1 } }) { shapeId } }"#,
        |v| &v["shapeId"]
    );
    make!(
        "worldToken",
        "createWorldToken",
        r#"mutation { createWorldToken(input: { worldId: "{world}" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "actor",
        "createActor",
        r#"mutation { createActor(input: { worldId: "{world}", label: "Seeded", isNpc: false }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "item",
        "createItem",
        r#"mutation { createItem(input: { worldId: "{world}", name: "Seeded" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "placedItem",
        "createToken",
        r#"mutation { createToken(input: { sceneId: "{scene}", x: 5, y: 5, metadata: { item: "{item}" } }) { tokenId } }"#,
        |v| &v["tokenId"]
    );
    make!(
        "ability",
        "createAbility",
        r#"mutation { createAbility(input: { worldId: "{world}", name: "Seeded", classification: "spell" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "abilityEffect",
        "addAbilityEffect",
        r#"mutation { addAbilityEffect(abilityId: "{ability}", effect: { effectType: HEAL, formula: "1", target: "self" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "itemEffect",
        "addItemEffect",
        r#"mutation { addItemEffect(itemId: "{item}", effect: { effectType: HEAL, formula: "1", target: "self" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "actorAbility",
        "attachAbilityToActor",
        r#"mutation { attachAbilityToActor(input: { actorId: "{actor}", abilityId: "{ability}" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "inventoryEntry",
        "addItemToInventory",
        r#"mutation { addItemToInventory(input: { actorId: "{actor}", itemId: "{item}", quantity: 1 }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "lore",
        "createLoreEntry",
        r#"mutation { createLoreEntry(input: { worldId: "{world}", title: "Seeded", content: "x" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "loreRevision",
        "createLoreEntry",
        r#"mutation { createLoreEntry(input: { worldId: "{world}", title: "Revised", content: "x" }) { currentRevisionId } }"#,
        |v| &v["currentRevisionId"]
    );
    make!(
        "collection",
        "createCollection",
        r#"mutation { createCollection(input: { worldId: "{world}", name: "Seeded" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "collectionMember",
        "addCollectionMember",
        r#"mutation { addCollectionMember(input: { collectionId: "{collection}", memberType: "actor", memberId: "{actor}" }) { memberId } }"#,
        |v| &v["memberId"]
    );
    make!(
        "invite",
        "generateInviteCode",
        r#"mutation { generateInviteCode(input: { worldId: "{world}", maxUses: 5 }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "inviteCode",
        "rotateInviteCode",
        r#"mutation { rotateInviteCode(inviteId: "{invite}") { inviteCode } }"#,
        |v| &v["inviteCode"]
    );
    make!(
        "combat",
        "startCombat",
        r#"mutation { startCombat(input: { worldId: "{world}" }) { id } }"#,
        |v| &v["id"]
    );
    make!(
        "combatant",
        "addCombatant",
        r#"mutation { addCombatant(input: { combatId: "{combat}", label: "Seeded" }) { combatants { id } } }"#,
        |v| &v["combatants"][0]["id"]
    );
    seed_one(
        schema,
        owner,
        &fill(
            r#"mutation { switchOnCompendium(worldId: "{world}", compendiumId: "{compendium}") { compendiumId } }"#,
            &ids,
        ),
        "switchOnCompendium",
    )
    .await;
    make!(
        "interactive",
        "createInteractive",
        r#"mutation { createInteractive(input: { sceneId: "{scene}", subjectKind: "door", subjectRef: "{door}", trigger: "click", activation: "requires_approval" }) { interactiveId } }"#,
        |v| &v["interactiveId"]
    );

    // Rows no resolver makes without more than a test can supply: a pending
    // request (a player asks), share links (an agreement to terms), the
    // player's membership row.
    let mut conn = state.db_pool.get().unwrap();
    let scene: Uuid = ids["scene"].parse().unwrap();
    let interactive: Uuid = ids["interactive"].parse().unwrap();
    let request = Uuid::now_v7();
    {
        use crate::schema::interaction_requests as r;
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(r::table)
            .values((
                r::request_id.eq(request),
                r::interactive_id.eq(interactive),
                r::scene_id.eq(scene),
                r::requested_by.eq(player),
                r::state.eq("pending"),
                r::created_by.eq(player),
                r::updated_by.eq(player),
                r::created_at.eq(now),
                r::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("a pending request");
    }
    ids.insert("request", request.to_string());

    macro_rules! share {
        ($key:literal, $table:ident, $column:ident, $of:literal) => {{
            use crate::schema::$table as t;
            let id = Uuid::now_v7();
            diesel::insert_into(t::table)
                .values((
                    t::id.eq(id),
                    t::$column.eq(ids[$of].parse::<Uuid>().unwrap()),
                    t::share_code.eq(format!("pause{}", &id.simple().to_string()[..20])),
                    t::created_by.eq(owner),
                ))
                .execute(&mut conn)
                .expect("a share link");
            ids.insert($key, id.to_string());
        }};
    }
    share!("actorShare", world_actor_shares, actor_id, "actor");
    share!("itemShare", world_item_shares, item_id, "item");
    share!("abilityShare", world_ability_shares, ability_id, "ability");
    share!(
        "collectionShare",
        world_collection_shares,
        collection_id,
        "collection"
    );

    {
        use crate::schema::world_members as m;
        let member: Uuid = m::table
            .filter(m::world_id.eq(world))
            .filter(m::user_id.eq(player))
            .select(m::id)
            .first(&mut conn)
            .expect("the player is a member");
        ids.insert("playerMember", member.to_string());
    }

    ids
}

/// A multipart upload, built the way a browser sends one (GraphQL multipart
/// request spec), for the three mutations that take a file.
fn upload_request(name: &str, ids: &BTreeMap<&'static str, String>) -> Request {
    let (document, variables) = match name {
        "uploadCanvasImage" => (
            "mutation($f: Upload!, $w: UUID!, $s: UUID!) { uploadCanvasImage(worldId: $w, sceneId: $s, kind: PASTED, file: $f) { __typename } }",
            serde_json::json!({ "f": null, "w": ids["world"], "s": ids["scene"] }),
        ),
        "uploadActorImage" => (
            r#"mutation($f: Upload!, $a: UUID!) { uploadActorImage(actorId: $a, role: "portrait", file: $f) { __typename } }"#,
            serde_json::json!({ "f": null, "a": ids["actor"] }),
        ),
        "uploadLoreImage" => (
            "mutation($f: Upload!, $l: UUID!) { uploadLoreImage(loreEntryId: $l, file: $f) { __typename } }",
            serde_json::json!({ "f": null, "l": ids["lore"] }),
        ),
        other => panic!("no upload request for {other}"),
    };
    let mut request =
        Request::new(document).variables(async_graphql::Variables::from_json(variables));
    let mut file = tempfile::tempfile().expect("a temp file");
    std::io::Write::write_all(&mut file, &crate::test_support::tiny_png_bytes()).unwrap();
    request.set_upload(
        "variables.f",
        async_graphql::UploadValue {
            filename: "x.png".to_string(),
            content_type: Some("image/png".to_string()),
            content: file,
        },
    );
    request
}

fn refused(name: &str, response: &Value) -> bool {
    if REPORTS_INSTEAD.contains(&name) {
        return response["data"][name].as_array().is_some_and(|outcomes| {
            !outcomes.is_empty() && outcomes.iter().all(|o| o["reason"] == "PLAY_PAUSED")
        });
    }
    response["errors"].as_array().is_some_and(|errors| {
        errors
            .iter()
            .any(|e| e["extensions"]["code"] == WORLD_PLAY_PAUSED)
    })
}

/// Half two: every gated field, called against a paused world by the one
/// person who runs it and by an operator who plays in it, is refused with
/// the pause's code — and the world records no event while they try.
#[tokio::test]
async fn every_gated_field_refuses_a_paused_world() {
    // The share-link mutations ask whether this instance may publish before
    // anything else, and the answer is process- and database-global. Held for
    // the whole run, as the share tests hold it; see `InstancePublishing`.
    let _publishing =
        crate::graphql::mutations_collection_shares::publishing_gate::publishable_instance();
    let state = crate::test_support::test_app_state();
    let schema = schema(state.clone());

    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let newcomer = insert_test_user(&mut conn);
    let newcomer_operator = insert_test_user(&mut conn);
    for admin in [operator, newcomer_operator] {
        diesel::update(crate::schema::users::table.filter(crate::schema::users::id.eq(admin)))
            .set(crate::schema::users::is_admin.eq(true))
            .execute(&mut conn)
            .unwrap();
    }
    let world = insert_test_world(&mut conn, owner);
    insert_test_world_member(&mut conn, world, operator, "Player");
    insert_test_world_member(&mut conn, world, player, "Player");
    drop(conn);

    let ids = seed(&state, &schema, owner, world, player).await;

    let mut conn = state.db_pool.get().unwrap();
    pause_world(
        &mut conn,
        operator,
        world,
        "Stopping play.",
        TriggerDetail::operator(),
    )
    .expect("paused");
    let events_before = world_event_count(&mut conn, world);
    drop(conn);

    let mut failures = Vec::new();
    for (name, document) in GATED {
        let callers = CALLED_AS
            .iter()
            .find(|(field, _, _)| field == name)
            .map_or(DEFAULT_CALLERS, |(_, callers, _)| *callers);
        let everyone = callers
            .iter()
            .chain(DEFAULT_CALLERS.iter().filter(|who| !callers.contains(who)));
        for who in everyone {
            let user = match who {
                Who::GameMaster => caller(owner, false),
                Who::SiteAdmin => caller(operator, true),
                Who::Player => caller(player, false),
                Who::Newcomer => caller(newcomer, false),
                Who::NewcomerSiteAdmin => caller(newcomer_operator, true),
            };
            let request = if document.starts_with("UPLOAD ") {
                upload_request(name, &ids)
            } else {
                Request::new(fill(document, &ids))
            };
            let response = first_response(&schema, request.data(user)).await;
            if callers.contains(who) {
                if !refused(name, &response) {
                    failures.push(format!("{name} as {who:?}: {response}"));
                }
            } else if response.get("errors").is_none() {
                failures.push(format!(
                    "{name} as {who:?}, who CALLED_AS says cannot reach it, got through: {response}"
                ));
            }
        }
    }

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        world_event_count(&mut conn, world),
        events_before,
        "a refused call recorded a world event, so it wrote before it was refused"
    );

    assert!(
        failures.is_empty(),
        "{} gated calls on a paused world were not refused with {WORLD_PLAY_PAUSED}:\n  {}",
        failures.len(),
        failures.join("\n  "),
    );
}

fn world_event_count(conn: &mut PgConnection, world: Uuid) -> i64 {
    use crate::schema::world_events;
    world_events::table
        .filter(world_events::world_id.eq(world))
        .count()
        .get_result(conn)
        .unwrap()
}
