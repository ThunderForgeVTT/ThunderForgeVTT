//! The closed list of what a pause refuses, over the schema the product
//! actually serves (spec 051 T074, contracts/live-play-lock.md).
//!
//! # Why this is in the app crate
//!
//! `thunderforge_server::graphql::play_pause_surface_tests` closes the list
//! for the server's own roots, and cannot see anything else: a system pack's
//! root fields are merged in here, in `schema_roots.rs`. That left a pack's
//! mutations outside the list, and Genie's thirteen went ungated. So this test
//! reads the **merged** schema, with the same two halves:
//!
//! 1. **Nothing is unclassified.** A root field the server contributes is held
//!    to the server's tables, read from the server rather than copied. A root
//!    field a pack contributes — query, mutation or subscription — is in
//!    exactly one table of exactly one pack's `PackSurface`, and in none of
//!    the server's.
//! 2. **The classification is honest.** Every field a pack lists as gated is
//!    called against a paused world, as its Game Master and as a site admin
//!    who is a member, and must answer `WORLD_PLAY_PAUSED` — and the world
//!    records no event while they try.
//!
//! Packs are collected through `inventory`, never named, for the reason
//! `system_packs.rs` gives. The server's gated fields are executed by the
//! server's own test; this one executes the packs'.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use async_graphql::{ObjectType, Request, Schema, SubscriptionType};
use diesel::prelude::*;
use futures_util::StreamExt as _;
use serde_json::Value;
use uuid::Uuid;

use thunderforge_server::auth_middleware::AuthenticatedUser;
use thunderforge_server::graphql::play_pause_surface_tables::{
    GATED, NOT_WORLD_SCOPED, OPERATOR, PLAY_STARTING_QUERIES,
};
use thunderforge_server::graphql::{MutationRoot, QueryRoot, SubscriptionRoot};
use thunderforge_server::play_pause::gate::WORLD_PLAY_PAUSED;
use thunderforge_server::play_pause::surface::pack_surfaces;
use thunderforge_server::play_pause::{TriggerDetail, pause_world};
use thunderforge_server::state::AppState;
use thunderforge_server::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

use crate::schema_roots::{AppMutationRoot, AppQueryRoot, AppSchema};

/// Root field names, by root type.
#[derive(Default)]
struct Roots {
    query: BTreeSet<String>,
    mutation: BTreeSet<String>,
    subscription: BTreeSet<String>,
}

impl Roots {
    fn kind_of(&self, field: &str) -> &'static str {
        if self.query.contains(field) {
            "query"
        } else if self.mutation.contains(field) {
            "mutation"
        } else {
            "subscription"
        }
    }

    fn all(&self) -> impl Iterator<Item = &String> {
        self.query
            .iter()
            .chain(&self.mutation)
            .chain(&self.subscription)
    }
}

/// Read from introspection rather than the SDL, so a description never reads
/// as a field.
async fn roots<Q, M, S>(schema: &Schema<Q, M, S>) -> Roots
where
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
{
    let response = schema
        .execute(
            "{ __schema { queryType { fields { name } } mutationType { fields { name } } \
             subscriptionType { fields { name } } } }",
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let names = |root: &str| -> BTreeSet<String> {
        data["__schema"][root]["fields"]
            .as_array()
            .unwrap_or_else(|| panic!("introspection gave no {root}"))
            .iter()
            .map(|f| f["name"].as_str().unwrap().to_string())
            .collect()
    };
    Roots {
        query: names("queryType"),
        mutation: names("mutationType"),
        subscription: names("subscriptionType"),
    }
}

fn server_schema() -> Schema<QueryRoot, MutationRoot, SubscriptionRoot> {
    Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        SubscriptionRoot,
    )
    .finish()
}

fn merged_schema() -> AppSchema {
    Schema::build(
        AppQueryRoot::default(),
        AppMutationRoot::default(),
        SubscriptionRoot,
    )
    .finish()
}

