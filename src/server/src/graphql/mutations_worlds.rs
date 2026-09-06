//! Creating a world, and every setting that hangs off one.
//!
//! `create_world_impl` carries the property the rest of the file is written
//! around: a world and its first scene are created in one transaction, so a
//! world can never exist with zero scenes through this path.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};

use super::*;
use crate::state::AppState;

/// Spec 008 (US1, FR-004/FR-006): creates a world and its one default
/// scene in a single DB transaction — both succeed or both fail, so a
/// world can never exist with zero scenes through this path. Default
/// scene values mirror `create_scene`'s own defaults exactly
/// (data-model.md), inlined here rather than calling that resolver since
/// this needs to run inside the same transaction as the world insert.
/// Factored out of the `create_world` resolver (mirrors this codebase's
/// `_impl` convention, e.g. `mutations_assets.rs`'s
/// `upload_canvas_image_impl`) so it's directly unit-testable without a
/// full GraphQL execution context.
/// What a world's first scene is called.
///
/// **Not the world's name** (spec 026 FR-009f). It used to be, and that meant
/// the first scene every Game Master owns carried their world's name — so
/// sharing that scene in a collection disclosed the world's name to anonymous
/// viewers through the member's own title, with nothing telling them. The
/// collection preview sends no world field at all and was never the leak; the
/// leak was here, in data the author is presumed to have chosen and did not.
///
/// Deliberately plain rather than evocative. The intent recorded in spec 026 is
/// that a new world starts on something playable — a real starter map rather
/// than an empty grid — and `examples/maps/README.md` already names the shape
/// that map should have. It is **not** seeded yet: every file in that directory
/// came from a personal, non-redistributable map collection, and that README
/// says in as many words not to ship them in a release artifact. Naming this
/// scene after art it does not have would be the worse half of the change.
/// When a licensed starter map exists, this constant and the scene it names are
/// the one place to change.
pub const STARTER_SCENE_NAME: &str = "Starting Scene";

pub async fn create_world_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    input: GraphQLCreateWorldInput,
) -> Result<GraphQLWorld, String> {
    let prepared_input = prepare_world_input(
        input,
        crate::admin::default_game_system_id(state).as_deref(),
    )?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let now = Utc::now().naive_utc();

    // A world created on a ruleset that has a pack written for it starts in
    // that pack. Without this a 5e world opened in the generic base pack while
    // `forged-steel` sat installed and unmentioned, and the only way to find
    // it was to go looking in settings for something you had no reason to
    // think existed. An explicit choice always wins; see `pack_targeting` for
    // why an ambiguous one is declined rather than guessed.
    let interface_pack_id = prepared_input.interface_pack_id.or_else(|| {
        let system_id = prepared_input.game_system_id.as_deref()?;
        crate::interface_packs::pack_targeting(&state.directories.interface_packs_dir, system_id)
    });

    let new_world = World {
        id: uuid::Uuid::now_v7(),
        name: prepared_input.name,
        description: prepared_input.description,
        game_system_id: prepared_input.game_system_id,
        interface_pack_id,
        created_by: user_id,
        updated_by: user_id,
        created_at: now,
        updated_at: now,
        session_notes: None,
        allow_player_created_actors: false,
        genie_resource_carryover_enabled: false,
        default_scene_grid_type: "square".to_string(),
        active_scene_id: None,
    };

    let inserted_world = new_world.clone();
    // Spec 022 (FR-002d, ADR-046): Play now shows an empty/unloaded canvas
    // whenever `worlds.active_scene_id` is null — but spec 010 (FR-004)
    // already guarantees every freshly created world has its default
    // scene ready to play immediately, with no separate "create a scene"
    // step. Reconciling the two: the default scene created here is also
    // immediately set as the world's active scene, so a brand-new world
    // is never stuck in the empty-canvas state; `active_scene_id` only
    // stays null for a world where nothing has ever been created/launched
    // (not reachable via normal world creation).
    let default_scene_id = uuid::Uuid::now_v7();
    let world_system_id = inserted_world.game_system_id.clone();
    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;

        conn.transaction(|conn| {
            diesel::insert_into(worlds::table)
                .values(&inserted_world)
                .execute(conn)?;

            let scene_values = (
                scenes::scene_id.eq(default_scene_id),
                scenes::world_id.eq(inserted_world.id),
                scenes::name.eq(STARTER_SCENE_NAME),
                scenes::description.eq::<Option<String>>(None),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(5),
                scenes::grid_type.eq("square"),
                scenes::width.eq(100),
                scenes::height.eq(100),
                scenes::metadata.eq::<Option<serde_json::Value>>(None),
                scenes::owner_id.eq(user_id),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            );

            diesel::insert_into(scenes::table)
                .values(scene_values)
                .execute(conn)?;

            diesel::update(worlds::table.filter(worlds::id.eq(inserted_world.id)))
                .set(worlds::active_scene_id.eq(default_scene_id))
                .execute(conn)?;

            // Whatever the world's system wants doing when a world appears.
            //
            // This was a branch on one system's name inserting that system's
            // session row — the last game system named in shared server code,
            // and the only entry left in `check-system-registry.mjs`'s known
            // list. The row still gets inserted; the pack does it now, and
            // this file no longer knows which system that is or what the row
            // is for (spec 032 T014a2, FR-004, ADR-063).
            //
            // Inside the transaction deliberately: a system that could not set
            // itself up should not leave a half-made world behind.
            crate::world_hooks::run_world_created(
                conn,
                world_system_id.as_deref(),
                crate::world_hooks::WorldCreated {
                    world_id: inserted_world.id,
                    created_by: user_id,
                },
            )?;

            Ok::<_, diesel::result::Error>(())
        })
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|error| world_write_error(error, "Failed to create world").message)?;

    // NOTE: world creation does not insert a world_members owner row.
    // require_world_member() (src/server/src/auth/world_membership.rs,
    // spec 002) falls back to worlds.created_by to compensate for this
    // gap. See that module's doc comment for the full story — fixing it
    // at the source (inserting an owner world_members row here) is a
    // separate, deliberate follow-up, not done as part of this cleanup.
    let mut returned_world = new_world;
    returned_world.active_scene_id = Some(default_scene_id);
    Ok(GraphQLWorld::from(returned_world))
}

