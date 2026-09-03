//! Spec 032's server surface, exercised through the seams a browser actually
//! reaches: the axum router and the GraphQL schema.
//!
//! `interface_packs_tests.rs` and `graphql.rs`'s `world_interface_pack_tests`
//! both call the functions directly, which proves the logic and proves nothing
//! about the wiring. Every failure this file exists to catch is invisible to
//! those tests: a route that was never mounted, a resolver that compiles but
//! was never merged into the mutation root, a handler whose error path returns
//! a shape nobody reads. Each of those ships green and fails for the first
//! Game Master who opens the settings page.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::auth_middleware::AuthenticatedUser;
use crate::test_support::*;

/// Test state's directories point at a temp dir, so the pack directory has to
/// be aimed at the repository's real packs — mirrors
/// `world_interface_pack_tests::state_with_real_packs` deliberately, because a
/// route serving an empty directory answers 200 with `[]` and proves nothing.
fn state_with_real_packs() -> crate::state::AppState {
    let mut state = test_app_state();
    state.directories.interface_packs_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/interface")
        .to_string_lossy()
        .into_owned();
    state
}

/// Mounted at the prefix `main.rs` mounts it at, so the paths asserted below
/// are the paths the web client sends (minus the `/api` nest, which adds
/// nothing this file can break).
async fn get(state: &crate::state::AppState, uri: &str) -> (StatusCode, serde_json::Value) {
    let app = axum::Router::new()
        .nest("/interface-packs", crate::interface_packs::router())
        .with_state(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("router must answer");

    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body must read");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

// ---------------------------------------------------------------------------
// REST, through the router
// ---------------------------------------------------------------------------

/// The listing a Game Master picks from. Serves as the mount check too: an
/// unrouted listing is a 404 here rather than an empty dropdown in the UI.
#[tokio::test]
async fn the_listing_route_answers_with_the_installed_packs_in_title_order() {
    let (status, body) = get(&state_with_real_packs(), "/interface-packs").await;

    assert_eq!(status, StatusCode::OK, "the listing route must be mounted");

    let packs = body.as_array().expect("the listing is a JSON array");
    assert!(
        packs
            .iter()
            .any(|p| p["id"] == crate::interface_packs::BASE_PACK_ID),
        "Forge appears by being in the directory, like anything else: {body}"
    );

    // FR-007: nothing is pinned, so the wire order is the title order. A
    // future "put Forge first" special case would fail right here.
    let titles: Vec<&str> = packs
        .iter()
        .map(|p| p["title"].as_str().expect("every pack is titled"))
        .collect();
    let mut sorted = titles.clone();
    sorted.sort_unstable();
    assert_eq!(titles, sorted, "title order, with nothing pinned");

    // The serialised shape is part of the contract the client reads; camelCase
    // renaming that silently reverted would leave the UI blank, not erroring.
    let forge = packs
        .iter()
        .find(|p| p["id"] == crate::interface_packs::BASE_PACK_ID)
        .unwrap();
    for field in ["id", "title", "version", "description", "targets"] {
        assert!(
            !forge[field].is_null(),
            "summary is missing {field}: {forge}"
        );
    }
}

/// The manifest the browser downloads and applies. Deserialising it back into
/// the spec type is the point: a 200 carrying something `InterfaceManifest`
/// cannot read is a blank interface at the table, not a server error.
#[tokio::test]
async fn a_manifest_route_serves_a_document_the_client_type_can_read() {
    let (status, body) = get(
        &state_with_real_packs(),
        "/interface-packs/forge/manifest.json",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let manifest: pack_system_spec::interface::InterfaceManifest =
        serde_json::from_value(body).expect("the served body must be a manifest");
    assert_eq!(manifest.id, crate::interface_packs::BASE_PACK_ID);
}

/// Fails closed. A pack that is absent (or has drifted out of compliance) must
/// come back as a refusal carrying findings, never as a 200 with a half-usable
/// document — FR-019's degraded state is exactly what that would manufacture.
#[tokio::test]
async fn an_absent_pack_is_refused_with_findings_rather_than_served() {
    let (status, body) = get(
        &state_with_real_packs(),
        "/interface-packs/no-such-pack/manifest.json",
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let findings = body["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("the refusal must say why: {body}"));
    assert!(
        !findings.is_empty(),
        "a refusal with no findings is a shrug"
    );
}

/// The path parameter is the only thing between a URL and the filesystem, and
/// the guard lives in `read_manifest` — which this test reaches the long way
/// round, through axum's own percent-decoding, because a guard that runs after
/// decoding is the only guard that counts.
#[tokio::test]
async fn a_pack_id_that_is_a_path_cannot_reach_outside_the_packs_directory() {
    let state = state_with_real_packs();

    // `%2E%2E` and `%2F` survive URL normalisation and arrive at the handler
    // as `..` and `/`; the sibling `packs/systems` directory is the nearest
    // real thing an escape would land in.
    for hostile in [
        "/interface-packs/%2E%2E/manifest.json",
        "/interface-packs/a%2Fb/manifest.json",
        "/interface-packs/%2E%2E%2Fsystems/manifest.json",
        "/interface-packs/%2E%2E%2F%2E%2E%2Fpacks%2Fsystems/manifest.json",
    ] {
        let (status, body) = get(&state, hostile).await;
        assert_ne!(
            status,
            StatusCode::OK,
            "{hostile} resolved to something: {body}"
        );
    }
}

// ---------------------------------------------------------------------------
// GraphQL, through the schema
// ---------------------------------------------------------------------------

fn schema(state: &crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state.clone())
    .finish()
}

/// The mutation text the web client sends, executed the way `graphql_handler`
/// executes it — the authenticated user arrives as request data, not as an
/// argument.
async fn run_mutation(
    state: &crate::state::AppState,
    user_id: uuid::Uuid,
    world_id: uuid::Uuid,
    pack: Option<&str>,
) -> async_graphql::Response {
    let query = r#"
        mutation SetPack($input: UpdateWorldInterfacePackInput!) {
            updateWorldInterfacePack(input: $input) {
                id
                interfacePackId
            }
        }
    "#;

    let variables = async_graphql::Variables::from_json(serde_json::json!({
        "input": {
            "worldId": world_id,
            "interfacePackId": pack,
        }
    }));

    let request = async_graphql::Request::new(query)
        .variables(variables)
        .data(AuthenticatedUser {
            user_id,
            session_id: uuid::Uuid::now_v7(),
            expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
            is_admin: false,
            role: "User".to_string(),
        });

    schema(state).execute(request).await
}

/// The test that catches a resolver that was written, compiles, is unit-tested,
/// and was never merged into the mutation root. It also pins the names the
/// hand-written client query uses: `updateWorldInterfacePack` and
/// `interfacePackId`, camelCased by async-graphql from the snake_case Rust.
/// Any of those changing fails here rather than at a Game Master's keyboard.
#[tokio::test]
async fn the_mutation_is_reachable_on_the_root_and_a_dm_can_execute_it() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    let response = run_mutation(&state, owner, world_id, Some("forge")).await;

    assert!(
        response.errors.is_empty(),
        "the DM's mutation must execute: {:?}",
        response.errors
    );

    let data = response.data.into_json().expect("a data payload");
    assert_eq!(
        data["updateWorldInterfacePack"]["interfacePackId"], "forge",
        "the mutation returns the world it just changed: {data}"
    );
}

/// The refusal has to survive the trip through the schema intact. An error
/// that arrives as a bare "internal error" leaves the client unable to say
/// what went wrong, which is the same as saying nothing.
#[tokio::test]
async fn a_player_executing_the_mutation_is_told_what_authority_is_required() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player, "Player");
    drop(conn);

    let response = run_mutation(&state, player, world_id, Some("forge")).await;

    let message = response
        .errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| panic!("a player does not choose the table's look"));
    assert!(
        message.contains("DM"),
        "the refusal names the authority required: {message}"
    );
}

