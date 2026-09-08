//! One attempt at turning a submission into an issue, and the closed
//! vocabulary its failures are recorded in.
//!
//! # There is no path that does work without a row
//!
//! An attempt writes a `feedback_delivery_attempts` row with `finished_at`
//! NULL, does the work, and finishes the row. The NULL is the single-flight
//! guard — [`super::schedule::due_now`] refuses a submission whose latest
//! attempt has not finished — and it is also what makes FR-021 answerable: an
//! operator asking "what has not been delivered and why" is asking about a
//! history of attempts, which a single mutable status column cannot express.
//!
//! # Why the reason is an enum and that is the point
//!
//! FR-021 says no reason may disclose a credential or any fragment of one. A
//! free-text field carrying a host's response body is one 401 payload away
//! from breaking that, and the failure would be invisible in review because
//! the string looked fine on the day it was written. A closed vocabulary
//! cannot leak, and the mapping from host error to variant is
//! [`FailureReason::from_host`] — a function with tests, rather than a
//! judgement made at each call site.
//!
//! # "Exactly once", stated honestly
//!
//! GitHub's issue endpoint accepts no idempotency key, and the request that
//! produces a duplicate is the one that **succeeded at the host and whose
//! response never arrived** — indistinguishable, from here, from one that
//! never arrived at all. Three mechanisms, in order: a `delivery_key` visible
//! at the destination; a search before create, but **only after an ambiguous
//! failure**, because a 4xx means the host declined and created nothing; and
//! single-flight by state. The residual case — the host's search index is not
//! immediate — is why the first backoff step is thirty seconds and why the key
//! is one this product controls: a duplicate, if one occurs, carries the same
//! key as its twin and is therefore findable and closable.

use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::{AttachmentKind, DeliveryState, issue_body};
use crate::repo_host::scoped::HostFailure;
use crate::schema::{feedback_delivery_attempts, feedback_submissions};
use crate::state::AppState;

/// Why a submission has not been delivered.
///
/// Never a host body, never a credential, never a fragment of one. Stored as
/// the variant's string and rendered to an operator as the variant, so there
/// is no point at which arbitrary text could enter it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    /// No destination, or credentials that do not resolve. FR-030: this is a
    /// setup step, not an incident.
    NotConfigured,
    CredentialsRejected,
    DestinationNotFound,
    PermissionRefused,
    HostUnavailable,
    HostRateLimited,
    RejectedByHost,
    /// The evidence was purged before delivery succeeded. Honest rather than
    /// tidy: a report delivered without the thing it was for is worse than one
    /// an operator can see failed and why.
    AttachmentsExpired,
}

impl FailureReason {
    pub fn as_str(self) -> &'static str {
        match self {
            FailureReason::NotConfigured => "not_configured",
            FailureReason::CredentialsRejected => "credentials_rejected",
            FailureReason::DestinationNotFound => "destination_not_found",
            FailureReason::PermissionRefused => "permission_refused",
            FailureReason::HostUnavailable => "host_unavailable",
            FailureReason::HostRateLimited => "host_rate_limited",
            FailureReason::RejectedByHost => "rejected_by_host",
            FailureReason::AttachmentsExpired => "attachments_expired",
        }
    }

    pub fn parse(value: &str) -> Option<FailureReason> {
        match value {
            "not_configured" => Some(FailureReason::NotConfigured),
            "credentials_rejected" => Some(FailureReason::CredentialsRejected),
            "destination_not_found" => Some(FailureReason::DestinationNotFound),
            "permission_refused" => Some(FailureReason::PermissionRefused),
            "host_unavailable" => Some(FailureReason::HostUnavailable),
            "host_rate_limited" => Some(FailureReason::HostRateLimited),
            "rejected_by_host" => Some(FailureReason::RejectedByHost),
            "attachments_expired" => Some(FailureReason::AttachmentsExpired),
            _ => None,
        }
    }

    /// One host failure, in terms an operator can act on.
    ///
    /// Every arm is a different fix: a rejected assertion is a key to replace,
    /// a 403 is a permission to grant, a 404 is an installation that was
    /// removed or a repository that was renamed, and a 5xx is somebody else's
    /// outage to wait out. Collapsing them would make the operator's view a
    /// list of things that all say "it did not work".
    pub fn from_host(failure: &HostFailure) -> FailureReason {
        match failure.status {
            None => FailureReason::HostUnavailable,
            Some(401) => FailureReason::CredentialsRejected,
            Some(403) => FailureReason::PermissionRefused,
            Some(404) => FailureReason::DestinationNotFound,
            Some(429) => FailureReason::HostRateLimited,
            Some(code) if code >= 500 => FailureReason::HostUnavailable,
            Some(_) => FailureReason::RejectedByHost,
        }
    }
}