/// Spec 011: "Last Session Notes" — a single per-world freeform recap,
/// DM/GM-only to write (contracts/session-notes.md), read by any world
/// member via the existing `world(id)` query's `sessionNotes` field.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldSessionNotesInput {
    pub world_id: uuid::Uuid,
    pub notes: String,
}

/// Testable core of `WorldMutation::update_world_session_notes`, split out
/// so tests don't need a GraphQL `Context` (see `mutations_actors.rs`'s
/// `_impl` convention). DM/GM-only (FR-012). Saving an empty string is a
/// valid, explicit save (FR-013), not rejected as "no change".
pub async fn update_world_session_notes_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldSessionNotesInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may update session notes",
        ));
    }

    let world_id = input.world_id;
    let notes = input.notes;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::session_notes.eq(notes))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update session notes"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 016 (FR-004, T009): assigns/changes a world's active system pack.
/// No such mutation existed before this spec — `game_system_id` could
/// previously only be set at `createWorld` time (and spec 008 removed the
/// UI for that), leaving no way to assign or change it afterward. This is
/// the write half of the new System Settings surface
/// (`WorldSystemSettingsPage.tsx`) that also renders the target system's
/// `legal` notice, per this feature's scope-correction note in tasks.md.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldGameSystemInput {
    pub world_id: uuid::Uuid,
    pub game_system_id: String,
    /// What the caller is acknowledging, from `worldContentInventory`.
    ///
    /// Required when the world holds authored content (FR-028). A digest
    /// rather than a boolean because a boolean can be passed by a caller who
    /// never saw a count — which is the bypass the requirement exists to
    /// prevent — and stays true if the world changed while the dialog was
    /// open. See ADR-065.
    pub acknowledged_digest: Option<String>,
}

/// Testable core of `WorldMutation::update_world_game_system` (see
/// `update_world_session_notes_impl`'s identical shape/rationale).
/// DM/GM-only — mirrors `update_world_session_notes_impl`'s permission
/// check exactly, since assigning a world's ruleset is as GM-scoped a
/// decision as its session recap.
/// Spec 032 (FR-010): set or clear a world's interface pack.
///
/// Mirrors `update_world_game_system_impl`, including its refusal wording,
/// because this is the same kind of fact about a world and the two should not
/// be reached for differently. It differs in two ways, both deliberate: the id
/// is nullable, since clearing a binding means "the default" and that is a
/// real thing to want; and the pack must both exist and validate, because
/// accepting an id for a pack that cannot be applied would manufacture
/// FR-019's degraded state from the one place that knows better.
pub async fn update_world_interface_pack_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: crate::graphql::input_types::UpdateWorldInterfacePackInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change a world's interface pack",
        ));
    }

    let world_id = input.world_id;
    let requested = input
        .interface_pack_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());

    if let Some(id) = requested.as_deref() {
        let packs_dir = state.directories.interface_packs_dir.clone();
        crate::interface_packs::load_validated(&packs_dir, id).map_err(|findings| {
            Error::new(format!(
                "Interface pack '{id}' cannot be applied: {}",
                findings.join("; ")
            ))
        })?;
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let stored = requested.clone();
    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::interface_pack_id.eq(stored))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update world"))?;

    // Everyone in the world re-resolves on receipt, so the table sees the
    // change without reloading (SC-001).
    if let Ok(mut conn) = state.db_pool.get() {
        let _ = crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_WORLD_APPEARANCE_CHANGED,
            Some(serde_json::json!({
                "action": "changed",
                "interfacePackId": requested,
            })),
            user_id,
        );
    }

    Ok(GraphQLWorld::from(updated))
}

