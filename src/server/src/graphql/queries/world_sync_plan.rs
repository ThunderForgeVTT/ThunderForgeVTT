//! Spec 028 (T022, T033, T034): the `worldSyncPlan` query.
//!
//! See `specs/028-client-world-cache/contracts/graphql-delta-sync.md`. A
//! client sends what it believes it holds for one world; the server answers
//! with what to fetch and what to discard, and says nothing at all about
//! items whose copy is already current — that silence is the whole point,
//! and is what makes reopening an unchanged world nearly free.
//!
//! # Why the plan is computed *from* authorized items
//!
//! The decision worth understanding here is the shape of the code, not the
//! queries. We build a map of what **this caller may see**, and only then
//! hand it to [`thunderforge_cache_core::delta::compute_plan`]. Nothing is
//! filtered afterwards.
//!
//! That ordering is the security property. `compute_plan` is a pure function
//! over the map it is given, so it structurally cannot offer content that
//! never entered the map — there is no "and then remove the forbidden ones"
//! step to forget, and no future edit to that function can reintroduce one.
//! An item the caller may not see therefore cannot appear in `fetch` at all.
//!
//! It also means revocation needs no separate channel. A held item the caller
//! has lost access to is simply missing from the authorized map, and falls
//! into the same `evict` branch as one that was deleted — byte-identical
//! entries, by design (FR-015). The client cannot tell "you may not see this"
//! from "this no longer exists", so it learns nothing (FR-014, FR-047), and
//! cache correctness and permission revocation come out of one mechanism
//! instead of two that can disagree.
//!
//! (This note used to record a disagreement with the contract doc, which
//! listed "client claims an item it may not see" as *omitted from both
//! lists* while `compute_plan` resolved it as "evicted, identically to a
//! deleted item". The contract has since been corrected to match, so there
//! is nothing left to reconcile — kept only because the reasoning is worth
//! having: an unauthorized item and a deleted one must be byte-identical in
//! the response, or the difference between them tells the caller something
//! about content they may not see.)
//!
//! # Authorization is redone on every call
//!
//! Nothing about the caller's rights is cached between calls (T033). Every
//! invocation re-runs `require_world_member` and re-derives DM-ness, because
//! the client's manifest is a claim of *possession*, never of *entitlement* —
//! a stale authorization here would keep serving a removed member for as long
//! as the cache lived.

use async_graphql::{
    Context, Error, ErrorExtensions, InputObject, Result as GraphQLResult, SimpleObject,
};
use diesel::prelude::*;
use std::collections::{BTreeMap, HashSet};
use thunderforge_cache_core::delta::{CurrentItem, SyncPlan, compute_plan};
use thunderforge_cache_core::manifest::CANONICAL_VERSION;
use thunderforge_cache_core::{Fingerprint, HeldItem, ItemId};
use uuid::Uuid;

use crate::auth::scene_visibility::visible_scene_ids;
use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::graphql::{app_state, authenticated_user};
use crate::state::AppState;

/// Upper bound on a single manifest, per the contract's "`held` exceeds a
/// sane bound → rejected; clients must page".
///
/// Sized well above any plausible real world (a world is scenes plus canvas
/// images, not documents), so hitting it means either a bug or an attempt to
/// make the server hold an unbounded client-supplied allocation. Rejecting is
/// safe: a client that legitimately grows past this can page, and the
/// worst case of paging is a slower cold start.
pub const MAX_HELD_ITEMS: usize = 5_000;

/// The message a non-member sees. Deliberately identical to the one
/// `uploadCanvasImage` / `canvasImageAssetsForScene` produce, and identical
/// whether the world exists or not: `require_world_member` cannot distinguish
/// "no membership row" from "no such world", so neither can the caller.
const NOT_A_MEMBER: &str = "user is not a member of this world";

/// One item the client claims to hold.
///
/// Named `HeldItemInput` on the wire to match the contract; the `GraphQL`
/// prefix on the Rust type follows this module's neighbours instead.
#[derive(InputObject, Debug, Clone)]
#[graphql(name = "HeldItemInput")]
pub struct GraphQLHeldItemInput {
    /// `"scene:<uuid>"` or `"asset:<uuid>"`.
    pub id: String,
    /// Lowercase hex SHA-256 the client believes it holds.
    pub fingerprint: String,
}

