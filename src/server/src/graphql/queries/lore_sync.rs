//! Spec 034: reading the state of a world's repository connection.
//!
//! Three fields, and the shape of the type they return is as much of the
//! contract as the fields themselves. See
//! `specs/034-lore-git-sync/contracts/graphql-lore-sync.md`.
//!
//! # What this module cannot say, and why that is enforced by absence
//!
//! `LoreRepositoryConnection` carries **no credential at any depth**
//! (FR-035) and **no `installationRef` or `hostKind`** (FR-004c). Both exist
//! on the row; neither is convertible into the GraphQL type, and that is the
//! whole mechanism. A field that does not exist cannot be selected by an
//! over-eager client, cannot be added to a fragment by someone debugging, and
//! cannot be logged by a gateway — where a field that exists and is "not
//! meant to be used" relies on everybody downstream agreeing about that
//! forever.
//!
//! The `installationRef` case is the subtler one: a client that could read it
//! would be a client that could branch on which host arranged the grant, and
//! host-branching in a client is exactly what FR-004 forbids the moment a
//! second host exists.
//!
//! # There is no query that lists connections
//!
//! FR-039. Every field here is reached *through* a world the caller already
//! has authority over. No field takes a repository, an owner, or a page
//! number, so there is nothing to enumerate and no index to build.

use async_graphql::{
    Context, Enum, Error, ErrorExtensions, Object, Result as GraphQLResult, SimpleObject,
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_owner_of_world;
use crate::graphql::{app_state, authenticated_user, require_visible_world};
use crate::models::{LoreFidelityNote, LoreRepositoryConnection, LoreSyncRun};
use crate::schema::{lore_fidelity_notes, lore_repository_connections, lore_sync_runs};
use crate::state::AppState;

/// A connection's current state, in FR-029's own words.
///
/// `Deactivated` is the fourth (`data-model.md`'s state table; the contract
/// document lists only the first three, which predates FR-041c). It is
/// deliberately not folded into `NeedsAttention`: a Game Master told to
/// "check the connection" for an enforcement action they cannot undo will
/// keep trying to fix it, which is the failure FR-041c exists to prevent.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum LoreSyncState {
    Working,
    NeedsAttention,
    NeverConfigured,
    Deactivated,
}

impl LoreSyncState {
    /// The stored spelling. One function each way, next to each other, so the
    /// two cannot drift the way two independent string tables do.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::NeedsAttention => "needs_attention",
            Self::NeverConfigured => "never_configured",
            Self::Deactivated => "deactivated",
        }
    }

    /// A state string this build does not recognise resolves to
    /// `NeedsAttention` rather than to `Working`: an unreadable state is a
    /// reason to look at the connection, never a reason to trust it.
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "working" => Self::Working,
            "never_configured" => Self::NeverConfigured,
            "deactivated" => Self::Deactivated,
            _ => Self::NeedsAttention,
        }
    }
}

/// Something the mirror could not represent (FR-013, FR-037).
///
/// Enumerated rather than logged, because SC-008 requires a Game Master to be
/// *shown* every fidelity loss rather than to discover it by reading a clone.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "LoreFidelityNote")]
pub struct GraphQLLoreFidelityNote {
    pub id: Uuid,
    /// `null` for a note about the whole connection — permission flattening,
    /// or that the mirror is publicly visible.
    pub lore_entry_id: Option<Uuid>,
    pub kind: String,
    pub detail: String,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
}

impl From<LoreFidelityNote> for GraphQLLoreFidelityNote {
    fn from(row: LoreFidelityNote) -> Self {
        Self {
            id: row.id,
            lore_entry_id: row.lore_entry_id,
            kind: row.kind,
            detail: row.detail,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        }
    }
}