pub async fn update_world_game_system_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldGameSystemInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change a world's game system",
        ));
    }

    let world_id = input.world_id;
    let game_system_id = input.game_system_id;
    if game_system_id.trim().is_empty() {
        return Err(Error::new("gameSystemId must not be empty"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let packs_dir = state.directories.interface_packs_dir.clone();
    let systems_dir = state.directories.systems_dir.clone();
    let system_for_pairing = game_system_id.clone();
    let acknowledged = input.acknowledged_digest.clone();
    let target_for_counting = game_system_id.clone();

    let updated = tokio::task::spawn_blocking(move || {
        // FR-030: selecting the system already in force changes nothing and
        // asks nothing. Checked before the counts, so a no-op never presents a
        // warning about content it is not going to affect.
        let current_system: Option<String> = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::game_system_id)
            .first::<Option<String>>(&mut conn)
            .ok()
            .flatten();

        if current_system.as_deref() == Some(system_for_pairing.as_str()) {
            return worlds::table
                .filter(worlds::id.eq(world_id))
                .select(World::as_select())
                .first::<World>(&mut conn);
        }

        // FR-028. The server recomputes rather than trusting what it was
        // handed: the digest says *which* numbers were acknowledged, and a
        // world that gained content while the dialog was open is re-confirmed
        // rather than switched behind the Game Master's back.
        let inventory = crate::graphql::queries::world_content::inventory_of(
            &mut conn,
            &systems_dir,
            world_id,
            Some(&target_for_counting),
        )?;

        if !inventory.is_empty && acknowledged.as_deref() != Some(inventory.digest.as_str()) {
            // Rolled back into an error message by the caller below. Diesel's
            // error type is what this closure can carry, and `NotFound` is the
            // one the caller maps to a refusal rather than a database fault.
            return Err(diesel::result::Error::NotFound);
        }

        // The pack binding is repaired in the same statement that changes the
        // system, because between the two a world is bound to a pack written
        // for a ruleset it no longer plays — and the settings picker only
        // offers packs targeting the current system, so that state is not
        // merely wrong but invisible from the screen that owns it.
        let current: Option<String> = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::interface_pack_id)
            .first::<Option<String>>(&mut conn)
            .ok()
            .flatten();

        let interface_pack_id = crate::interface_packs::pack_after_system_change(
            &packs_dir,
            current.as_deref(),
            &system_for_pairing,
        );

        let updated = diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set((
                worlds::game_system_id.eq(Some(game_system_id.clone())),
                worlds::interface_pack_id.eq(interface_pack_id),
            ))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)?;

        // Announced like every other cross-participant change. A world's
        // ruleset changing is at least as worth telling the table about as its
        // palette, which has been announced since spec 032.
        let _ = crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_WORLD_SYSTEM_CHANGED,
            Some(serde_json::json!({
                "action": "changed",
                "gameSystemId": game_system_id,
            })),
            user_id,
        );

        Ok(updated)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|error| match error {
        diesel::result::Error::NotFound => Error::new(
            "This world holds authored content. Confirm what the change affects before applying it.",
        ),
        _ => Error::new("Failed to update game system"),
    })?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 017 (FR-007): the GM-controlled world setting gating whether the
/// Actor Selection screen's "create your own character" option is shown.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldAllowPlayerCreatedActorsInput {
    pub world_id: uuid::Uuid,
    pub allow: bool,
}

/// Testable core of `WorldMutation::update_world_allow_player_created_actors`
/// (mirrors `update_world_session_notes_impl`'s shape/rationale exactly).
/// DM/GM-only.
pub async fn update_world_allow_player_created_actors_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldAllowPlayerCreatedActorsInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change this world's player-created-actors setting",
        ));
    }

    let world_id = input.world_id;
    let allow = input.allow;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::allow_player_created_actors.eq(allow))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update allow_player_created_actors"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 020 (FR-003, research.md R1): the GM-controlled per-world setting
/// gating whether Genie Session Resource holdings carry over into the
/// next session.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldGenieResourceCarryoverInput {
    pub world_id: uuid::Uuid,
    pub enabled: bool,
}