/// What one attempt did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Created,
    /// A retry found an issue an earlier ambiguous attempt had already made.
    /// Its own value rather than folded into `Created`, because it is the
    /// evidence that FR-019 held under the one condition that could break it.
    Adopted,
    Failed,
    NotConfigured,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Created => "created",
            Outcome::Adopted => "adopted",
            Outcome::Failed => "failed",
            Outcome::NotConfigured => "not_configured",
        }
    }
}

/// The latest attempt on one submission: how many there have been, whether one
/// is in flight, and whether the last one left the host's state ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LatestAttempt {
    pub attempt: i32,
    pub in_flight: bool,
    /// True when the previous attempt may have created an issue whose response
    /// was lost — a transport failure, a 5xx, or no status at all. **This is
    /// what decides whether the next attempt searches before it creates**, and
    /// it is deliberately false for a 4xx.
    pub ambiguous: bool,
    pub started_at: Option<NaiveDateTime>,
}

pub fn latest_attempt(
    conn: &mut PgConnection,
    submission_id: Uuid,
) -> Result<LatestAttempt, String> {
    #[allow(clippy::type_complexity)]
    let row: Option<(
        i32,
        Option<NaiveDateTime>,
        Option<String>,
        Option<i32>,
        NaiveDateTime,
    )> = feedback_delivery_attempts::table
        .filter(feedback_delivery_attempts::submission_id.eq(submission_id))
        .order(feedback_delivery_attempts::started_at.desc())
        .select((
            feedback_delivery_attempts::attempt,
            feedback_delivery_attempts::finished_at,
            feedback_delivery_attempts::outcome,
            feedback_delivery_attempts::http_status,
            feedback_delivery_attempts::started_at,
        ))
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to read the delivery attempts: {e}"))?;

    Ok(match row {
        None => LatestAttempt::default(),
        Some((attempt, finished_at, outcome, http_status, started_at)) => LatestAttempt {
            attempt,
            in_flight: finished_at.is_none(),
            ambiguous: is_ambiguous(outcome.as_deref(), http_status),
            started_at: Some(started_at),
        },
    })
}

/// Whether the previous attempt left the host's state unknown.
///
/// Only a **failed** attempt can be ambiguous, and only one whose answer was
/// lost: a transport failure or a 5xx. `not_configured` finishes with no status
/// and never reached the host at all, so reading "no status" as ambiguous would
/// make every unconfigured instance search for an issue nobody tried to create
/// — a request, a rate-limit unit, and a confusing line in a host's audit log,
/// all for nothing.
fn is_ambiguous(outcome: Option<&str>, http_status: Option<i32>) -> bool {
    if outcome != Some(Outcome::Failed.as_str()) {
        return false;
    }
    match http_status {
        None => true,
        Some(code) => code >= 500,
    }
}