#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "PlanItem")]
pub struct GraphQLPlanItem {
    pub id: String,
    pub fingerprint: String,
    pub byte_size: i32,
    /// Whether any peer is *reachable* for this world right now (FR-044) —
    /// not whether any peer holds this item.
    ///
    /// The distinction is deliberate (T087). The server does not track which
    /// bytes each client caches and must not start: nothing in the spec asks
    /// for it, and a standing map of who-has-what would be a real privacy
    /// cost paid for an advisory hint. So this is `true` when at least one
    /// other live session is registered for the world, and `false` otherwise.
    /// Its usefulness is letting a client skip peer attempts entirely when it
    /// is alone at the table, which is the common case.
    ///
    /// Advisory either way: a client must behave identically whether it is
    /// true, false, or ignored, because peer transfer is a strict
    /// optimization with a mandatory server fallback (FR-048). That asymmetry
    /// is why reachability is the safe thing to report — a `false` can only
    /// cost a server fetch the client was going to be able to make anyway,
    /// whereas a wrong `true` costs a stall before the same fallback.
    pub peer_available: bool,
}

#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "SyncPlan")]
pub struct GraphQLSyncPlan {
    pub fetch: Vec<GraphQLPlanItem>,
    pub evict: Vec<String>,
    /// Server's view of the caller's budget ceiling, bytes. Advisory, and
    /// currently always null: the client measures its own storage quota
    /// (R8), and the server has no better number to offer. Present in the
    /// schema so supplying one later is not a breaking change.
    pub budget_hint: Option<i32>,
    /// A mismatch invalidates every scene-state fingerprint the client
    /// holds, because the bytes those hashes were taken over are no longer
    /// produced the same way.
    pub canonical_version: i32,
}

/// Why a manifest could not be accepted.
///
/// Malformed input is rejected rather than skipped. Silently dropping an
/// unparseable entry would present as an item that never syncs and never
/// errors — the failure mode hardest to notice and hardest to diagnose.
#[derive(Debug, thiserror::Error)]
pub enum WorldSyncPlanError {
    #[error("{NOT_A_MEMBER}")]
    Forbidden,
    #[error("manifest exceeds the maximum of {max} held items (got {actual}); page the request")]
    TooManyHeldItems { max: usize, actual: usize },
    #[error("malformed held item id: {0}")]
    MalformedItemId(String),
    #[error("malformed fingerprint for {id}: {reason}")]
    MalformedFingerprint { id: String, reason: String },
    #[error("database error: {0}")]
    Database(String),
}

/// Mirrors `mutations_assets::to_graphql_error`: async-graphql's blanket
/// `From<T: Display>` means a second `From` impl would conflict (E0119), so
/// the `FORBIDDEN` extension is attached at the call site instead.
pub fn to_graphql_error(e: WorldSyncPlanError) -> Error {
    let msg = e.to_string();
    if matches!(e, WorldSyncPlanError::Forbidden) {
        Error::new(msg).extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
    } else {
        Error::new(msg)
    }
}

/// Parse the wire manifest into the shared type, rejecting anything
/// malformed.
///
/// Kept separate from the DB work so the bound and the parsing are enforced
/// before a connection is taken out of the pool — a caller sending garbage
/// should not cost a connection.
fn parse_held(held: Vec<GraphQLHeldItemInput>) -> Result<Vec<HeldItem>, WorldSyncPlanError> {
    if held.len() > MAX_HELD_ITEMS {
        return Err(WorldSyncPlanError::TooManyHeldItems {
            max: MAX_HELD_ITEMS,
            actual: held.len(),
        });
    }

    held.into_iter()
        .map(|h| {
            let id = ItemId::from_wire(&h.id)
                .ok_or_else(|| WorldSyncPlanError::MalformedItemId(h.id.clone()))?;
            let fingerprint = Fingerprint::from_hex(&h.fingerprint).map_err(|e| {
                WorldSyncPlanError::MalformedFingerprint {
                    id: h.id.clone(),
                    reason: e.to_string(),
                }
            })?;
            Ok(HeldItem { id, fingerprint })
        })
        .collect()
}