/// Testable core of `WorldMutation::update_world_genie_resource_carryover`
/// (mirrors `update_world_allow_player_created_actors_impl`'s identical
/// shape/rationale). DM/GM-only.
pub async fn update_world_genie_resource_carryover_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldGenieResourceCarryoverInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change this world's resource carryover setting",
        ));
    }

    let world_id = input.world_id;
    let enabled = input.enabled;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::genie_resource_carryover_enabled.eq(enabled))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update genie_resource_carryover_enabled"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 022 (FR-014): the GM-controlled per-world default grid type
/// applied to newly created scenes.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldDefaultSceneGridTypeInput {
    pub world_id: uuid::Uuid,
    pub grid_type: String,
}

/// Testable core of `WorldMutation::update_world_default_scene_grid_type`
/// (mirrors `update_world_genie_resource_carryover_impl`'s identical
/// shape). DM/GM-only. `grid_type` is validated against the same set the
/// `scenes.grid_type`/`worlds.default_scene_grid_type` CHECK constraints
/// already enforce at the DB layer — this just turns a constraint
/// violation into a clean GraphQL error instead of a raw SQL error.
pub async fn update_world_default_scene_grid_type_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldDefaultSceneGridTypeInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change this world's default scene grid type",
        ));
    }

    if !matches!(input.grid_type.as_str(), "square" | "hex" | "gridless") {
        return Err(Error::new(
            "gridType must be one of \"square\", \"hex\", \"gridless\"",
        ));
    }

    let world_id = input.world_id;
    let grid_type = input.grid_type;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::default_scene_grid_type.eq(grid_type))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update default_scene_grid_type"))?;

    Ok(GraphQLWorld::from(updated))
}

#[derive(Default)]
pub struct WorldMutation;

#[async_graphql::Object]
impl WorldMutation {
    async fn create_world(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWorldInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_world_impl(state, auth_user.user_id, input)
            .await
            .map_err(Error::new)
    }

    async fn update_world_session_notes(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldSessionNotesInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_session_notes_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn update_world_game_system(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldGameSystemInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_game_system_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    /// Spec 032 (FR-010). `null` clears the binding, returning the world to
    /// the base pack.
    async fn update_world_interface_pack(
        &self,
        ctx: &Context<'_>,
        input: crate::graphql::input_types::UpdateWorldInterfacePackInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_interface_pack_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn update_world_allow_player_created_actors(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldAllowPlayerCreatedActorsInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_allow_player_created_actors_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    async fn update_world_genie_resource_carryover(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldGenieResourceCarryoverInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_genie_resource_carryover_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    async fn update_world_default_scene_grid_type(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldDefaultSceneGridTypeInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_default_scene_grid_type_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    async fn rename_world(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        world_name: String,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let world_name = normalize_world_name(&world_name);
        validate_world_name(&world_name).map_err(Error::new)?;
        let now = Utc::now().naive_utc();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let updated = tokio::task::spawn_blocking(move || {
            diesel::update(
                worlds::table
                    .filter(worlds::id.eq(world_id))
                    .filter(worlds::created_by.eq(user_id)),
            )
            .set((
                worlds::name.eq(world_name),
                worlds::updated_by.eq(user_id),
                worlds::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

            worlds::table
                .filter(worlds::id.eq(world_id))
                .select(World::as_select())
                .first::<World>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|error| world_write_error(error, "Failed to rename world"))?;

        match updated {
            Some(world) => Ok(GraphQLWorld::from(world)),
            None => Err(Error::new("Forbidden")),
        }
    }

    async fn delete_world(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
    ) -> GraphQLResult<GraphQLDeleteWorldPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        // Owner only, and this was a real hole: the check below is world
        // *membership*, so before this line existed any member — including a
        // Player who had merely accepted an invite — could delete the whole
        // world. Deleting is the one action with no way back, so it is the
        // one that most needs the narrow gate.
        //
        // Checked before the world is loaded, so a non-owner learns nothing
        // about a world they cannot act on beyond the fact of their own
        // membership, which they already knew.
        if !crate::auth::world_membership::is_owner_of_world(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            id,
        )
        .await?
        {
            return Err(Error::new("Forbidden"));
        }

        let existing = load_visible_world_by_id(state, auth_user.user_id, false, id).await?;

        let Some(world) = existing else {
            return Err(Error::new("World not found"));
        };

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            diesel::delete(worlds::table.filter(worlds::id.eq(id))).execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete world"))?;

        Ok(GraphQLDeleteWorldPayload {
            id: world.id,
            status: "deleted".to_string(),
            message: format!("World '{}' was deleted", world.name),
        })
    }
}