/// Open an attempt row. Returns its id; the row is unfinished until
/// [`finish`] is called, and while it is unfinished the submission is not due.
pub fn begin(
    conn: &mut PgConnection,
    submission_id: Uuid,
    attempt: i32,
    now: NaiveDateTime,
) -> Result<Uuid, String> {
    let id = Uuid::now_v7();
    diesel::insert_into(feedback_delivery_attempts::table)
        .values((
            feedback_delivery_attempts::id.eq(id),
            feedback_delivery_attempts::submission_id.eq(submission_id),
            feedback_delivery_attempts::attempt.eq(attempt),
            feedback_delivery_attempts::started_at.eq(now),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to open a delivery attempt: {e}"))?;
    Ok(id)
}

pub fn finish(
    conn: &mut PgConnection,
    attempt_id: Uuid,
    outcome: Outcome,
    reason: Option<FailureReason>,
    http_status: Option<i32>,
    now: NaiveDateTime,
) -> Result<(), String> {
    diesel::update(
        feedback_delivery_attempts::table.filter(feedback_delivery_attempts::id.eq(attempt_id)),
    )
    .set((
        feedback_delivery_attempts::finished_at.eq(now),
        feedback_delivery_attempts::outcome.eq(outcome.as_str()),
        feedback_delivery_attempts::failure_reason.eq(reason.map(FailureReason::as_str)),
        feedback_delivery_attempts::http_status.eq(http_status),
    ))
    .execute(conn)
    .map_err(|e| format!("Failed to finish a delivery attempt: {e}"))?;
    Ok(())
}

/// Try one submission, whatever the outcome, and record what happened.
///
/// Public because an operator's "try it again now" would otherwise have to
/// wait for a tick, and because the code under test should be the code in
/// production — the same argument `mail::schedule::attempt` makes for itself.
pub async fn attempt(state: &AppState, submission_id: Uuid) -> Result<Outcome, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let submission = super::load(&mut conn, submission_id)?
        .ok_or_else(|| "That submission is no longer recorded.".to_string())?;
    let latest = latest_attempt(&mut conn, submission_id)?;
    if latest.in_flight {
        // Somebody else has it. Not an error: the honest answer is that
        // nothing happened here.
        return Ok(Outcome::Failed);
    }

    let now = Utc::now().naive_utc();
    let attempt_id = begin(&mut conn, submission_id, latest.attempt + 1, now)?;

    // The evidence is gone, so a report about it cannot be made. Recorded
    // rather than retried forever: the operator's view is where this is meant
    // to be seen, and it has been visible since the first failure.
    if submission.attachments_purged_at.is_some() {
        finish(
            &mut conn,
            attempt_id,
            Outcome::Failed,
            Some(FailureReason::AttachmentsExpired),
            None,
            Utc::now().naive_utc(),
        )?;
        return Ok(Outcome::Failed);
    }

    let destination = super::destination(&mut conn)?;
    let settings = crate::settings::resolver::resolve_all(state).await?;
    let host = match state.feedback.host(&settings, destination.as_ref()) {
        Ok(host) => host,
        Err(_missing) => {
            // FR-030: the submission stays pending, forever if need be. The
            // reason names a vocabulary entry; the *keys* an operator has to
            // set are rendered from the declarations by the operator surface,
            // which is where they belong and where they cannot be a value.
            finish(
                &mut conn,
                attempt_id,
                Outcome::NotConfigured,
                Some(FailureReason::NotConfigured),
                None,
                Utc::now().naive_utc(),
            )?;
            return Ok(Outcome::NotConfigured);
        }
    };
    let is_public = destination.as_ref().and_then(|d| d.is_public);

    // Search before create, but only after an ambiguous failure. A 4xx never
    // triggers this, because a 4xx means the host declined and created nothing
    // — and a search that finds nothing costs a request and a rate-limit unit
    // for no information.
    if latest.ambiguous {
        match host.find_by_key(&submission.delivery_key.to_string()).await {
            Ok(Some(issue)) => {
                let finished = Utc::now().naive_utc();
                super::record_delivered(&mut conn, submission_id, &issue, finished)?;
                finish(
                    &mut conn,
                    attempt_id,
                    Outcome::Adopted,
                    None,
                    None,
                    finished,
                )?;
                return Ok(Outcome::Adopted);
            }
            Ok(None) => {}
            Err(failure) => {
                return fail(&mut conn, attempt_id, &failure).map(|_| Outcome::Failed);
            }
        }
    }

    let attachments = super::attachments_of(&mut conn, submission_id)?;
    let world_name = world_name(&mut conn, submission.world_id)?;
    drop(conn);

    let mut files = Vec::new();
    let mut log_text: Option<String> = None;
    for row in &attachments {
        let Some(kind) = row.kind() else { continue };
        let bytes = match read_object(&row.storage_path).await {
            Ok(bytes) => bytes,
            // The bytes are gone from this instance's storage but the row says
            // otherwise. Retrying will not conjure them, and delivering a
            // report whose evidence is missing is the failure this reports as
            // itself rather than papering over.
            Err(_) => {
                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| "Failed to get DB connection".to_string())?;
                finish(
                    &mut conn,
                    attempt_id,
                    Outcome::Failed,
                    Some(FailureReason::AttachmentsExpired),
                    None,
                    Utc::now().naive_utc(),
                )?;
                return Ok(Outcome::Failed);
            }
        };
        if kind == AttachmentKind::Logs {
            log_text = Some(String::from_utf8_lossy(&bytes).to_string());
        }
        let path = format!("attachments/{submission_id}/{}", kind.file_name());
        match host.attach(&path, &bytes).await {
            Ok(attached) => files.push(issue_body::AttachedFile {
                kind,
                blob_url: attached.blob_url,
                // Embedded only for a repository this instance has *observed*
                // to be public. An unknown visibility links rather than
                // embeds, because a broken image for every maintainer is worse
                // than a link for all of them.
                embed_url: match (kind, is_public) {
                    (AttachmentKind::Screenshot, Some(true)) => Some(attached.raw_url),
                    _ => None,
                },
                entries_kept: row.entries_kept,
                entries_dropped: row.entries_dropped,
                redaction_count: row.redaction_count,
            }),
            Err(failure) => {
                let mut conn = state
                    .db_pool
                    .get()
                    .map_err(|_| "Failed to get DB connection".to_string())?;
                return fail(&mut conn, attempt_id, &failure).map(|_| Outcome::Failed);
            }
        }
    }

    let kind = submission.kind();
    let context = issue_body::Context {
        screen_path: submission.screen_path.clone(),
        world_name,
        game_system_id: submission.game_system_id.clone(),
        client_version: submission.client_version.clone(),
        server_version: submission.server_version.clone(),
        browser: submission.browser.clone(),
        submitted_at: format!(
            "{}Z",
            submission.created_at.and_utc().format("%Y-%m-%dT%H:%M:%S")
        ),
    };
    let title = issue_body::title(kind, submission.summary.as_deref(), &submission.message);
    let body = issue_body::body(
        &submission.message,
        &context,
        submission.id,
        submission.delivery_key,
        &files,
        log_text.as_deref(),
    );
    let labels = issue_body::labels(kind);

    let created = host.create(&title, &body, &labels).await;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    match created {
        Ok(issue) => {
            let finished = Utc::now().naive_utc();
            super::record_delivered(&mut conn, submission_id, &issue, finished)?;
            finish(
                &mut conn,
                attempt_id,
                Outcome::Created,
                None,
                Some(201),
                finished,
            )?;
            Ok(Outcome::Created)
        }
        Err(failure) => fail(&mut conn, attempt_id, &failure).map(|_| Outcome::Failed),
    }
}

