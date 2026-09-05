//! The grant boundary: the two calls this module makes into the host adapter.
//!
//! Split out of `mutations_lore_sync.rs` because it is the seam FR-004a asks
//! to be pointed at, and the parent module's own documentation already named
//! it as a boundary. Everything here is about *obtaining* a connection —
//! consent, hand-off, and validating what the user typed. Nothing here writes
//! a row; that is the parent's half.

use async_graphql::{Error, ErrorExtensions, InputObject, Result as GraphQLResult, SimpleObject};
use uuid::Uuid;

use super::DEFAULT_BRANCH;
use crate::state::AppState;
use thunderforge_repo_host::RepoHost as _;

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
pub(crate) async fn begin_grant(
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
pub(crate) async fn complete_grant(
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
pub(crate) fn integration_unavailable_with(guidance: String) -> Error {
    Error::new(guidance).extend_with(|_, ext| ext.set("code", "REPOSITORY_INTEGRATION_UNAVAILABLE"))
}

/// A repository-relative directory, or an explanation.
///
/// Validated here rather than trusted, because this string becomes a path
/// inside a clone on this server's disk before it becomes a path in someone's
/// repository. `..` is the one that matters; the rest keep the value
/// recognisable to the person who typed it.
pub(crate) fn normalize_directory(value: &str) -> Result<String, Error> {
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

pub(crate) fn normalize_branch(value: Option<&str>) -> Result<String, Error> {
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