/// A nullable input field is easy to lose in transit — an `Option<String>`
/// that arrives as "absent" rather than "explicitly null" would leave the
/// binding untouched and the Game Master pressing the button forever.
#[tokio::test]
async fn an_explicit_null_clears_the_binding_through_the_schema() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    run_mutation(&state, owner, world_id, Some("forge")).await;

    let response = run_mutation(&state, owner, world_id, None).await;
    assert!(
        response.errors.is_empty(),
        "clearing is a legitimate choice: {:?}",
        response.errors
    );

    let data = response.data.into_json().expect("a data payload");
    assert!(
        data["updateWorldInterfacePack"]["interfacePackId"].is_null(),
        "null must return the world to the base pack: {data}"
    );
}

/// Spec 032 T019a, at the layer that can prove it.
///
/// A pool's two numbers must reach the client *as numbers*. When they did not,
/// the only way to draw a bar was to parse the rendered string back apart, and
/// a system writing "4 of 7" instead of "4 / 7" lost its bar with nothing
/// failing anywhere.
///
/// Executed against the built schema rather than asserted against the SDL as a
/// string: a query naming a field the schema does not have is rejected, and
/// one naming a field it does have validates. Deliberately not attempted from
/// the browser — `/api/graphql` requires a CSRF token a hand-rolled `fetch`
/// does not carry, so an e2e version of this returned an empty 401 body and
/// its "does not say unknown field" assertion passed vacuously.
#[tokio::test]
async fn a_declared_value_exposes_its_pool_as_two_numbers() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    let scene_id = insert_test_scene(&mut conn, world_id, owner);
    drop(conn);

    let request = async_graphql::Request::new(
        r#"
            query Probe($sceneId: UUID!) {
                tokenAttributes(sceneId: $sceneId) {
                    tokenId
                    values { id label origin fraction { current max } }
                }
            }
        "#,
    )
    .variables(async_graphql::Variables::from_json(
        serde_json::json!({ "sceneId": scene_id }),
    ))
    .data(AuthenticatedUser {
        user_id: owner,
        session_id: uuid::Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
    });

    let response = schema(&state).execute(request).await;

    assert!(
        response.errors.is_empty(),
        "a query naming `fraction` must validate — without that field a bar \
         has no maximum and the renderer goes back to parsing text: {:?}",
        response.errors
    );
}