fn in_server_tables(field: &str) -> Vec<&'static str> {
    [
        ("GATED", GATED.iter().any(|(name, _)| *name == field)),
        ("NOT_WORLD_SCOPED", NOT_WORLD_SCOPED.contains(&field)),
        ("OPERATOR", OPERATOR.contains(&field)),
    ]
    .into_iter()
    .filter_map(|(table, listed)| listed.then_some(table))
    .collect()
}

/// Half one: every root field of the merged schema is classified once.
#[tokio::test]
async fn every_root_field_of_the_merged_schema_is_classified_once() {
    let merged = roots(&merged_schema()).await;
    let server = roots(&server_schema()).await;
    let mut problems = Vec::new();

    // The server's own, to the server's tables. Queries are reads unless they
    // start play, which the server's test already requires be gated.
    for field in merged.mutation.iter().chain(&merged.subscription) {
        if !server.mutation.contains(field) && !server.subscription.contains(field) {
            continue;
        }
        match in_server_tables(field).as_slice() {
            [_] => {}
            [] => problems.push(format!("{field}: a server field in no table")),
            many => problems.push(format!("{field}: in {}", many.join(" and "))),
        }
    }

    // The packs' own, to exactly one pack table.
    let pack_fields: BTreeSet<&String> = merged
        .all()
        .filter(|field| {
            !server.query.contains(*field)
                && !server.mutation.contains(*field)
                && !server.subscription.contains(*field)
        })
        .collect();
    assert!(
        !pack_fields.is_empty(),
        "the merged schema has no field the server lacks; is a pack's root still in schema_roots.rs?"
    );

    let mut claimed: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for surface in pack_surfaces() {
        let tables: [(&str, Vec<&str>); 3] = [
            (
                "gated",
                surface.gated.iter().map(|(name, _)| *name).collect(),
            ),
            ("not_world_scoped", surface.not_world_scoped.to_vec()),
            ("reads", surface.reads.to_vec()),
        ];
        for (table, names) in tables {
            for name in names {
                claimed
                    .entry(name)
                    .or_default()
                    .push(format!("{}.{table}", surface.system_id));
            }
        }
    }

    for field in &pack_fields {
        match claimed.get(field.as_str()).map(Vec::as_slice) {
            Some([_]) => {}
            None | Some([]) => problems.push(format!(
                "{field}: a pack {} in no pack's PackSurface",
                merged.kind_of(field)
            )),
            Some(many) => problems.push(format!("{field}: in {}", many.join(" and "))),
        }
        if !in_server_tables(field).is_empty() {
            problems.push(format!(
                "{field}: a pack field listed in the server's tables"
            ));
        }
    }

    for (name, tables) in &claimed {
        let field = name.to_string();
        if !pack_fields.contains(&field) {
            problems.push(format!(
                "{name}: in {}, but no pack contributes that root field. Renamed or removed?",
                tables.join(" and ")
            ));
        } else if tables.iter().any(|t| t.ends_with(".reads")) && !merged.query.contains(&field) {
            problems.push(format!(
                "{name}: listed as a read, but it is a {}",
                merged.kind_of(name)
            ));
        }
    }

    for query in PLAY_STARTING_QUERIES {
        if !merged.query.contains(*query) {
            problems.push(format!(
                "{query}: starts play, and is not in the merged schema"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "every root field of the merged schema is gated against a paused world, \
         or classified with a reason — a pack classifies its own in a `PackSurface`:\n  {}",
        problems.join("\n  "),
    );
}

/// A request that calls some other field would pass for the wrong reason.
#[test]
fn every_gated_pack_request_calls_its_own_field() {
    for surface in pack_surfaces() {
        let steps = surface
            .gated
            .iter()
            .copied()
            .chain(surface.seed.iter().map(|s| (s.field, s.document)));
        for (name, document) in steps {
            let selection = &document[document.find('{').expect("a selection") + 1..];
            let called: String = selection
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric())
                .collect();
            assert_eq!(
                called, name,
                "{}: `{name}`'s request calls `{called}`",
                surface.system_id
            );
        }
    }
}

// --- half two -------------------------------------------------------------

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

/// Run one document and return its first response as JSON. A stream that
/// says nothing for five seconds was opened, which is the finding.
async fn first_response(schema: &AppSchema, request: Request) -> Value {
    let mut stream = schema.execute_stream(request);
    match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
        Ok(response) => serde_json::to_value(response.expect("one response")).unwrap(),
        Err(_) => serde_json::json!({ "timedOut": true }),
    }
}

fn fill(template: &str, ids: &BTreeMap<&'static str, String>) -> String {
    let mut filled = template.to_string();
    for (key, value) in ids {
        filled = filled.replace(&format!("{{{key}}}"), value);
    }
    filled
}

fn world_event_count(state: &AppState, world: Uuid) -> i64 {
    use thunderforge_server::schema::world_events;
    let mut conn = state.db_pool.get().unwrap();
    world_events::table
        .filter(world_events::world_id.eq(world))
        .count()
        .get_result(&mut conn)
        .unwrap()
}

/// Half two: every gated pack field, called against a paused world by the
/// one person who runs it and by an operator who plays in it, is refused with
/// the pause's code.
#[tokio::test]
async fn every_gated_pack_field_refuses_a_paused_world() {
    let state = test_app_state();
    let schema = Schema::build(
        AppQueryRoot::default(),
        AppMutationRoot::default(),
        SubscriptionRoot,
    )
    .data(state.clone())
    .finish();

    let mut failures = Vec::new();
    let mut called = 0;
    // A world of its own per pack, so one pack's seed never meets another's.
    for surface in pack_surfaces().filter(|s| !s.gated.is_empty()) {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let operator = insert_test_user(&mut conn);
        diesel::update(
            thunderforge_server::schema::users::table
                .filter(thunderforge_server::schema::users::id.eq(operator)),
        )
        .set(thunderforge_server::schema::users::is_admin.eq(true))
        .execute(&mut conn)
        .unwrap();
        let world = insert_test_world(&mut conn, owner);
        insert_test_world_member(&mut conn, world, operator, "Player");
        drop(conn);

        let mut ids: BTreeMap<&'static str, String> = BTreeMap::new();
        ids.insert("world", world.to_string());
        for step in surface.seed {
            let request = Request::new(fill(step.document, &ids)).data(caller(owner, false));
            let response = first_response(&schema, request).await;
            assert!(
                response.get("errors").is_none(),
                "{}: seeding `{}` failed: {response}",
                surface.system_id,
                step.key
            );
            let id = response["data"][step.field]
                .pointer(step.pick)
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "{}: seeding `{}` gave {response}",
                        surface.system_id, step.key
                    )
                });
            ids.insert(step.key, id.to_string());
        }

        let mut conn = state.db_pool.get().unwrap();
        pause_world(
            &mut conn,
            operator,
            world,
            "Stopping play.",
            TriggerDetail::operator(),
        )
        .expect("paused");
        drop(conn);
        let events_before = world_event_count(&state, world);

        for (name, document) in surface.gated {
            for (who, user) in [
                ("GameMaster", caller(owner, false)),
                ("SiteAdmin", caller(operator, true)),
            ] {
                called += 1;
                let request = Request::new(fill(document, &ids)).data(user);
                let response = first_response(&schema, request).await;
                let refused = response["errors"].as_array().is_some_and(|errors| {
                    errors
                        .iter()
                        .any(|e| e["extensions"]["code"] == WORLD_PLAY_PAUSED)
                });
                if !refused {
                    failures.push(format!("{}.{name} as {who}: {response}", surface.system_id));
                }
            }
        }

        let events_after = world_event_count(&state, world);
        if events_after != events_before {
            failures.push(format!(
                "{}: {} world events recorded by refused calls, so something wrote before it \
                 was refused",
                surface.system_id,
                events_after - events_before
            ));
        }
    }

    assert!(
        called > 0,
        "no pack submitted a gated field; is inventory linking the packs?"
    );
    assert!(
        failures.is_empty(),
        "{} gated pack calls on a paused world were not refused with {WORLD_PLAY_PAUSED}:\n  {}",
        failures.len(),
        failures.join("\n  "),
    );
}