/// Everything in `world_id` that `user_id` is permitted to see, as
/// fingerprints.
///
/// Runs on a blocking connection and does the whole authorization chain
/// itself rather than calling the async `is_dm_of_world`: the role that
/// decides DM-ness is already in hand from `require_world_member`, so
/// re-deriving it asynchronously would take a second pooled connection to
/// answer a question already answered.
fn authorized_current(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> Result<BTreeMap<ItemId, CurrentItem>, WorldSyncPlanError> {
    use crate::schema::{canvas_image_assets, scene_state_fingerprints};

    // (T033) Re-authorized here, on every call, from the database — never
    // from anything the client sent.
    let role = require_world_member(conn, user_id, world_id).map_err(|e| match e {
        WorldMembershipError::NotAMember => WorldSyncPlanError::Forbidden,
        WorldMembershipError::Database(msg) => WorldSyncPlanError::Database(msg),
    })?;
    let is_dm = is_admin || role == "Owner" || role == "GM";

    // Per-object visibility (T034, ADR-050), via the rule stated in
    // `auth::scene_visibility` — the same module the byte route now asks, so
    // a plan and a direct `GET /canvas-assets/{id}` cannot disagree about
    // which assets a player may have. Building the visible-scene set first
    // and deriving everything else from it keeps this query's answer
    // consistent with what the ordinary scene queries would return.
    let visible_scenes: Vec<Uuid> = visible_scene_ids(conn, is_dm, world_id)
        .map_err(|e| WorldSyncPlanError::Database(e.to_string()))?;
    let visible_scene_set: HashSet<Uuid> = visible_scenes.iter().copied().collect();

    let mut current: BTreeMap<ItemId, CurrentItem> = BTreeMap::new();

    // Scene states. A scene with no fingerprint row yet — never mutated
    // since the feature shipped, or awaiting backfill — enters the map with
    // `None`, which `compute_plan` treats as "must fetch". Omitting it
    // instead would read as "your copy is current", which is the one answer
    // that is never safe to guess.
    let fingerprints: Vec<(Uuid, String, i32)> = scene_state_fingerprints::table
        .filter(scene_state_fingerprints::scene_id.eq_any(&visible_scenes))
        .select((
            scene_state_fingerprints::scene_id,
            scene_state_fingerprints::content_hash,
            scene_state_fingerprints::canonical_version,
        ))
        .load(conn)
        .map_err(|e| WorldSyncPlanError::Database(e.to_string()))?;
    let by_scene: BTreeMap<Uuid, (String, i32)> = fingerprints
        .into_iter()
        .map(|(scene_id, hash, version)| (scene_id, (hash, version)))
        .collect();

    for scene_id in &visible_scenes {
        let fingerprint = by_scene.get(scene_id).and_then(|(hash, version)| {
            // A hash taken under a superseded canonical form is not
            // comparable to one the client would compute today, so it is
            // treated as unknown rather than as a mismatch. Both lead to a
            // fetch; only "unknown" also stops us from asserting a wrong
            // fingerprint to the client.
            if *version != CANONICAL_VERSION as i32 {
                return None;
            }
            Fingerprint::from_hex(hash).ok()
        });
        current.insert(
            ItemId::SceneState(*scene_id),
            CurrentItem {
                fingerprint,
                // Scene state is not a byte transfer — it arrives through the
                // ordinary GraphQL/subscription path, not `GET
                // /canvas-assets/{id}` — so there is no size to report and
                // no budget to charge it against.
                byte_size: 0,
            },
        );
    }

    // Canvas image assets. Scoped to the world, then narrowed to what the
    // caller may see: an asset attached to a scene the caller cannot see is
    // dropped here, before `compute_plan`, so it can appear in neither list.
    // A world-scoped asset (`scene_id IS NULL`) belongs to no scene and is
    // visible to any member.
    let assets: Vec<(Uuid, Option<Uuid>, Option<String>, i64)> = canvas_image_assets::table
        .filter(canvas_image_assets::world_id.eq(world_id))
        .select((
            canvas_image_assets::asset_id,
            canvas_image_assets::scene_id,
            canvas_image_assets::content_hash,
            canvas_image_assets::byte_size,
        ))
        .load(conn)
        .map_err(|e| WorldSyncPlanError::Database(e.to_string()))?;

    for (asset_id, scene_id, content_hash, byte_size) in assets {
        if let Some(scene_id) = scene_id
            && !visible_scene_set.contains(&scene_id)
        {
            continue;
        }
        current.insert(
            ItemId::CanvasAsset(asset_id),
            CurrentItem {
                // NULL means the backfill has not reached this row. Never
                // "unchanged" (R3) — and an unparseable hash is treated the
                // same way, because a corrupt stored value must not be
                // handed to a client as truth.
                fingerprint: content_hash
                    .as_deref()
                    .and_then(|h| Fingerprint::from_hex(h).ok()),
                byte_size: byte_size.max(0) as u64,
            },
        );
    }

    Ok(current)
}

/// Compute one caller's plan for one world.
///
/// Separated from the resolver, like `upload_canvas_image_impl`, so the
/// authorize-then-collect-then-plan ordering is directly testable without
/// standing up a `Schema` execution.
pub async fn world_sync_plan_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    held: Vec<GraphQLHeldItemInput>,
) -> Result<SyncPlan, WorldSyncPlanError> {
    let held = parse_held(held)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| WorldSyncPlanError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        let current = authorized_current(&mut conn, user_id, is_admin, world_id)?;
        // The only place the client's claims meet the server's answer, and
        // it happens after the map is already restricted to what the caller
        // may see.
        Ok(compute_plan(&held, &current))
    })
    .await
    .map_err(|e| WorldSyncPlanError::Database(e.to_string()))?
}

