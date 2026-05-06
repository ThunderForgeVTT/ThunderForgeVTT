//! GraphQL helper functions for context extraction, validation, and data loading.
//!
//! This module contains reusable functions shared across queries, mutations, and subscriptions:
//! - **Context helpers**: Extract AppState and AuthenticatedUser from GraphQL context
//! - **Validators**: Ensure input data meets constraints (world names, reference IDs, etc.)
//! - **Loaders**: Fetch data from database with permission checks and optimizations
//! - **Error handlers**: Convert database errors to user-friendly GraphQL errors

use crate::auth_middleware::AuthenticatedUser;
use crate::models::*;
use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use diesel::result::Error as DieselError;

use crate::schema::*;
use crate::AppState;

pub use async_graphql::Result as GraphQLResultType;

// ============================================================================
// CONTEXT HELPERS
// ============================================================================

/// Extract the AppState from the GraphQL context.
///
/// Returns an error if the context doesn't contain an AppState (which should never
/// happen in production, but may occur in tests or malformed requests).
pub fn app_state<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a AppState> {
    ctx.data::<AppState>()
        .map_err(|_| Error::new("Application state unavailable"))
}

/// Extract the authenticated user from the GraphQL context.
///
/// Returns an error if no user is authenticated (i.e., the request lacks valid auth headers).
pub fn authenticated_user<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a AuthenticatedUser> {
    ctx.data::<AuthenticatedUser>()
        .map_err(|_| Error::new("Authentication required"))
}

/// Extract and validate that the user is an admin.
///
/// Returns an error if the user is not authenticated or lacks admin privileges.
pub fn admin_user<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a AuthenticatedUser> {
    let user = authenticated_user(ctx)?;
    if user.is_admin {
        Ok(user)
    } else {
        Err(Error::new("Admin privileges required"))
    }
}

// ============================================================================
// VALIDATION HELPERS
// ============================================================================

const MIN_WORLD_NAME_LEN: usize = 3;
const MAX_WORLD_NAME_LEN: usize = 64;
const MAX_WORLD_DESCRIPTION_LEN: usize = 600;
const MAX_WORLD_REFERENCE_ID_LEN: usize = 64;

/// Normalize a world name by collapsing multiple spaces into single spaces.
///
/// This is a light normalization that preserves readability while ensuring consistency.
pub fn normalize_world_name(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalize optional text by trimming whitespace and filtering empty strings.
///
/// Returns None if the input is None or becomes empty after normalization.
pub fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

/// Validate a world name against length and character restrictions.
///
/// # Constraints
/// - Length: 1–128 characters
/// - Characters: Alphanumeric, spaces, apostrophes, and punctuation: - _ . , : ! ? ( )
pub fn validate_world_name(name: &str) -> Result<(), String> {
    if name.len() < MIN_WORLD_NAME_LEN || name.len() > MAX_WORLD_NAME_LEN {
        return Err(format!(
            "World name must be between {MIN_WORLD_NAME_LEN} and {MAX_WORLD_NAME_LEN} characters"
        ));
    }

    if !name.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                ' ' | '\'' | '-' | '_' | '.' | ',' | ':' | '!' | '?' | '(' | ')'
            )
    }) {
        return Err(
            "World name may only contain letters, numbers, spaces, apostrophes, and - _ . , : ! ? ( )"
                .to_string(),
        );
    }

    Ok(())
}

/// Validate an optional reference ID (used for game system ID, interface pack ID, etc.).
///
/// # Constraints
/// - If present: ≤256 characters, alphanumeric and - _ . : only
/// - If None: Passes validation (optional field)
pub fn validate_optional_reference_id(label: &str, value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };

    if value.len() > MAX_WORLD_REFERENCE_ID_LEN {
        return Err(format!(
            "{label} must be {MAX_WORLD_REFERENCE_ID_LEN} characters or fewer"
        ));
    }

    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return Err(format!(
            "{label} may only contain letters, numbers, '-', '_', '.', and ':'"
        ));
    }

    Ok(())
}

/// Prepared (validated and normalized) input for world creation.
pub struct PreparedWorldInput {
    pub name: String,
    pub description: Option<String>,
    pub game_system_id: Option<String>,
    pub interface_pack_id: Option<String>,
}

/// Prepare and validate world creation input.
///
/// This combines normalization and validation:
/// 1. Normalize all text fields (collapse spaces, trim)
/// 2. Validate world name (length, characters)
/// 3. Validate optional reference IDs (game system, interface pack)
/// 4. Validate description length
///
/// # Errors
/// Returns a user-friendly error message if any validation fails.
pub fn prepare_world_input(
    input: crate::graphql::input_types::GraphQLCreateWorldInput,
) -> Result<PreparedWorldInput, String> {
    let name = normalize_world_name(&input.name);
    validate_world_name(&name)?;

    let description = normalize_optional_text(input.description);
    if description
        .as_ref()
        .is_some_and(|value| value.len() > MAX_WORLD_DESCRIPTION_LEN)
    {
        return Err(format!(
            "World description must be {MAX_WORLD_DESCRIPTION_LEN} characters or fewer"
        ));
    }

    let game_system_id = normalize_optional_text(input.game_system_id);
    validate_optional_reference_id("Game system ID", game_system_id.as_deref())?;

    let interface_pack_id = normalize_optional_text(input.interface_pack_id);
    validate_optional_reference_id("Interface pack ID", interface_pack_id.as_deref())?;

    Ok(PreparedWorldInput {
        name,
        description,
        game_system_id,
        interface_pack_id,
    })
}

