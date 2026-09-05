//! Spec 034: establishing, acknowledging and removing a world's repository
//! connection.
//!
//! Four mutations, all owner-level (FR-002), and **none of them writes to a
//! world's lore**. That is the property that makes the first delivery safe by
//! construction rather than by care, and it is visible here as the absence of
//! any statement touching a lore table — the one exception being the test that
//! proves removal leaves those tables alone.
//!
//! # Authority is re-checked, never captured
//!
//! FR-003. Every mutation asks [`require_world_owner`] on the way in, against
//! the caller making *this* call. Nothing stores who created a connection as
//! though that settled anything: ownership of a world changes, and a
//! connection established by a former owner must stop being manageable by
//! them the moment they stop owning the world. The same rule applies per
//! synchronisation run, which is the scheduler's half of the requirement.
//!
//! # The grant boundary
//!
//! [`begin_grant`] and [`complete_grant`] are the two calls this module makes
//! into the host adapter, and they are the seam FR-004a asks to be pointed at.
//! Their implementations belong to **T014, `src/server/src/repo_host.rs`** —
//! the only file in `src/server` permitted to name a host. Until it exists
//! they answer with a plain refusal rather than a panic: an unfinished
//! integration must present as "this instance cannot do that yet", which is
//! the same thing an operator who has registered nothing sees, and never as a
//! crashed request.