/// One attempt to bring the repository into agreement with the world.
///
/// Owner-only, wherever it is reached from: `failureReason` is written from
/// the remote's own refusal and can name a repository, a branch, or a
/// permission a player has no business knowing about.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "LoreSyncRun")]
pub struct GraphQLLoreSyncRun {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub started_at: NaiveDateTime,
    pub finished_at: Option<NaiveDateTime>,
    pub outcome: Option<String>,
    pub from_commit: Option<String>,
    pub to_commit: Option<String>,
    pub entries_written: i32,
    /// Plain language naming the remedy (FR-029), never a raw host error.
    pub failure_reason: Option<String>,
    pub attempt: i32,
}

impl From<LoreSyncRun> for GraphQLLoreSyncRun {
    fn from(row: LoreSyncRun) -> Self {
        Self {
            id: row.id,
            connection_id: row.connection_id,
            started_at: row.started_at,
            finished_at: row.finished_at,
            outcome: row.outcome,
            from_commit: row.from_commit,
            to_commit: row.to_commit,
            entries_written: row.entries_written,
            failure_reason: row.failure_reason,
            attempt: row.attempt,
        }
    }
}

/// A world's connection, as a client may see it.
///
/// Constructed only by [`From<LoreRepositoryConnection>`], which drops
/// `host_kind` and `installation_ref` on the floor. That conversion is the
/// single door between the row and the wire, so FR-004c and FR-035 are
/// satisfied in one place rather than at every call site.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex, name = "LoreRepositoryConnection")]
pub struct GraphQLLoreRepositoryConnection {
    pub id: Uuid,
    pub world_id: Uuid,
    /// `owner/name`. Host-neutral by construction — it is how every
    /// repository host this feature could reach names a repository.
    pub repository_ref: String,
    pub branch: String,
    pub directory: String,
    pub incoming_enabled: bool,
    pub state: LoreSyncState,
    /// Why the connection is in that state, in language that names the
    /// remedy (FR-029).
    pub state_reason: Option<String>,
    /// FR-038's gate. While this is null the background task never picks the
    /// connection up, so a client rendering "waiting for you" here is
    /// describing the actual behaviour rather than mirroring it.
    pub notice_acknowledged_at: Option<NaiveDateTime>,
    pub last_synced_at: Option<NaiveDateTime>,
    /// Whether the repository was **observed** to be publicly visible at
    /// `visibilityCheckedAt` (FR-040a).
    ///
    /// An observation, never a guarantee: visibility is changed at the host,
    /// which does not tell us when it happens, so a repository recorded as
    /// private may be public right now. Anywhere this is shown must show
    /// `visibilityCheckedAt` beside it and say when it was last seen — the
    /// difference between "everyone you invited" and "everyone on the
    /// internet" is the largest consequence of this feature (FR-037a), and a
    /// stale reassurance about it is worse than no answer.
    ///
    /// `null` means never checked, which must not be read as "private".
    pub repository_is_public: Option<bool>,
    pub visibility_checked_at: Option<NaiveDateTime>,
}