// ============================================================================
// ERROR HANDLERS
// ============================================================================

/// Convert a Diesel database error to a user-friendly GraphQL error.
///
/// Translates specific database error types (e.g., unique constraint violations)
/// into meaningful messages. Falls back to a generic message if the error is unknown.
pub fn world_write_error(error: DieselError, fallback_message: &str) -> Error {
    match error {
        DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
            Error::new("You already own a world with this name")
        }
        _ => Error::new(fallback_message),
    }
}

// ============================================================================
// DATA LOADERS
// ============================================================================

/// Load all available game systems, ordered by title.
pub async fn load_game_systems(state: &AppState) -> GraphQLResult<Vec<GameSystem>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        game_systems::table
            .order(game_systems::title.asc())
            .select(GameSystem::as_select())
            .load::<GameSystem>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query game systems"))
}

/// Load all worlds owned by the given user, ordered by most recently updated.
pub async fn load_owned_worlds(state: &AppState, user_id: uuid::Uuid) -> GraphQLResult<Vec<World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        worlds::table
            .filter(worlds::created_by.eq(user_id))
            .order(worlds::created_at.desc())
            .select(World::as_select())
            .load::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query worlds"))
}

/// Load all worlds in the system (admin use only). Ordered by most recently updated.
pub async fn load_all_worlds(state: &AppState) -> GraphQLResult<Vec<World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        worlds::table
            .order((worlds::updated_at.desc(), worlds::created_at.desc()))
            .select(World::as_select())
            .load::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query worlds"))
}

/// Load all world tokens owned by the given user, ordered by most recently created.
pub async fn load_owned_world_tokens(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<WorldToken>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_tokens::table
            .filter(world_tokens::created_by.eq(user_id))
            .order(world_tokens::created_at.desc())
            .select(WorldToken::as_select())
            .load::<WorldToken>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world tokens"))
}

/// Load all world events owned by the given user, ordered by most recently created.
pub async fn load_owned_world_events(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<WorldEvent>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_events::table
            .filter(world_events::created_by.eq(user_id))
            .order(world_events::created_at.desc())
            .select(WorldEvent::as_select())
            .load::<WorldEvent>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world events"))
}

/// Load all policies owned by the given user, ordered by most recently created.
pub async fn load_owned_policies(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<Policy>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        policies::table
            .filter(policies::created_by.eq(user_id))
            .order(policies::created_at.desc())
            .select(Policy::as_select())
            .load::<Policy>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query policies"))
}

/// Load a single world by ID with permission checks.
///
/// # Permissions
/// - If the user is an admin: Returns the world if it exists
/// - If the user is not an admin: Returns the world only if they own it (created_by match)
/// - Otherwise: Returns Forbidden error
pub async fn load_visible_world_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
) -> GraphQLResult<Option<World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        worlds::table
            .filter(worlds::id.eq(world_id))
            .select(World::as_select())
            .first::<World>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world"))?;

    match found {
        Some(world) if !is_admin && world.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

/// Load a single world token by ID with ownership verification.
///
/// # Permissions
/// Returns the token only if the user owns it (created_by match).
pub async fn load_owned_world_token_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    token_id: String,
) -> GraphQLResult<Option<WorldToken>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        world_tokens::table
            .filter(world_tokens::id.eq(token_id))
            .select(WorldToken::as_select())
            .first::<WorldToken>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world token"))?;

    match found {
        Some(token) if token.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

/// Load a single world event by ID with ownership verification.
///
/// # Permissions
/// Returns the event only if the user owns it (created_by match).
pub async fn load_owned_world_event_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    event_id: i64,
) -> GraphQLResult<Option<WorldEvent>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        world_events::table
            .filter(world_events::id.eq(event_id))
            .select(WorldEvent::as_select())
            .first::<WorldEvent>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world event"))?;

    match found {
        Some(event) if event.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

/// Load a single policy by ID with ownership verification.
///
/// # Permissions
/// Returns the policy only if the user owns it (created_by match).
pub async fn load_owned_policy_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    policy_id: uuid::Uuid,
) -> GraphQLResult<Option<Policy>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        policies::table
            .filter(policies::id.eq(policy_id))
            .select(Policy::as_select())
            .first::<Policy>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query policy"))?;

    match found {
        Some(policy) if policy.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

/// Load a single game system by ID.
pub async fn load_game_system_by_id(
    state: &AppState,
    system_id: uuid::Uuid,
) -> GraphQLResult<Option<GameSystem>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        game_systems::table
            .filter(game_systems::id.eq(system_id))
            .select(GameSystem::as_select())
            .first::<GameSystem>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query game system"))
}

/// Load world_id from a scene_id (used for permission checks).
///
/// # Returns
/// Returns the world_id if the scene exists, otherwise returns an error.
pub async fn get_world_id_from_scene(
    state: &AppState,
    scene_id: uuid::Uuid,
) -> GraphQLResult<uuid::Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let scene = tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select((scenes::world_id,))
            .first::<(uuid::Uuid,)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load scene"))?;

    match scene {
        Some((world_id,)) => Ok(world_id),
        None => Err(Error::new("Scene not found")),
    }
}