/// Render a computed plan onto the wire.
///
/// Takes `peer_available` rather than reading the registry itself, and is a
/// function rather than a `From` impl for that reason: the hint is a property
/// of *who is online*, which this module has no business reaching out for,
/// and passing it in is what lets the plan shape be tested without a live
/// connection registry.
pub fn to_graphql_plan(plan: SyncPlan, peer_available: bool) -> GraphQLSyncPlan {
    GraphQLSyncPlan {
        fetch: plan
            .fetch
            .into_iter()
            .map(|item| GraphQLPlanItem {
                id: item.id.to_wire(),
                fingerprint: item.fingerprint.to_hex(),
                // `Int` is 32-bit in GraphQL. Saturating rather than
                // wrapping: an asset larger than 2GiB cannot exist here
                // (uploads are capped far below it), and if one somehow
                // did, "very large" is a survivable lie where a negative
                // size is not.
                byte_size: i32::try_from(item.byte_size).unwrap_or(i32::MAX),
                // One answer for every item in the plan, because the
                // question this field answers is "is anyone else here",
                // not "does anyone have this".
                peer_available,
            })
            .collect(),
        evict: plan.evict.iter().map(ItemId::to_wire).collect(),
        budget_hint: None,
        canonical_version: CANONICAL_VERSION as i32,
    }
}

#[derive(Default)]
pub struct WorldSyncPlanQuery;

#[async_graphql::Object]
impl WorldSyncPlanQuery {
    async fn world_sync_plan(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        held: Vec<GraphQLHeldItemInput>,
    ) -> GraphQLResult<GraphQLSyncPlan> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let plan =
            world_sync_plan_impl(state, auth_user.user_id, auth_user.is_admin, world_id, held)
                .await
                .map_err(to_graphql_error)?;
        // Read after authorization, never before: a caller who may not see
        // this world must not learn from the shape of a refusal whether
        // anybody is playing in it.
        let peer_available =
            crate::peer_signaling::registry().has_peer_for(world_id, auth_user.user_id);
        Ok(to_graphql_plan(plan, peer_available))
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests against a real Postgres (`DATABASE_URL`); no mocks.
    //! Each test builds its own throwaway fixtures via `crate::test_support`.

    use super::*;
    use crate::test_support::*;

    fn hex(byte: u8) -> String {
        Fingerprint::of_bytes(&[byte]).to_hex()
    }