#[async_graphql::ComplexObject]
impl GraphQLLoreRepositoryConnection {
    /// Everything the mirror could not carry, for this connection.
    ///
    /// A field on the connection rather than a top-level query, so there is
    /// no path to a note except through a world the caller already reached
    /// (FR-039). Readable by any member for the same reason the connection is:
    /// a note says what the mirror loses, not what the repository contains.
    async fn fidelity_notes(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLLoreFidelityNote>> {
        let state = app_state(ctx)?;
        let connection_id = self.id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let rows = tokio::task::spawn_blocking(move || {
            lore_fidelity_notes::table
                .filter(lore_fidelity_notes::connection_id.eq(connection_id))
                .order(lore_fidelity_notes::first_seen_at.asc())
                .select(LoreFidelityNote::as_select())
                .load::<LoreFidelityNote>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load fidelity notes"))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

impl From<LoreRepositoryConnection> for GraphQLLoreRepositoryConnection {
    fn from(row: LoreRepositoryConnection) -> Self {
        // `host_kind` and `installation_ref` are read from the row here and
        // deliberately not carried forward. Adding either to the struct above
        // would be the change FR-004c forbids, and it would have to be made
        // here to have any effect — which is why the conversion is a single
        // function rather than field-by-field construction at call sites.
        Self {
            id: row.id,
            world_id: row.world_id,
            repository_ref: row.repository_ref,
            branch: row.branch,
            directory: row.directory,
            incoming_enabled: row.incoming_enabled,
            state: LoreSyncState::from_db_str(&row.state),
            state_reason: row.state_reason,
            notice_acknowledged_at: row.notice_acknowledged_at,
            last_synced_at: row.last_synced_at,
            repository_is_public: row.repository_is_public,
            visibility_checked_at: row.visibility_checked_at,
        }
    }
}

/// Whether this instance can offer the feature at all, and what the operator
/// must do if it cannot.
///
/// The point of this type is `configured: false` (FR-036b). An instance whose
/// operator has registered nothing is not broken, it simply does not have the
/// feature, and the world settings surface renders nothing connectable. It is
/// asked *before* the connection flow is offered rather than after it fails,
/// because a Game Master must never be shown a flow that cannot complete.
#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "RepositoryIntegrationStatus")]
pub struct RepositoryIntegrationStatus {
    pub configured: bool,
    /// What the operator must do, in their words. Never a stack trace, and
    /// never the value of anything it names.
    pub operator_guidance: Option<String>,
}

/// Whether this instance can offer repository synchronisation at all.
///
/// Public because the mutations need the same answer before they start a grant
/// a Game Master cannot finish — the check has to be one function, or the
/// query and the flow it gates will eventually disagree.
///
/// There was briefly a second copy of this logic here, a pure function taking
/// the three values as arguments. It was the more testable shape and it became
/// dead the moment this was rewired to parse the key rather than check for its
/// presence — while staying green, because its own tests were the only thing
/// calling it. Two implementations of one rule is how they drift; the rules and
/// their tests now live together in `repo_host`.
///
/// T014 (`src/server/src/repo_host.rs`) should extend this by constructing
/// the host application once at startup, so that a private key which is
/// *present but not a usable RSA key* is reported here too rather than at the
/// moment a Game Master presses connect. Presence is what this build can
/// check without naming a host.
pub fn instance_repository_integration() -> RepositoryIntegrationStatus {
    // Delegated to `repo_host::registration_from_env`, which does what a
    // presence check cannot: it **parses the key**. A value that is set but is
    // not a usable RSA private key is the case a presence check calls
    // configured, and it is the one that turns into an unreadable signing
    // error the first time a Game Master tries to connect — which is precisely
    // what FR-036c exists to prevent.
    //
    // It also accepts the four shapes a PEM arrives in from an environment —
    // a file path, base64, escaped newlines, real newlines — so an operator
    // whose deployment platform only takes single-line values is configured
    // rather than mysteriously broken.
    match crate::repo_host::registration_from_env() {
        Ok(_) => RepositoryIntegrationStatus {
            configured: true,
            operator_guidance: None,
        },
        Err(problems) => RepositoryIntegrationStatus {
            configured: false,
            // Every problem at once. An operator restarting once per missing
            // variable is a configuration experience nobody finishes.
            operator_guidance: Some(
                problems
                    .iter()
                    .map(|p| p.guidance())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        },
    }
}

/// The world's connection row, or `None`. No authority check — every caller
/// makes its own, because the two questions ("is there one" and "may you see
/// it") have different answers for the same row.
pub async fn load_connection(
    state: &AppState,
    world_id: Uuid,
) -> GraphQLResult<Option<LoreRepositoryConnection>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        lore_repository_connections::table
            .filter(lore_repository_connections::world_id.eq(world_id))
            .select(LoreRepositoryConnection::as_select())
            .first::<LoreRepositoryConnection>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load repository connection"))
}

/// Owner-level authority over one world, or a `FORBIDDEN` error naming the
/// rule rather than the row.
///
/// Shared by every field and mutation in this feature so that FR-002 is
/// re-answered per call. Nothing here caches the answer: FR-003 requires the
/// authority to synchronise to derive from authority *now*, and a captured
/// answer is a former owner's connection still running.
pub async fn require_world_owner(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<()> {
    if is_owner_of_world(state, user_id, is_admin, world_id).await? {
        Ok(())
    } else {
        Err(
            Error::new("Only a world's owner can manage its repository connection")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
        )
    }
}

#[derive(Default)]
pub struct LoreSyncQuery;

#[Object]
impl LoreSyncQuery {
    /// The world's connection, or null when it has none.
    ///
    /// Any world member, not just the owner. The fields here describe *state*,
    /// and hiding a broken connection from the players who can already read
    /// the lore it mirrors helps nobody — while `loreSyncRuns`, which can name
    /// repository details, stays owner-only.
    async fn lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLLoreRepositoryConnection>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        require_visible_world(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        Ok(load_connection(state, world_id).await?.map(Into::into))
    }

    /// Recent synchronisation attempts, newest first.
    ///
    /// Owner-level (FR-002): a run's failure reason is the one place a
    /// repository's own words reach the API, and they can name a repository,
    /// a branch or a permission that is the owner's business alone.
    ///
    /// A world with no connection answers with an empty list rather than an
    /// error — "no connection" and "a connection that has never run" look the
    /// same to someone reading a settings page, and both mean "nothing has
    /// happened yet".
    async fn lore_sync_runs(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLLoreSyncRun>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        require_world_owner(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        let Some(connection) = load_connection(state, world_id).await? else {
            return Ok(Vec::new());
        };

        // Clamped rather than trusted: a caller asking for every run of a
        // connection that has been retrying for a month would be asking for a
        // page nobody reads, at the database's expense.
        let limit = limit.unwrap_or(20).clamp(1, 100) as i64;
        let connection_id = connection.id;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let rows = tokio::task::spawn_blocking(move || {
            lore_sync_runs::table
                .filter(lore_sync_runs::connection_id.eq(connection_id))
                .order(lore_sync_runs::started_at.desc())
                .limit(limit)
                .select(LoreSyncRun::as_select())
                .load::<LoreSyncRun>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load synchronisation runs"))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Whether this instance can offer repository synchronisation at all.
    ///
    /// Asked before the connection UI is rendered, not after a connection
    /// fails (FR-036b). Instance-wide and world-independent, so it takes no
    /// argument and reveals nothing about any world — but it does require a
    /// signed-in caller, because the operator guidance names this
    /// deployment's configuration and an anonymous visitor has no reason to
    /// read it.
    async fn instance_repository_integration(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<RepositoryIntegrationStatus> {
        authenticated_user(ctx)?;
        Ok(instance_repository_integration())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The registration diagnostics moved to `crate::repo_host`, where the
    // implementation now lives. They were exercising a second copy of the
    // same rules here — a copy that stopped being called when the diagnostic
    // was rewired to actually parse the key, and stayed green because its own
    // tests were the only thing keeping it alive.

    /// An unrecognised stored state resolves towards attention, never towards
    /// "working". The alternative is a build that quietly reports a
    /// connection as healthy because it does not understand what it read.
    #[test]
    fn an_unknown_state_string_resolves_to_needing_attention() {
        assert_eq!(
            LoreSyncState::from_db_str("something-a-later-migration-added"),
            LoreSyncState::NeedsAttention
        );
        for state in [
            LoreSyncState::Working,
            LoreSyncState::NeedsAttention,
            LoreSyncState::NeverConfigured,
            LoreSyncState::Deactivated,
        ] {
            assert_eq!(LoreSyncState::from_db_str(state.as_db_str()), state);
        }
    }
}