// ---------------------------------------------------------------------------
// T061: a bound pack that is no longer installed
// ---------------------------------------------------------------------------

/// FR-018 and FR-020, proved where the condition can actually be created.
///
/// `updateWorldInterfacePack` validates a pack before storing it, so the
/// product refuses on purpose to bind a world to something that is not there.
/// The state this covers arises the other way round: a pack is bound, and later
/// removed from the deployment. An end-to-end test cannot reach it without
/// moving files under a running server, so it is done here against a temporary
/// packs directory instead — `world-appearance.spec.ts` carries the half that
/// *is* reachable from a browser, which is that every surface words the unset
/// state identically.
mod a_bound_pack_that_goes_missing {
    use std::fs;
    use std::path::Path;

    fn repo_packs() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/interface")
    }

    fn copy_pack(from: &Path, into: &Path, id: &str) {
        let target = into.join(id);
        fs::create_dir_all(&target).expect("create pack dir");
        fs::copy(
            from.join(id).join("interface.json"),
            target.join("interface.json"),
        )
        .expect("copy manifest");
    }

    /// The whole cycle: present, gone, restored — with nothing re-bound.
    ///
    /// The world's stored `interface_pack_id` is not touched at any point
    /// here, and that is the point of FR-020: restoring a pack must cost no
    /// manual step. Nothing in this test writes a binding, because nothing in
    /// the product should have to.
    #[test]
    fn is_refused_while_absent_and_loads_again_when_restored_with_no_rebinding() {
        let source = repo_packs();
        let temp = tempfile::tempdir().expect("temp packs dir");
        let dir = temp.path();
        let packs_dir = dir.to_string_lossy().into_owned();

        // A deployment with the base pack and one targeted pack installed.
        copy_pack(&source, dir, "forge");
        copy_pack(&source, dir, "forged-steel");

        assert!(
            crate::interface_packs::load_validated(&packs_dir, "forged-steel").is_ok(),
            "the pack loads while it is installed"
        );

        // The operator removes it. The world's binding is untouched — this is
        // a change to the deployment, not to the world.
        fs::remove_dir_all(dir.join("forged-steel")).expect("uninstall the pack");

        assert!(
            crate::interface_packs::load_validated(&packs_dir, "forged-steel").is_err(),
            "a pack that is not installed must fail closed rather than serve something"
        );

        // FR-018: and the base pack is still there to fall back to, which is
        // what makes the failure above cost nothing. A deployment that lost
        // its fallback along with its pack would have no look at all.
        assert!(
            crate::interface_packs::load_validated(&packs_dir, "forge").is_ok(),
            "the base pack still loads while the chosen one is missing"
        );

        // FR-020: restored, and working again, with no re-binding step —
        // there was no step to take, because the binding never changed.
        copy_pack(&source, dir, "forged-steel");
        assert!(
            crate::interface_packs::load_validated(&packs_dir, "forged-steel").is_ok(),
            "restoring the pack restores full function"
        );
    }

    /// The listing drops it too, so a picker cannot offer what is gone.
    #[test]
    fn is_no_longer_offered_while_it_is_absent() {
        let source = repo_packs();
        let temp = tempfile::tempdir().expect("temp packs dir");
        let dir = temp.path();
        let packs_dir = dir.to_string_lossy().into_owned();

        copy_pack(&source, dir, "forge");
        copy_pack(&source, dir, "forged-steel");

        let listed = |dir: &str| -> Vec<String> {
            crate::interface_packs::list_installed(dir)
                .into_iter()
                .map(|pack| pack.id)
                .collect()
        };

        assert!(listed(&packs_dir).iter().any(|id| id == "forged-steel"));

        fs::remove_dir_all(dir.join("forged-steel")).expect("uninstall the pack");

        let after = listed(&packs_dir);
        assert!(
            !after.iter().any(|id| id == "forged-steel"),
            "an uninstalled pack must not be offered"
        );
        assert!(
            after.iter().any(|id| id == "forge"),
            "and the base pack is still offered"
        );
    }
}