use async_graphql::Enum;
use async_graphql::{
    Context, Error, ErrorExtensions, InputObject, Object, Result as GraphQLResult, SimpleObject,
};
use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::queries::lore_sync::{
    GraphQLLoreRepositoryConnection, LoreSyncState, load_connection, require_world_owner,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::LoreRepositoryConnection;
use crate::schema::lore_repository_connections;
use crate::state::AppState;
use thunderforge_repo_host::RepoHost as _;

/// Where the branch defaults, when the caller expresses no preference.
const DEFAULT_BRANCH: &str = "main";
/// Where a world's files go inside the repository, when the caller expresses
/// no preference. A subdirectory rather than the root, because FR-032
/// requires a first synchronisation into a repository with existing files to
/// leave them alone, and a default that writes at the root makes a collision
/// with the user's own `README.md` the *expected* first experience.
const DEFAULT_DIRECTORY: &str = "lore";

/// One permission the user is being asked for, and why they are being asked.
///
/// The reason travels with the permission rather than being looked up beside
/// it, so a consent screen physically cannot render the list without it —
/// FR-036e's issue-opening permission is wider than "write the files we
/// mirror", and a user not told why would be right to refuse.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "GrantedPermission")]
pub struct GraphQLGrantedPermission {
    /// The host's own name for it, for an operator reading an audit log.
    pub id: String,
    /// What it allows, in the user's words.
    pub summary: String,
    /// Why this feature asks for it.
    pub reason: String,
}

/// Where the user is sent to grant access, and what they will be asked for.
///
/// **This is the one place a host-specific concept legitimately appears**
/// (FR-004b) — and it appears as an opaque URL plus a list of sentences, which
/// is as much as a client needs and as little as FR-004c permits. Nothing
/// downstream of the grant sees any of it again.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ConnectionGrantHandoff")]
pub struct ConnectionGrantHandoff {
    pub url: String,
    /// Must be shown before the user is sent to `url` (FR-036), both entries
    /// of it (FR-036e).
    pub permissions: Vec<GraphQLGrantedPermission>,
}

#[derive(InputObject, Debug, Clone)]
pub struct CompleteConnectionInput {
    pub world_id: Uuid,
    /// Whatever the host handed back when the user finished granting access,
    /// verbatim and uninterpreted.
    ///
    /// A string rather than a structured type on purpose: this module does not
    /// know what is inside it and must not learn. The adapter validates it,
    /// and refuses a grant covering more than the single repository being
    /// connected (FR-036a) — narrowing a too-broad grant after the fact is not
    /// an option, because a grant we hold and promise not to use is still a
    /// grant we hold.
    pub grant_response: String,
    /// Which repository this world binds to, as `owner/name` (FR-036f).
    ///
    /// Required in practice, and `Option` only so that a client which omits it
    /// gets told *which repositories it may choose from* rather than a schema
    /// error naming a field. An installation routinely covers several — the
    /// grant is broad, the binding is not — and picking for the user is how a
    /// world ends up mirroring into a repository nobody chose. That is not
    /// hypothetical: the first live run of this flow connected a world to this
    /// project's own public source repository, because it took whichever the
    /// host listed first.
    pub repository_ref: Option<String>,
    /// Defaults to `main`.
    pub branch: Option<String>,
    /// Defaults to `lore`. Repository-relative, no leading slash.
    pub directory: Option<String>,
}

/// What comes back across the grant boundary.
///
/// Host-neutral except for the two fields the row keeps and the API never
/// returns. `installation_ref` is a value this module stores and hands back
/// later without ever reading — FR-004c is about interpretation, not about
/// custody.
#[derive(Debug, Clone)]
pub struct GrantedConnection {
    /// `owner/name`.
    pub repository_ref: String,
    /// How access was arranged, for the adapter's own dispatch. Never leaves
    /// the row.
    pub host_kind: String,
    /// The opaque handle the adapter needs to ask for a credential later.
    pub installation_ref: String,
    /// Whether the host reported the repository as public **at the moment of
    /// the grant** (FR-040a). An observation, not a guarantee.
    pub repository_is_public: Option<bool>,
}

/// Start the grant: build the URL the user is sent to and the permission list
/// they must be shown first.
///
/// **T014 must implement this** in `src/server/src/repo_host.rs`, by calling
/// `thunderforge_repo_host::RepoHost::grant_handoff` with an anti-forgery
/// `state` value this server minted and can recognise on return, and mapping
/// its `GrantedPermission`s onto [`GraphQLGrantedPermission`]. The `world_id`
/// is passed because that `state` must bind the hand-off to one world — a
/// grant completed against a different world than it was begun for is the
/// obvious way this flow gets abused.
async fn begin_grant(
    state: &AppState,
    world_id: Uuid,
    started_by: Uuid,
) -> GraphQLResult<ConnectionGrantHandoff> {
    let registered = crate::repo_host::registration_from_env().map_err(|problems| {
        integration_unavailable_with(
            problems
                .iter()
                .map(|p| p.guidance())
                .collect::<Vec<_>>()
                .join(" "),
        )
    })?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // The state binds this hand-off to one world and one person. A grant
    // completed against a different world than it was begun for is the obvious
    // way this flow gets abused, and the state is what makes that impossible
    // rather than merely discouraged.
    let anti_forgery = crate::lore_sync::grant::begin(
        &mut conn,
        world_id,
        started_by,
        Some(&format!("/world/{world_id}/settings/system")),
    )
    .map_err(|e| Error::new(format!("Could not begin the connection: {e:?}")))?;

    let handoff = registered
        .app
        .grant_handoff(&anti_forgery)
        .map_err(|e| Error::new(format!("Could not build the hand-off: {e}")))?;

    Ok(ConnectionGrantHandoff {
        url: handoff.url,
        permissions: handoff
            .permissions
            .iter()
            .map(|p| GraphQLGrantedPermission {
                id: p.id.to_string(),
                summary: p.summary.to_string(),
                reason: p.reason.to_string(),
            })
            .collect(),
    })
}

/// Finish the grant: validate what the host returned and yield the neutral
/// facts the connection row is built from.
///
/// **T014 must implement this**, by calling
/// `thunderforge_repo_host::RepoHost::validate_grant` on `grant_response`
/// (which is where FR-036a's "no broader than one repository" refusal lives),
/// checking that the echoed anti-forgery state matches the one
/// [`begin_grant`] minted for this world, and performing the token exchange so
/// that a connection is never created against a grant that cannot produce a
/// credential. The credential itself is stored encrypted by the caching half
/// of T014 and never returns from here — nothing in this module has a field
/// to put one in (FR-035).
async fn complete_grant(
    state: &AppState,
    world_id: Uuid,
    grant_response: &str,
    requested_repository: Option<&str>,
    returning_user: Uuid,
) -> GraphQLResult<GrantedConnection> {
    // `grant_response` is `<state>:<installation reference>` — everything in
    // it is attacker-controlled, so none of it is believed before the state is.
    let (anti_forgery, installation_ref) = grant_response
        .split_once(':')
        .ok_or_else(|| Error::new("The repository host's response could not be read."))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let claim = crate::lore_sync::grant::consume(&mut conn, anti_forgery, returning_user).map_err(
        |_| {
            // One message for every refusal. Distinguishing "no such state"
            // from "expired" from "not yours" answers a question about whether
            // a state ever existed, which is what someone guessing wants.
            Error::new("This connection attempt is no longer valid. Start it again.")
        },
    )?;

    // The world the hand-off was begun for, not the one the caller now names.
    if claim.world_id != world_id {
        return Err(Error::new(
            "This connection attempt was started for a different world.",
        ));
    }

    // Only now is the installation asked about. Everything above establishes
    // that we should be asking at all.
    let repositories = crate::repo_host::visible_repositories()
        .await
        .map_err(|e| Error::new(format!("Could not read the granted access: {e}")))?;

    let granted: Vec<_> = repositories
        .into_iter()
        .filter(|r| r.installation_id == installation_ref)
        .collect();

    if granted.is_empty() {
        return Err(Error::new(
            "That installation grants access to no repository this instance can see. \
             Install the application on the repository you want to connect, then try again.",
        ));
    }

    // **The world binds to a repository the user names** (FR-036f).
    //
    // An earlier version took the first repository the installation covered,
    // and the first live run connected a world to this project's own public
    // source repository — because an installation routinely covers several and
    // "first" is whatever the host happened to list. The grant may be broad;
    // the binding may not be, and the difference is a choice a person makes
    // rather than an ordering we inherit.
    let requested = requested_repository.ok_or_else(|| {
        Error::new(format!(
            "Choose which repository this world should write to. This installation covers: {}.",
            granted
                .iter()
                .map(|r| r.full_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let chosen = granted
        .iter()
        .find(|r| r.full_name.eq_ignore_ascii_case(requested))
        .ok_or_else(|| {
            Error::new(format!(
                "The application is not installed on {requested}. It covers: {}.",
                granted
                    .iter()
                    .map(|r| r.full_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;

    Ok(GrantedConnection {
        repository_ref: chosen.full_name.clone(),
        host_kind: "github".to_string(),
        installation_ref: installation_ref.to_string(),
        repository_is_public: Some(chosen.public),
    })
}

/// The unavailable error, with a specific reason rather than the generic one.
fn integration_unavailable_with(guidance: String) -> Error {
    Error::new(guidance).extend_with(|_, ext| ext.set("code", "REPOSITORY_INTEGRATION_UNAVAILABLE"))
}

/// A repository-relative directory, or an explanation.
///
/// Validated here rather than trusted, because this string becomes a path
/// inside a clone on this server's disk before it becomes a path in someone's
/// repository. `..` is the one that matters; the rest keep the value
/// recognisable to the person who typed it.
fn normalize_directory(value: &str) -> Result<String, Error> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err(Error::new(
            "Give the directory the world's lore should live in, such as `lore`.",
        ));
    }
    if trimmed.contains('\\') {
        return Err(Error::new("Use `/` to separate directories, not `\\`."));
    }
    if trimmed
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Error::new(
            "The directory must be a plain path inside the repository, with no `.` or `..` parts.",
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_branch(value: Option<&str>) -> Result<String, Error> {
    let branch = value.map(str::trim).unwrap_or(DEFAULT_BRANCH);
    if branch.is_empty() {
        return Ok(DEFAULT_BRANCH.to_string());
    }
    // Refuses the characters git itself refuses in a ref name, plus a leading
    // `-`, which would be read as an option by the git invocations that carry
    // this value.
    let rejected = branch.starts_with('-')
        || branch
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
        || branch.contains("..");
    if rejected {
        return Err(Error::new(
            "That is not a usable branch name. Use something like `main`.",
        ));
    }
    Ok(branch.to_string())
}

/// Testable core of `beginLoreRepositoryConnection` (the repo's `_impl`
/// convention, so tests need no GraphQL `Context`).
pub async fn begin_lore_repository_connection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<ConnectionGrantHandoff> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    // FR-001, checked before the user is sent anywhere. The database enforces
    // it too, but discovering a duplicate *after* someone has installed an
    // application on a repository would mean asking them to undo something at
    // the host to recover from our own oversight.
    if load_connection(state, world_id).await?.is_some() {
        return Err(already_connected());
    }

    begin_grant(state, world_id, user_id).await
}

fn already_connected() -> Error {
    Error::new(
        "This world is already connected to a repository. Remove that connection before making another.",
    )
    .extend_with(|_, ext| ext.set("code", "ALREADY_CONNECTED"))
}

fn directory_claimed() -> Error {
    Error::new(
        "Another world is already synchronising into that directory of that repository. \
         Choose a different directory, or a different repository.",
    )
    .extend_with(|_, ext| ext.set("code", "REPOSITORY_DIRECTORY_CLAIMED"))
}

/// Testable core of `completeLoreRepositoryConnection`.
///
/// Creates nothing when any of its refusals fires. The two uniqueness rules
/// are checked here *and* enforced by the schema; the check here exists to
/// produce a sentence a Game Master can act on, and the constraint exists
/// because a check is a race and a constraint is not.
pub async fn complete_lore_repository_connection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CompleteConnectionInput,
) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
    let world_id = input.world_id;
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let branch = normalize_branch(input.branch.as_deref())?;
    let directory = normalize_directory(input.directory.as_deref().unwrap_or(DEFAULT_DIRECTORY))?;

    // FR-001.
    if load_connection(state, world_id).await?.is_some() {
        return Err(already_connected());
    }

    let granted = complete_grant(
        state,
        world_id,
        &input.grant_response,
        input.repository_ref.as_deref(),
        user_id,
    )
    .await?;

    // FR-033. Two worlds writing into one directory of one repository would
    // interleave two histories into one tree, and neither owner could tell
    // which commits were theirs.
    if repository_directory_is_claimed(state, &granted.repository_ref, &directory, world_id).await?
    {
        return Err(directory_claimed());
    }

    let now = chrono::Utc::now().naive_utc();
    let row = LoreRepositoryConnection {
        id: Uuid::now_v7(),
        world_id,
        host_kind: granted.host_kind,
        installation_ref: granted.installation_ref,
        repository_ref: granted.repository_ref,
        branch,
        directory,
        // Story 3 only, and off until it exists. A connection that accepted
        // incoming edits on the day it was created would be the first
        // delivery writing to a world's lore.
        incoming_enabled: false,
        // FR-038: null until the Game Master acknowledges the notice, and the
        // background task never picks up a null row. The gate is this column,
        // not the screen that shows the notice.
        notice_acknowledged_at: None,
        state: LoreSyncState::NeverConfigured.as_db_str().to_string(),
        state_reason: None,
        repository_is_public: granted.repository_is_public,
        visibility_checked_at: granted.repository_is_public.map(|_| now),
        deactivated_at: None,
        deactivated_reason: None,
        last_synced_at: None,
        last_written_commit: None,
        created_by: user_id,
        updated_by: user_id,
        created_at: now,
        updated_at: now,
    };

    let inserted = insert_connection(state, row).await?;
    Ok(inserted.into())
}

async fn repository_directory_is_claimed(
    state: &AppState,
    repository_ref: &str,
    directory: &str,
    excluding_world: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let repository_ref = repository_ref.to_string();
    let directory = directory.to_string();

    tokio::task::spawn_blocking(move || {
        lore_repository_connections::table
            .filter(lore_repository_connections::repository_ref.eq(repository_ref))
            .filter(lore_repository_connections::directory.eq(directory))
            .filter(lore_repository_connections::world_id.ne(excluding_world))
            .select(lore_repository_connections::id)
            .first::<Uuid>(&mut conn)
            .optional()
            .map(|found| found.is_some())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to check repository directory"))
}

/// Inserts, and translates the two uniqueness constraints back into the same
/// sentences the pre-checks produce. A caller that lost the race between the
/// check and the insert gets the explanation, not a database error.
async fn insert_connection(
    state: &AppState,
    row: LoreRepositoryConnection,
) -> GraphQLResult<LoreRepositoryConnection> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = row.world_id;

    let result = tokio::task::spawn_blocking(move || {
        diesel::insert_into(lore_repository_connections::table)
            .values(row)
            .returning(LoreRepositoryConnection::as_returning())
            .get_result::<LoreRepositoryConnection>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?;

    result.map_err(|err| match err {
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            ref info,
        ) => {
            // Which constraint fired decides which sentence is true, and the
            // constraint name is the only thing that distinguishes them.
            if info
                .constraint_name()
                .is_some_and(|name| name.contains("world"))
            {
                already_connected()
            } else {
                directory_claimed()
            }
        }
        _ => {
            let _ = world_id;
            Error::new("Failed to save the repository connection")
        }
    })
}

/// Testable core of `acknowledgeLoreSyncNotice`.
///
/// Idempotent, and deliberately does not re-stamp an existing acknowledgement:
/// the recorded time is when the Game Master was told, and a second press of a
/// button must not rewrite that.
pub async fn acknowledge_lore_sync_notice_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let existing = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;

    if existing.notice_acknowledged_at.is_some() {
        return Ok(existing.into());
    }

    let now = chrono::Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world_id)),
        )
        .set((
            lore_repository_connections::notice_acknowledged_at.eq(Some(now)),
            lore_repository_connections::updated_at.eq(now),
            lore_repository_connections::updated_by.eq(user_id),
        ))
        .returning(LoreRepositoryConnection::as_returning())
        .get_result::<LoreRepositoryConnection>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to record the acknowledgement"))?;

    Ok(updated.into())
}

fn no_connection() -> Error {
    Error::new("This world has no repository connection.")
        .extend_with(|_, ext| ext.set("code", "NOT_FOUND"))
}

/// Testable core of `removeLoreRepositoryConnection`.
///
/// Deletes the connection row and nothing else. FR-005: the world's lore is
/// untouched, and the repository's contents are untouched — a removal is the
/// platform forgetting, not a retraction. Nothing is deleted at the host, ever.
///
/// The server-side working clone is a rebuildable cache. Discarding it is
/// `lore_sync::workspace::discard(root, connection_id)`, and this function does
/// not call it because nothing yet configures `root` — T023 owns where clones
/// live, and that call belongs here the moment it does. Nothing depends on the
/// clone's removal for correctness: deleting the row is what stops
/// synchronisation, because the background task iterates rows, and a clone left
/// behind is disk, not behaviour.
///
/// Returns `false` when there was nothing to remove, so a client that asks
/// twice sees "already gone" rather than an error.
/// How a Game Master answers FR-031's divergence.
///
/// Two options, and the absence of a third is the point. Reconciling would mean
/// merging prose, which FR-024 forbids everywhere in this spec — and a
/// synchronisation that silently merged someone's repository edits with their
/// world's would be the one failure mode this feature exists to avoid.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DivergenceResolution {
    /// Make the repository match the world again, discarding the divergent
    /// history. The world is authoritative (FR-021), and this is the Game
    /// Master saying so on purpose.
    OverwriteRemote,
    /// Stop. The connection is removed and the repository is left exactly as
    /// it is, including whatever diverged.
    AbandonConnection,
}

/// FR-031's explicit choice.
///
/// A run that stopped for divergence does not resume on its own, at any
/// backoff, forever. That is deliberate: the alternative is a system that waits
/// a while and then overwrites work it warned about, which is worse than one
/// that waits indefinitely for a person.
pub async fn resolve_lore_sync_divergence_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    resolution: DivergenceResolution,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let existing = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;

    // An enforcement deactivation is not a divergence a Game Master may
    // resolve. Letting this reopen one would make FR-041a's "cannot be lifted
    // by its owner" false through a side door.
    if existing.state == "deactivated" {
        return Err(Error::new(
            "This connection was deactivated by an administrator and cannot be resumed here.",
        ));
    }

    match resolution {
        DivergenceResolution::AbandonConnection => {
            remove_lore_repository_connection_impl(state, user_id, is_admin, world_id).await
        }
        DivergenceResolution::OverwriteRemote => {
            let now = chrono::Utc::now().naive_utc();
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;

            // Clearing `last_written_commit` is what actually authorises the
            // overwrite: the next push leases against nothing rather than
            // against a commit the remote no longer has, which is the only
            // way past `--force-with-lease`. The authorisation is therefore a
            // stored fact rather than a flag the pass carries in memory, so a
            // restart cannot lose it and nothing else can invent it.
            tokio::task::spawn_blocking(move || {
                diesel::update(
                    lore_repository_connections::table
                        .filter(lore_repository_connections::world_id.eq(world_id)),
                )
                .set((
                    lore_repository_connections::last_written_commit.eq::<Option<String>>(None),
                    lore_repository_connections::state.eq("working"),
                    lore_repository_connections::state_reason.eq::<Option<String>>(None),
                    lore_repository_connections::updated_at.eq(now),
                    lore_repository_connections::updated_by.eq(user_id),
                ))
                .execute(&mut conn)
            })
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to record the resolution"))?;

            Ok(true)
        }
    }
}

/// FR-023: nothing is applied until someone with authority says so.
///
/// Owner-level, re-checked here rather than trusted from whenever the change
/// was detected — a change can sit pending for as long as nobody looks at it,
/// and the person who may accept it is the person who has authority *now*.
pub async fn accept_lore_incoming_change_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    change_id: Uuid,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let connection = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;

    // The gate, not a boolean check. `IncomingEnabled` is the only key to a
    // lore write in this feature, and its constructor is where FR-022 and
    // FR-041a are enforced — asking it here rather than re-testing the flags
    // means this path cannot drift away from the rule.
    let gate = crate::lore_sync::incoming::IncomingEnabled::for_connection(&connection)
        .ok_or_else(|| {
            Error::new("This world has not enabled accepting changes from its repository.")
        })?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        crate::lore_sync::incoming::accept(&mut conn, &gate, change_id, user_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map(|_| true)
    .map_err(|e| Error::new(e.to_string()))
}

/// FR-026: declining a proposed deletion is safe, and the next pass restores
/// the file. Declining anything else simply leaves the world alone.
pub async fn decline_lore_incoming_change_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    change_id: Uuid,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let connection = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;
    let gate = crate::lore_sync::incoming::IncomingEnabled::for_connection(&connection)
        .ok_or_else(|| {
            Error::new("This world has not enabled accepting changes from its repository.")
        })?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        crate::lore_sync::incoming::decline(&mut conn, &gate, change_id, user_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map(|_| true)
    .map_err(|e| Error::new(e.to_string()))
}

/// FR-041a: deactivate a connection against its owner's wishes.
///
/// Administrator-only, and the resulting state is the one a Game Master cannot
/// leave by fixing something (FR-041c). A commitment made to a rights holder
/// that the product cannot carry out is worse than no commitment, so this
/// exists whether or not anything has needed it yet.
///
/// It does not delete the connection. Removal would let the owner simply
/// reconnect, and would also destroy the record of why it was stopped.
pub async fn deactivate_lore_sync_impl(
    state: &AppState,
    is_admin: bool,
    world_id: Uuid,
    reason: String,
) -> GraphQLResult<bool> {
    if !is_admin {
        return Err(Error::new(
            "Only an administrator may deactivate a connection.",
        ));
    }

    let now = chrono::Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let affected = tokio::task::spawn_blocking(move || {
        diesel::update(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world_id)),
        )
        .set((
            lore_repository_connections::state.eq("deactivated"),
            lore_repository_connections::deactivated_at.eq(Some(now)),
            lore_repository_connections::deactivated_reason.eq(Some(reason)),
            lore_repository_connections::updated_at.eq(now),
        ))
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to deactivate the connection"))?;

    Ok(affected > 0)
}

pub async fn remove_lore_repository_connection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let deleted = tokio::task::spawn_blocking(move || {
        diesel::delete(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world_id)),
        )
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove the repository connection"))?;

    Ok(deleted > 0)
}

#[derive(Default)]
pub struct LoreSyncMutation;

#[Object]
impl LoreSyncMutation {
    /// Begin the grant. Returns where to send the user and what they must be
    /// shown before going (FR-036, FR-036e).
    async fn begin_lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<ConnectionGrantHandoff> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        begin_lore_repository_connection_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await
    }

    /// Finish the grant and create the connection.
    ///
    /// Creates nothing when the world already has a connection (FR-001), when
    /// the repository directory is claimed by another world (FR-033), or when
    /// the grant covers more than the one repository being connected
    /// (FR-036a).
    async fn complete_lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        input: CompleteConnectionInput,
    ) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        complete_lore_repository_connection_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    /// Record that the Game Master has read FR-037's notice.
    ///
    /// Synchronisation does not begin until this succeeds: a connection whose
    /// `noticeAcknowledgedAt` is null is never picked up (FR-038). The
    /// notice's wording is a client concern; the gate is here, because a
    /// client-side-only gate is not a gate.
    async fn acknowledge_lore_sync_notice(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        acknowledge_lore_sync_notice_impl(state, auth_user.user_id, auth_user.is_admin, world_id)
            .await
    }

    /// FR-031. Answer a divergence: overwrite the remote, or abandon the
    /// connection. There is deliberately no third option that reconciles,
    /// because reconciling would mean merging prose (FR-024).
    async fn resolve_lore_sync_divergence(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        resolution: DivergenceResolution,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        resolve_lore_sync_divergence_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            resolution,
        )
        .await
    }

    /// FR-023. Apply one pending change, as an ordinary attributed revision.
    async fn accept_lore_incoming_change(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        change_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        accept_lore_incoming_change_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            change_id,
        )
        .await
    }

    /// FR-026. Decline one. A declined deletion is restored by the next pass.
    async fn decline_lore_incoming_change(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        change_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        decline_lore_incoming_change_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            change_id,
        )
        .await
    }

    /// FR-041a. Deactivate a connection against its owner's wishes.
    ///
    /// Administrator-only. Exposed as a mutation rather than left to a database
    /// console because a commitment made to a rights holder that requires
    /// someone to hand-edit a table is a commitment that will not be kept under
    /// time pressure.
    async fn deactivate_lore_sync(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        reason: String,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        deactivate_lore_sync_impl(state, auth_user.is_admin, world_id, reason).await
    }

    /// Remove the connection. Leaves the world's lore and the repository's
    /// contents entirely intact (FR-005).
    async fn remove_lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_lore_repository_connection_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// A connection row for a world that already has one, inserted directly:
    /// these tests are about the *refusals*, and going through
    /// `complete_lore_repository_connection_impl` to create the first one
    /// would mean going through the grant boundary T014 has not built yet.
    fn insert_connection_row(
        conn: &mut PgConnection,
        world_id: Uuid,
        owner: Uuid,
        repository_ref: &str,
        directory: &str,
    ) -> Uuid {
        let now = chrono::Utc::now().naive_utc();
        let id = Uuid::now_v7();
        diesel::insert_into(lore_repository_connections::table)
            .values(LoreRepositoryConnection {
                id,
                world_id,
                host_kind: "test".to_string(),
                installation_ref: "test-installation".to_string(),
                repository_ref: repository_ref.to_string(),
                branch: "main".to_string(),
                directory: directory.to_string(),
                incoming_enabled: false,
                notice_acknowledged_at: None,
                state: "never_configured".to_string(),
                state_reason: None,
                repository_is_public: None,
                visibility_checked_at: None,
                deactivated_at: None,
                deactivated_reason: None,
                last_synced_at: None,
                last_written_commit: None,
                created_by: owner,
                updated_by: owner,
                created_at: now,
                updated_at: now,
            })
            .execute(conn)
            .expect("failed to insert test connection");
        id
    }

    fn complete_input(world_id: Uuid, directory: &str) -> CompleteConnectionInput {
        CompleteConnectionInput {
            world_id,
            grant_response: "{}".to_string(),
            // These tests are about the refusals, which all happen before a
            // repository is chosen.
            repository_ref: None,
            branch: None,
            directory: Some(directory.to_string()),
        }
    }

    /// **FR-022, at the boundary a client can actually reach.**
    ///
    /// The gate is enforced inside `incoming`, by a type. This asserts the
    /// GraphQL surface asks for it rather than reimplementing a flag check —
    /// a world that never opted in must be unable to have a change applied
    /// even by someone with every permission.
    #[tokio::test]
    async fn a_world_that_never_opted_in_cannot_have_a_change_applied() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        // incoming_enabled defaults to false — the state every connection
        // starts in, and the one FR-022 protects.
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );

        let attempt = accept_lore_incoming_change_impl(
            &state,
            owner,
            true, // even as an administrator
            world,
            Uuid::now_v7(),
        )
        .await;

        let message = attempt.expect_err("a world that never opted in accepted a change");
        assert!(
            message.message.contains("has not enabled"),
            "the refusal did not name the reason: {}",
            message.message,
        );
    }

    /// The same gate on the declining path. Declining is harmless, but a world
    /// that never opted in has nothing to decline and should not be told it
    /// does.
    #[tokio::test]
    async fn a_world_that_never_opted_in_cannot_decline_either() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );

        assert!(
            decline_lore_incoming_change_impl(&state, owner, false, world, Uuid::now_v7())
                .await
                .is_err(),
        );
    }

    /// FR-041a again, through this door. An enforcement deactivation must
    /// close every write path, not only the synchronising one.
    #[tokio::test]
    async fn a_deactivated_connection_cannot_apply_incoming_changes() {
        use crate::schema::lore_repository_connections as c;
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );

        // Opted in, and then deactivated. The flag alone would let this pass.
        diesel::update(c::table.filter(c::world_id.eq(world)))
            .set((c::incoming_enabled.eq(true), c::state.eq("deactivated")))
            .execute(&mut conn)
            .expect("deactivate");

        assert!(
            accept_lore_incoming_change_impl(&state, owner, true, world, Uuid::now_v7())
                .await
                .is_err(),
            "a deactivated connection applied a change from its repository",
        );
    }

    /// FR-041a. A deactivation the owner can undo is not a deactivation, and a
    /// commitment made to a rights holder that the product cannot carry out is
    /// worse than no commitment.
    #[tokio::test]
    async fn only_an_administrator_may_deactivate_a_connection() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );

        let by_owner =
            deactivate_lore_sync_impl(&state, false, world, "not allowed".to_string()).await;
        assert!(
            by_owner.is_err(),
            "a world owner deactivated their own connection"
        );

        let by_admin =
            deactivate_lore_sync_impl(&state, true, world, "repeat infringer".to_string()).await;
        assert!(by_admin.expect("an administrator may"));
    }

    /// FR-041c and FR-031 together. Resolving a divergence must not be a side
    /// door out of an enforcement action — otherwise "cannot be lifted by its
    /// owner" is false, and false in the one place it matters.
    #[tokio::test]
    async fn a_deactivated_connection_cannot_be_resumed_by_resolving_a_divergence() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );
        deactivate_lore_sync_impl(&state, true, world, "enforcement".to_string())
            .await
            .expect("deactivated");

        let attempt = resolve_lore_sync_divergence_impl(
            &state,
            owner,
            false,
            world,
            DivergenceResolution::OverwriteRemote,
        )
        .await;

        assert!(
            attempt.is_err(),
            "an enforcement action was lifted by its owner"
        );
    }

    /// Overwriting authorises the next push by clearing what the lease is
    /// taken against. A stored fact rather than a flag carried in memory, so a
    /// restart cannot lose it and nothing else can invent it.
    #[tokio::test]
    async fn overwriting_clears_the_lease_the_next_push_would_fail_against() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );

        diesel::update(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world)),
        )
        .set((
            lore_repository_connections::last_written_commit.eq(Some("deadbeef".to_string())),
            lore_repository_connections::state.eq("needs_attention"),
        ))
        .execute(&mut conn)
        .expect("set up a diverged connection");

        resolve_lore_sync_divergence_impl(
            &state,
            owner,
            false,
            world,
            DivergenceResolution::OverwriteRemote,
        )
        .await
        .expect("the owner may overwrite");

        let after = load_connection(&state, world)
            .await
            .expect("loaded")
            .expect("present");
        assert_eq!(after.last_written_commit, None, "the stale lease survived");
        assert_eq!(after.state, "working");
    }

    /// The other answer. Abandoning leaves the repository exactly as it is,
    /// including whatever diverged — FR-005 says removing a connection touches
    /// nothing in the repository, and a divergence is not an exception.
    #[tokio::test]
    async fn abandoning_a_divergence_removes_the_connection() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_connection_row(
            &mut conn,
            world,
            owner,
            &format!("o/{}", Uuid::now_v7()),
            "lore",
        );

        resolve_lore_sync_divergence_impl(
            &state,
            owner,
            false,
            world,
            DivergenceResolution::AbandonConnection,
        )
        .await
        .expect("the owner may abandon");

        assert!(
            load_connection(&state, world)
                .await
                .expect("loaded")
                .is_none(),
            "the connection survived being abandoned",
        );
    }

    /// FR-002. A player in the world is not an owner, and the refusal happens
    /// before anything else — in particular before the caller is told whether
    /// the world has a connection at all.
    #[tokio::test]
    async fn a_non_owner_cannot_manage_the_connection() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let player = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_test_world_member(&mut conn, world, player, "Player");
        drop(conn);

        let begun = begin_lore_repository_connection_impl(&state, player, false, world).await;
        assert!(begun.is_err(), "a player was allowed to begin a connection");

        let acknowledged = acknowledge_lore_sync_notice_impl(&state, player, false, world).await;
        assert!(
            acknowledged.is_err(),
            "a player was allowed to acknowledge the notice",
        );

        let removed = remove_lore_repository_connection_impl(&state, player, false, world).await;
        assert!(
            removed.is_err(),
            "a player was allowed to remove the connection",
        );
    }

    /// FR-001. The second connection is refused with a sentence naming the
    /// remedy, and — the part that matters — refused *before* the user is
    /// handed off to the host, so nothing has to be undone at the host to
    /// recover.
    #[tokio::test]
    async fn a_world_cannot_take_a_second_connection() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let repo = format!("owner/{}", Uuid::now_v7());
        insert_connection_row(&mut conn, world, owner, &repo, "lore");
        drop(conn);

        let begun = begin_lore_repository_connection_impl(&state, owner, false, world).await;
        let message = begun.expect_err("a second connection was begun").message;
        assert!(message.contains("already connected"), "{message}");

        let completed = complete_lore_repository_connection_impl(
            &state,
            owner,
            false,
            complete_input(world, "elsewhere"),
        )
        .await;
        let message = completed
            .expect_err("a second connection was completed")
            .message;
        assert!(message.contains("already connected"), "{message}");
    }

    /// FR-033. Two worlds writing into one directory of one repository would
    /// interleave two histories into one tree.
    ///
    /// Asserted at the database, which is where the rule actually lives: the
    /// pre-check in `complete_lore_repository_connection_impl` runs after the
    /// grant boundary T014 has not built, so testing only the pre-check today
    /// would be testing an unreachable branch.
    #[tokio::test]
    async fn two_worlds_cannot_claim_one_repository_directory() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let first_world = insert_test_world(&mut conn, owner);
        let second_world = insert_test_world(&mut conn, owner);
        let repo = format!("owner/{}", Uuid::now_v7());
        insert_connection_row(&mut conn, first_world, owner, &repo, "lore");

        let claimed = repository_directory_is_claimed(&state, &repo, "lore", second_world).await;
        assert!(
            claimed.expect("the claim check should answer"),
            "the second world was told the directory was free",
        );

        let duplicate = diesel::insert_into(lore_repository_connections::table)
            .values(LoreRepositoryConnection {
                id: Uuid::now_v7(),
                world_id: second_world,
                host_kind: "test".to_string(),
                installation_ref: "test-installation".to_string(),
                repository_ref: repo.clone(),
                branch: "main".to_string(),
                directory: "lore".to_string(),
                incoming_enabled: false,
                notice_acknowledged_at: None,
                state: "never_configured".to_string(),
                state_reason: None,
                repository_is_public: None,
                visibility_checked_at: None,
                deactivated_at: None,
                deactivated_reason: None,
                last_synced_at: None,
                last_written_commit: None,
                created_by: owner,
                updated_by: owner,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            })
            .execute(&mut conn);
        assert!(
            duplicate.is_err(),
            "two worlds claimed one repository directory",
        );
    }

    /// FR-005. Removing a connection is the platform forgetting, and forgetting
    /// must not take the world's lore with it.
    ///
    /// This is the test that would catch a future `ON DELETE CASCADE` pointed
    /// the wrong way, which is exactly the mistake that would be invisible in
    /// review.
    #[tokio::test]
    async fn removing_a_connection_leaves_the_worlds_lore_intact() {
        use crate::schema::world_lore_entries;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let repo = format!("owner/{}", Uuid::now_v7());
        insert_connection_row(&mut conn, world, owner, &repo, "lore");

        let entry_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(entry_id),
                world_lore_entries::world_id.eq(world),
                world_lore_entries::title.eq("The Salt Road"),
                world_lore_entries::slug.eq(format!("salt-road-{}", entry_id.simple())),
                world_lore_entries::content.eq("It runs east."),
                world_lore_entries::created_by.eq(owner),
                world_lore_entries::created_at.eq(now),
                world_lore_entries::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("failed to insert test lore entry");
        drop(conn);

        let removed = remove_lore_repository_connection_impl(&state, owner, false, world)
            .await
            .expect("the owner should be able to remove the connection");
        assert!(removed, "the connection was not removed");

        let mut conn = state.db_pool.get().unwrap();
        let surviving = world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .select(world_lore_entries::content)
            .first::<String>(&mut conn)
            .expect("the lore entry should have survived the removal");
        assert_eq!(surviving, "It runs east.");

        assert!(
            load_connection(&state, world)
                .await
                .expect("the connection query should answer")
                .is_none(),
            "the connection row is still present",
        );
    }

    /// A second removal is not an error. A client that asks twice — a retried
    /// request, two tabs — should see "already gone", because that is what is
    /// true.
    #[tokio::test]
    async fn removing_a_missing_connection_is_not_an_error() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        drop(conn);

        let removed = remove_lore_repository_connection_impl(&state, owner, false, world)
            .await
            .expect("removing nothing should succeed");
        assert!(!removed);
    }

    /// The directory becomes a path on this server's disk before it becomes a
    /// path in someone's repository, so `..` is refused rather than
    /// normalised away.
    #[test]
    fn a_directory_cannot_escape_the_repository() {
        assert!(normalize_directory("../../etc").is_err());
        assert!(normalize_directory("lore/../..").is_err());
        assert!(normalize_directory("   ").is_err());
        assert_eq!(normalize_directory("/lore/").unwrap(), "lore");
        assert_eq!(
            normalize_directory("campaigns/lore").unwrap(),
            "campaigns/lore"
        );
    }

    /// A branch name starting with `-` would be read as an option by the git
    /// invocations that carry it.
    #[test]
    fn a_branch_name_cannot_be_an_option() {
        assert!(normalize_branch(Some("--upload-pack=evil")).is_err());
        assert!(normalize_branch(Some("has space")).is_err());
        assert_eq!(normalize_branch(None).unwrap(), DEFAULT_BRANCH);
        assert_eq!(
            normalize_branch(Some("release/1.0")).unwrap(),
            "release/1.0"
        );
    }
}