/// Record a failed attempt. The submission stays `pending`: an operator
/// abandons it, or the backoff eventually retries it, and neither of those is
/// this function's decision.
fn fail(conn: &mut PgConnection, attempt_id: Uuid, failure: &HostFailure) -> Result<(), String> {
    finish(
        conn,
        attempt_id,
        Outcome::Failed,
        Some(FailureReason::from_host(failure)),
        failure.status.map(i32::from),
        Utc::now().naive_utc(),
    )
}

async fn read_object(key: &str) -> Result<Vec<u8>, String> {
    let cfg = crate::storage::rustfs::RustFsConfig::from_env();
    crate::storage::rustfs::read_object(&cfg, key)
        .await
        .map_err(|e| format!("Failed to read an attachment: {e}"))
}

fn world_name(conn: &mut PgConnection, world_id: Option<Uuid>) -> Result<Option<String>, String> {
    use crate::schema::worlds;
    let Some(world_id) = world_id else {
        return Ok(None);
    };
    worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::name)
        .first::<String>(conn)
        .optional()
        .map_err(|e| format!("Failed to read the world's name: {e}"))
}

/// Everything still waiting, oldest first — the operator's list (FR-021).
///
/// Pending *and* abandoned, because an operator asking "what has not arrived"
/// means both, and an abandoned item that never appears again is one nobody
/// can decide to resume.
pub fn undelivered(conn: &mut PgConnection) -> Result<Vec<super::SubmissionRow>, String> {
    feedback_submissions::table
        .filter(
            feedback_submissions::delivery_state
                .eq(DeliveryState::Pending.as_str())
                .or(feedback_submissions::delivery_state.eq(DeliveryState::Abandoned.as_str())),
        )
        .order(feedback_submissions::created_at.asc())
        .select(super::SubmissionRow::as_select())
        .load(conn)
        .map_err(|e| format!("Failed to list undelivered feedback: {e}"))
}

#[cfg(test)]
#[path = "deliver_tests.rs"]
mod tests;