    fn insert_asset(
        conn: &mut PgConnection,
        world_id: Uuid,
        scene_id: Option<Uuid>,
        owner_id: Uuid,
        content_hash: Option<&str>,
    ) -> Uuid {
        use crate::schema::canvas_image_assets;
        let asset_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(canvas_image_assets::table)
            .values((
                canvas_image_assets::asset_id.eq(asset_id),
                canvas_image_assets::world_id.eq(world_id),
                canvas_image_assets::scene_id.eq(scene_id),
                canvas_image_assets::owner_user_id.eq(owner_id),
                canvas_image_assets::storage_path.eq(format!("test/{asset_id}.webp")),
                canvas_image_assets::original_format.eq("png"),
                canvas_image_assets::width_px.eq(16),
                canvas_image_assets::height_px.eq(16),
                canvas_image_assets::byte_size.eq(1234_i64),
                canvas_image_assets::kind.eq(crate::db_types::CanvasImageAssetKindEnum::Pasted),
                canvas_image_assets::created_by.eq(owner_id),
                canvas_image_assets::updated_by.eq(owner_id),
                canvas_image_assets::created_at.eq(now),
                canvas_image_assets::updated_at.eq(now),
                canvas_image_assets::content_hash.eq(content_hash.map(str::to_string)),
            ))
            .execute(conn)
            .expect("failed to insert test canvas image asset");
        asset_id
    }

    /// `insert_test_scene` hardcodes the scene name, and
    /// `unique_scene_name_per_world` forbids a second one in the same world,
    /// so tests needing two scenes insert them here.
    ///
    /// `hidden` is explicit because the column defaults to **true** (spec 022:
    /// scenes are hidden from players until the GM reveals them). A test that
    /// leaves it to the default gets a scene no player can see, which reads
    /// as this query hiding too much rather than as the fixture doing it.
    fn insert_scene(
        conn: &mut PgConnection,
        world_id: Uuid,
        owner_id: Uuid,
        name: &str,
        hidden: bool,
    ) -> Uuid {
        use crate::schema::scenes;
        let scene_id = Uuid::now_v7();
        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq(name),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(5),
                scenes::grid_type.eq("square"),
                scenes::width.eq(100),
                scenes::height.eq(100),
                scenes::owner_id.eq(owner_id),
                scenes::hidden.eq(hidden),
            ))
            .execute(conn)
            .expect("failed to insert test scene");
        scene_id
    }

    /// T023: the query is actually reachable on the schema, under the names
    /// the contract specifies.
    ///
    /// Worth a test rather than trusting the `MergedObject` list: the schema
    /// is only ever built in `main`, so a registration mistake — or a
    /// duplicate `SyncPlan`/`PlanItem` type name added later — surfaces as a
    /// startup panic in production rather than a compile error here.
    #[test]
    fn the_query_is_registered_under_the_contracts_names() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        assert!(sdl.contains("worldSyncPlan("), "query must be on the root");
        assert!(sdl.contains("type SyncPlan {"));
        assert!(sdl.contains("type PlanItem {"));
        assert!(sdl.contains("input HeldItemInput {"));
        assert!(sdl.contains("canonicalVersion: Int!"));
        assert!(sdl.contains("peerAvailable: Boolean!"));
    }

    /// T040: a non-member must be refused in exactly the way every other
    /// non-member access is refused — same message, same `FORBIDDEN` code —
    /// so the response cannot be used to probe whether a world exists.
    #[tokio::test]
    async fn non_member_is_refused_identically_to_any_other_non_member_access() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let stranger_id = insert_test_user(&mut conn);
        drop(conn);

        let err = world_sync_plan_impl(&state, stranger_id, false, world_id, vec![])
            .await
            .expect_err("a non-member must not receive a plan");

        assert!(matches!(err, WorldSyncPlanError::Forbidden));
        assert_eq!(err.to_string(), "user is not a member of this world");

        // The same shape as `uploadCanvasImage`'s refusal, which is what
        // "identically to any other non-member access" has to mean in
        // practice: message and extension code both.
        let gql = to_graphql_error(err);
        assert_eq!(gql.message, "user is not a member of this world");
        assert_eq!(
            gql.extensions
                .as_ref()
                .and_then(|ext| ext.get("code"))
                .map(|v| format!("{v:?}")),
            Some(format!(
                "{:?}",
                async_graphql::Value::from("FORBIDDEN".to_string())
            ))
        );
    }

    /// T040 (the disclosure half): a world that does not exist must produce
    /// the byte-identical refusal a real-but-forbidden world produces.
    /// Anything else turns this query into an existence oracle.
    #[tokio::test]
    async fn nonexistent_world_is_refused_identically_to_a_forbidden_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let real_world = insert_test_world(&mut conn, owner_id);
        let stranger_id = insert_test_user(&mut conn);
        drop(conn);

        let forbidden = world_sync_plan_impl(&state, stranger_id, false, real_world, vec![])
            .await
            .expect_err("existing world, non-member");
        let missing = world_sync_plan_impl(&state, stranger_id, false, Uuid::now_v7(), vec![])
            .await
            .expect_err("no such world");

        assert_eq!(forbidden.to_string(), missing.to_string());
        assert!(matches!(forbidden, WorldSyncPlanError::Forbidden));
        assert!(matches!(missing, WorldSyncPlanError::Forbidden));
    }

    /// T041: a client claiming an item it may not see must learn nothing
    /// from the answer.
    ///
    /// Note what "nothing" means here, because the contract doc's summary
    /// table and `compute_plan` (already implemented and tested) do not read
    /// the same way. `compute_plan` puts *any* held item that is not in the
    /// authorized map into `evict` — a forbidden one and a deleted one
    /// produce byte-identical entries. That is the disclosure guarantee the
    /// shared crate actually implements, and it is a real one: the caller
    /// cannot tell "you may not see this" from "this does not exist",
    /// because both are the same three words on the wire. What must never
    /// happen is the item appearing in `fetch`, which would hand over the
    /// content itself.
    ///
    /// The hidden scene here is the honest version of this test: the player
    /// is a genuine member of the world, so only the per-object visibility
    /// rule stands between them and the item.
    #[tokio::test]
    async fn a_claim_on_an_item_the_caller_may_not_see_reveals_nothing() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let visible_scene = insert_scene(&mut conn, world_id, owner_id, "Visible Scene", false);
        let hidden_scene = insert_scene(&mut conn, world_id, owner_id, "Hidden Scene", true);
        let hidden_asset = insert_asset(
            &mut conn,
            world_id,
            Some(hidden_scene),
            owner_id,
            Some(&hex(1)),
        );
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        // The player claims to hold the hidden scene's state, the hidden
        // scene's asset, and an item that never existed at all — claims of
        // possession, never of entitlement.
        let never_existed = Uuid::now_v7();
        let held = vec![
            GraphQLHeldItemInput {
                id: ItemId::SceneState(hidden_scene).to_wire(),
                fingerprint: hex(9),
            },
            GraphQLHeldItemInput {
                id: ItemId::CanvasAsset(hidden_asset).to_wire(),
                fingerprint: hex(1),
            },
            GraphQLHeldItemInput {
                id: ItemId::CanvasAsset(never_existed).to_wire(),
                fingerprint: hex(8),
            },
        ];

        let plan = world_sync_plan_impl(&state, player_id, false, world_id, held)
            .await
            .expect("a member must receive a plan");

        assert!(
            !plan
                .fetch
                .iter()
                .any(|i| i.id == ItemId::SceneState(hidden_scene)
                    || i.id == ItemId::CanvasAsset(hidden_asset)),
            "unauthorized items must never be offered for fetch"
        );

        // Forbidden and nonexistent are the same answer, which is what makes
        // the answer useless as an oracle.
        assert!(plan.evict.contains(&ItemId::CanvasAsset(hidden_asset)));
        assert!(plan.evict.contains(&ItemId::SceneState(hidden_scene)));
        assert!(plan.evict.contains(&ItemId::CanvasAsset(never_existed)));

        // ...while the scene the player *may* see is still planned for,
        // proving the omissions above are targeted rather than a blanket
        // empty plan.
        assert!(
            plan.fetch
                .iter()
                .any(|i| i.id == ItemId::SceneState(visible_scene)),
            "the visible scene must still appear in the plan"
        );

        // The same call as the DM: the item is real, and the DM is told so.
        // Without this the test would also pass against a server that had
        // simply lost the asset.
        let dm_plan = world_sync_plan_impl(&state, owner_id, false, world_id, vec![])
            .await
            .expect("the owner must receive a plan");
        assert!(
            dm_plan
                .fetch
                .iter()
                .any(|i| i.id == ItemId::CanvasAsset(hidden_asset)),
            "the hidden asset must be visible to the world's DM"
        );
    }

    /// T034: losing access and being deleted must be indistinguishable. An
    /// asset the caller may no longer see and an asset that never existed
    /// must produce identical `evict` entries and no `fetch` entry at all —
    /// the client cannot tell them apart, and must not try.
    #[tokio::test]
    async fn revoked_and_deleted_items_are_both_plain_evictions() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let hidden_scene = insert_scene(&mut conn, world_id, owner_id, "Hidden Scene", true);
        let revoked_asset = insert_asset(
            &mut conn,
            world_id,
            Some(hidden_scene),
            owner_id,
            Some(&hex(2)),
        );
        let deleted_asset = Uuid::now_v7(); // never inserted: "gone"
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let plan = world_sync_plan_impl(
            &state,
            player_id,
            false,
            world_id,
            vec![
                GraphQLHeldItemInput {
                    id: ItemId::CanvasAsset(revoked_asset).to_wire(),
                    fingerprint: hex(2),
                },
                GraphQLHeldItemInput {
                    id: ItemId::CanvasAsset(deleted_asset).to_wire(),
                    fingerprint: hex(3),
                },
            ],
        )
        .await
        .expect("a member must receive a plan");

        assert!(plan.evict.contains(&ItemId::CanvasAsset(revoked_asset)));
        assert!(plan.evict.contains(&ItemId::CanvasAsset(deleted_asset)));
        assert!(
            plan.fetch.is_empty(),
            "neither a revoked nor a deleted item may be offered for fetch"
        );
    }

    /// T033: authorization is redone per call. The same manifest, from the
    /// same user, must stop yielding content the moment membership ends —
    /// there is nothing cached from the earlier successful call.
    #[tokio::test]
    async fn authorization_is_recomputed_on_every_call() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        world_sync_plan_impl(&state, player_id, false, world_id, vec![])
            .await
            .expect("a member must receive a plan");

        let mut conn = state.db_pool.get().unwrap();
        use crate::schema::world_members;
        diesel::delete(
            world_members::table
                .filter(world_members::world_id.eq(world_id))
                .filter(world_members::user_id.eq(player_id)),
        )
        .execute(&mut conn)
        .expect("failed to revoke membership");
        drop(conn);

        let err = world_sync_plan_impl(&state, player_id, false, world_id, vec![])
            .await
            .expect_err("a removed member must be refused on the very next call");
        assert!(matches!(err, WorldSyncPlanError::Forbidden));
    }

    /// The contract's core win: an item whose held fingerprint matches the
    /// server's is omitted from both lists, while a NULL server hash is
    /// always fetched rather than assumed unchanged (R3).
    #[tokio::test]
    async fn matched_items_are_silent_and_null_hashes_are_always_fetched() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let matched = insert_asset(&mut conn, world_id, Some(scene_id), owner_id, Some(&hex(4)));
        let unhashed = insert_asset(&mut conn, world_id, Some(scene_id), owner_id, None);
        drop(conn);

        let plan = world_sync_plan_impl(
            &state,
            owner_id,
            false,
            world_id,
            vec![
                GraphQLHeldItemInput {
                    id: ItemId::CanvasAsset(matched).to_wire(),
                    fingerprint: hex(4),
                },
                GraphQLHeldItemInput {
                    id: ItemId::CanvasAsset(unhashed).to_wire(),
                    fingerprint: hex(5),
                },
            ],
        )
        .await
        .expect("the owner must receive a plan");

        assert!(
            !plan
                .fetch
                .iter()
                .any(|i| i.id == ItemId::CanvasAsset(matched))
                && !plan.evict.contains(&ItemId::CanvasAsset(matched)),
            "a matching fingerprint must produce silence"
        );
        assert!(
            plan.fetch
                .iter()
                .any(|i| i.id == ItemId::CanvasAsset(unhashed)),
            "a NULL server hash must always be fetched, never assumed unchanged"
        );
    }

    /// The contract rejects malformed input rather than coercing it: a bad
    /// fingerprint is a bug worth surfacing, and silently treating it as a
    /// miss would re-fetch forever while looking healthy.
    #[tokio::test]
    async fn malformed_manifest_entries_are_rejected_not_coerced() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let bad_fp = world_sync_plan_impl(
            &state,
            owner_id,
            false,
            world_id,
            vec![GraphQLHeldItemInput {
                id: ItemId::CanvasAsset(Uuid::now_v7()).to_wire(),
                fingerprint: "NOTHEX".to_string(),
            }],
        )
        .await
        .expect_err("a malformed fingerprint must be rejected");
        assert!(matches!(
            bad_fp,
            WorldSyncPlanError::MalformedFingerprint { .. }
        ));

        let bad_id = world_sync_plan_impl(
            &state,
            owner_id,
            false,
            world_id,
            vec![GraphQLHeldItemInput {
                id: "compendium:not-a-thing".to_string(),
                fingerprint: hex(6),
            }],
        )
        .await
        .expect_err("an unknown item-id prefix must be rejected");
        assert!(matches!(bad_id, WorldSyncPlanError::MalformedItemId(_)));

        let too_many = world_sync_plan_impl(
            &state,
            owner_id,
            false,
            world_id,
            (0..MAX_HELD_ITEMS + 1)
                .map(|_| GraphQLHeldItemInput {
                    id: ItemId::CanvasAsset(Uuid::now_v7()).to_wire(),
                    fingerprint: hex(7),
                })
                .collect(),
        )
        .await
        .expect_err("an oversized manifest must be rejected");
        assert!(matches!(
            too_many,
            WorldSyncPlanError::TooManyHeldItems { .. }
        ));
    }

    /// T087: the hint answers "is anyone else at this table", and it answers
    /// it for the whole plan at once — because the server does not know, and
    /// deliberately never learns, which bytes any client actually holds.
    ///
    /// Exercised through the process registry the resolver reads, rather than
    /// by passing a literal, so a change that stops the resolver's question
    /// ("is a peer reachable") from matching the registry's answer shows up
    /// here.
    #[tokio::test]
    async fn peer_available_is_false_alone_and_true_once_another_session_is_live() {
        use crate::peer_signaling::registry;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let other_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, other_id, "Player");
        let asset_id = insert_asset(&mut conn, world_id, None, owner_id, Some(&hex(1)));
        drop(conn);

        async fn plan_now(
            state: &AppState,
            owner_id: Uuid,
            world_id: Uuid,
            peers: bool,
        ) -> GraphQLSyncPlan {
            let p = world_sync_plan_impl(state, owner_id, false, world_id, vec![])
                .await
                .expect("the owner must receive a plan");
            to_graphql_plan(p, peers)
        }

        // Alone. The owner's own session does not count as a peer to itself —
        // a second tab shares the browser origin, and therefore shares the
        // very cache a transfer would be filling.
        let (_own, _own_rx) = registry().register(world_id, format!("own-{world_id}"), owner_id);
        let alone = plan_now(
            &state,
            owner_id,
            world_id,
            registry().has_peer_for(world_id, owner_id),
        )
        .await;
        assert!(
            alone
                .fetch
                .iter()
                .any(|i| i.id == ItemId::CanvasAsset(asset_id).to_wire()),
            "the fixture asset must actually be in the plan, or this asserts nothing"
        );
        assert!(
            alone.fetch.iter().all(|i| !i.peer_available),
            "with nobody else connected a client must be told not to bother trying peers"
        );

        // Somebody else joins.
        let (theirs, _their_rx) =
            registry().register(world_id, format!("theirs-{world_id}"), other_id);
        let together = plan_now(
            &state,
            owner_id,
            world_id,
            registry().has_peer_for(world_id, owner_id),
        )
        .await;
        assert!(
            together.fetch.iter().all(|i| i.peer_available),
            "another live session in the same world makes peers worth attempting"
        );

        // And it is reachability, not history: when they go, it goes.
        drop(theirs);
        let after = plan_now(
            &state,
            owner_id,
            world_id,
            registry().has_peer_for(world_id, owner_id),
        )
        .await;
        assert!(after.fetch.iter().all(|i| !i.peer_available));
    }
}
