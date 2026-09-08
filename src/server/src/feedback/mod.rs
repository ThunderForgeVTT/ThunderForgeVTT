//! Spec 037: what a person sent from inside the product, and what became of it.
//!
//! # The one ordering the whole feature rests on
//!
//! **The submission is recorded before any delivery is attempted** (FR-018).
//! That single sentence decides the architecture: the durable object is the
//! submission, and the tracker is somewhere a copy of it is later sent. The
//! alternative — call GitHub in the resolver and keep the issue URL — makes
//! GitHub the store, and FR-030 ("with no destination configured at all,
//! submissions MUST still be kept") then has nowhere to keep anything.
//!
//! So [`record`] writes rows and returns, and it never calls a host. Delivery
//! is [`schedule`]'s pass.
//!
//! # The shape of it, and where it came from
//!
//! This is `crate::mail` again, deliberately, because mail is the same problem
//! solved yesterday: a durable record, a state machine, a background sender
//! copied from `lore_sync/schedule.rs`, and a seam so a test can assert what
//! *would* have been sent without sending it.
//!
//! - [`mod@self`] — the record: kinds, states, retention, and the write.
//! - [`redaction`] — the validator that **refuses** a payload containing a
//!   secret and never rewrites one.
//! - [`issue_body`] — what arrives at the destination: title, labels, body.
//! - [`host`] — the seam. `IssueHost`, its GitHub implementation, and the
//!   capturing one tests hold.
//! - [`deliver`] — one attempt, and the fixed vocabulary a failure is recorded
//!   in.
//! - [`schedule`] — the pass: what is due, the backoff, the state refresh and
//!   the retention sweep, on one tick.
//! - [`graphql`] — the submitter's surface and the operator's.
//!
//! # What this module deliberately does not do
//!
//! It does not redact. FR-012 is kept by the client, at capture time, before
//! anything enters the log buffer — so that the bytes the person reviewed are
//! the bytes submitted, because they are the same array. The server's half is
//! to **refuse** an approved payload that still contains a secret, which is
//! [`redaction`]. Editing after approval would mean the person saw something
//! other than what was sent, which is the exact failure FR-012 exists to
//! prevent, even when the edit is an improvement.

use std::sync::Arc;

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{feedback_attachments, feedback_destination, feedback_submissions};
use crate::state::AppState;

pub mod deliver;
pub mod graphql;
pub mod host;
pub mod issue_body;
pub mod rate_limit;
pub mod redaction;
pub mod schedule;

/// How long this instance keeps an attachment's bytes.
///
/// Written onto each submission as `attachments_expire_at` at submission time
/// rather than computed at read time, so that changing this constant later
/// cannot retroactively shorten the retention somebody was promised (FR-016).
pub const RETENTION_DAYS: i64 = 30;

/// The key prefix every feedback object lives under. Computed server-side,
/// never client-supplied, on the rule `storage::rustfs::object_key` states —
/// and the prefix `storage::rustfs::delete_object` refuses to delete outside.
pub const STORAGE_PREFIX: &str = "feedback/";

/// The three kinds, and there are exactly three (FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    FeatureRequest,
    Issue,
    General,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::FeatureRequest => "feature_request",
            Kind::Issue => "issue",
            Kind::General => "general",
        }
    }

    pub fn parse(value: &str) -> Option<Kind> {
        match value {
            "feature_request" => Some(Kind::FeatureRequest),
            "issue" => Some(Kind::Issue),
            "general" => Some(Kind::General),
            _ => None,
        }
    }
}

/// Where a submission has got to. The person is never shown a failure: an
/// undelivered submission reads as `Pending`, because at the instance — which
/// FR-018 makes the record — it did arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryState {
    Pending,
    Delivered,
    Abandoned,
}

impl DeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryState::Pending => "pending",
            DeliveryState::Delivered => "delivered",
            DeliveryState::Abandoned => "abandoned",
        }
    }

    pub fn parse(value: &str) -> Option<DeliveryState> {
        match value {
            "pending" => Some(DeliveryState::Pending),
            "delivered" => Some(DeliveryState::Delivered),
            "abandoned" => Some(DeliveryState::Abandoned),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Logs,
    Screenshot,
}

impl AttachmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AttachmentKind::Logs => "logs",
            AttachmentKind::Screenshot => "screenshot",
        }
    }

    pub fn parse(value: &str) -> Option<AttachmentKind> {
        match value {
            "logs" => Some(AttachmentKind::Logs),
            "screenshot" => Some(AttachmentKind::Screenshot),
            _ => None,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            // The storage layer's first non-image object. Every existing write
            // passes "image/webp", which is worth saying out loud because it
            // is the line a future reader trips over.
            AttachmentKind::Logs => "text/plain",
            AttachmentKind::Screenshot => "image/webp",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            AttachmentKind::Logs => "log",
            AttachmentKind::Screenshot => "webp",
        }
    }

    /// The name this attachment is committed under at the destination.
    pub fn file_name(self) -> &'static str {
        match self {
            AttachmentKind::Logs => "logs.log",
            AttachmentKind::Screenshot => "screenshot.webp",
        }
    }
}

/// One attachment, exactly as the person approved it.
///
/// The bytes are carried through unaltered from the review step to the row.
/// Nothing between them transforms the content — that is not a stylistic
/// preference, it is `contracts/attachments.md`'s whole claim.
#[derive(Debug, Clone)]
pub struct NewAttachment {
    pub kind: AttachmentKind,
    pub content: Vec<u8>,
    pub entries_kept: Option<i32>,
    pub entries_dropped: Option<i32>,
    pub redaction_count: i32,
}

/// What a person sent. Assembled by the resolver from an authenticated
/// session and the input it validated.
#[derive(Debug, Clone)]
pub struct NewSubmission {
    pub user_id: Uuid,
    pub kind: Kind,
    pub message: String,
    pub summary: Option<String>,
    pub screen_path: Option<String>,
    pub world_id: Option<Uuid>,
    pub client_version: String,
    pub browser: String,
    pub attachments: Vec<NewAttachment>,
}

/// One submission, as the server reads it back.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = feedback_submissions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SubmissionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub message: String,
    pub summary: Option<String>,
    pub screen_path: Option<String>,
    pub world_id: Option<Uuid>,
    pub game_system_id: Option<String>,
    pub client_version: String,
    pub server_version: String,
    pub browser: String,
    pub delivery_state: String,
    pub delivery_key: Uuid,
    pub issue_url: Option<String>,
    pub issue_number: Option<i32>,
    pub issue_state: Option<String>,
    pub issue_state_checked_at: Option<NaiveDateTime>,
    pub attachments_expire_at: NaiveDateTime,
    pub attachments_purged_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

impl SubmissionRow {
    pub fn kind(&self) -> Kind {
        Kind::parse(&self.kind).unwrap_or(Kind::General)
    }

    pub fn delivery_state(&self) -> DeliveryState {
        DeliveryState::parse(&self.delivery_state).unwrap_or(DeliveryState::Pending)
    }
}

/// One attachment row. The row outlives its bytes: `purged_at` is set by the
/// sweep and the row stays, so a submission can still say what was attached.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = feedback_attachments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AttachmentRow {
    pub id: Uuid,
    pub submission_id: Uuid,
    pub kind: String,
    pub storage_path: String,
    pub content_type: String,
    pub byte_size: i64,
    pub entries_kept: Option<i32>,
    pub entries_dropped: Option<i32>,
    pub redaction_count: i32,
    pub purged_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

impl AttachmentRow {
    pub fn kind(&self) -> Option<AttachmentKind> {
        AttachmentKind::parse(&self.kind)
    }
}

/// Where issues go. At most one row, the instance's own.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = feedback_destination)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DestinationRow {
    pub id: Uuid,
    pub installation_ref: String,
    pub repository_ref: String,
    pub attachment_branch: String,
    pub is_public: Option<bool>,
    pub visibility_checked_at: Option<NaiveDateTime>,
}

impl DestinationRow {
    /// `owner/name`, split. A destination whose repository reference is not
    /// two parts is unusable, and saying so here means every call site does
    /// not have to.
    pub fn owner_and_name(&self) -> Option<(&str, &str)> {
        self.repository_ref.split_once('/')
    }
}

/// The one destination, if this instance has one.
pub fn destination(conn: &mut PgConnection) -> Result<Option<DestinationRow>, String> {
    feedback_destination::table
        .select(DestinationRow::as_select())
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to read the feedback destination: {e}"))
}

/// Record what the destination said about its own visibility, and when.
///
/// The observation time is stored with the answer because visibility changes
/// at the host without telling us — the sentence
/// `lore_repository_connections` already carries, and the reason FR-014's
/// notice renders both together or neither.
pub fn record_visibility(
    conn: &mut PgConnection,
    id: Uuid,
    is_public: bool,
    now: NaiveDateTime,
) -> Result<(), String> {
    diesel::update(feedback_destination::table.filter(feedback_destination::id.eq(id)))
        .set((
            feedback_destination::is_public.eq(is_public),
            feedback_destination::visibility_checked_at.eq(now),
            feedback_destination::updated_at.eq(now),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to record the destination's visibility: {e}"))?;
    Ok(())
}

/// This build, as the submission records it.
///
/// `option_env!` rather than `env!` for the SHA: it is absent in a plain
/// `cargo build` and present in CI, and "unknown build" is a better answer
/// than a field that is sometimes missing (research § R12).
pub fn server_version() -> String {
    match option_env!("THUNDERFORGE_GIT_SHA") {
        Some(sha) if !sha.is_empty() => {
            format!(
                "{} ({})",
                env!("CARGO_PKG_VERSION"),
                &sha[..sha.len().min(7)]
            )
        }
        _ => format!("{} (unknown build)", env!("CARGO_PKG_VERSION")),
    }
}

/// The storage key for one attachment. Server-side, never client-supplied.
///
/// No world id and no scene id: a submission from the login page has neither.
pub fn object_key(submission_id: Uuid, attachment_id: Uuid, kind: AttachmentKind) -> String {
    format!(
        "{STORAGE_PREFIX}{submission_id}/{attachment_id}.{}",
        kind.extension()
    )
}

/// Write a submission down. **The only way anything becomes feedback.**
///
/// Returns the row, which the resolver hands straight back to the person. It
/// consults no host, so it cannot fail because GitHub is down, unreachable or
/// unconfigured — FR-018 is this function's ordering and nothing else.
///
/// # Objects first, rows second, and why round that way
///
/// The bytes are written to storage before the rows that name them, so there
/// is never a row pointing at an object that does not exist. The reverse
/// failure — an object nothing points at — costs a few kilobytes that the
/// retention sweep's prefix is bounded by anyway, and it cannot mislead
/// anybody. This ordering is about the instance's own storage; it has no
/// bearing on FR-018, which is about the destination.
pub async fn record(state: &AppState, new: NewSubmission) -> Result<SubmissionRow, String> {
    let id = Uuid::now_v7();
    let now = Utc::now().naive_utc();
    let expires_at = now + Duration::days(RETENTION_DAYS);

    // The game system is resolved here, from the world id, and is never
    // accepted from the client (research § R13): a client that could name the
    // system could name a different world's. A world the caller is not a
    // member of drops out entirely rather than refusing the submission — a
    // report about a world you have left is still a report, and refusing it
    // would lose feedback to protect nothing.
    let (world_id, game_system_id) = match new.world_id {
        None => (None, None),
        Some(world_id) => {
            let mut conn = connection(state)?;
            world_context(&mut conn, world_id, new.user_id)?
        }
    };

    let mut prepared = Vec::new();
    for attachment in &new.attachments {
        let attachment_id = Uuid::now_v7();
        let key = object_key(id, attachment_id, attachment.kind);
        let bytes = attachment.content.clone();
        let cfg = crate::storage::rustfs::RustFsConfig::from_env();
        crate::storage::rustfs::write_object(
            &cfg,
            &key,
            bytes.clone(),
            attachment.kind.content_type(),
        )
        .await
        .map_err(|e| format!("Failed to store an attachment: {e}"))?;
        prepared.push((attachment_id, key, bytes.len() as i64, attachment));
    }

    let mut conn = connection(state)?;
    let row = conn
        .transaction::<SubmissionRow, diesel::result::Error, _>(|conn| {
            let row: SubmissionRow = diesel::insert_into(feedback_submissions::table)
                .values((
                    feedback_submissions::id.eq(id),
                    feedback_submissions::user_id.eq(new.user_id),
                    feedback_submissions::kind.eq(new.kind.as_str()),
                    feedback_submissions::message.eq(&new.message),
                    feedback_submissions::summary.eq(&new.summary),
                    feedback_submissions::screen_path.eq(&new.screen_path),
                    feedback_submissions::world_id.eq(world_id),
                    feedback_submissions::game_system_id.eq(&game_system_id),
                    feedback_submissions::client_version.eq(&new.client_version),
                    feedback_submissions::server_version.eq(server_version()),
                    feedback_submissions::browser.eq(&new.browser),
                    feedback_submissions::delivery_state.eq(DeliveryState::Pending.as_str()),
                    feedback_submissions::delivery_key.eq(Uuid::now_v7()),
                    feedback_submissions::attachments_expire_at.eq(expires_at),
                    feedback_submissions::created_by.eq(new.user_id),
                    feedback_submissions::updated_by.eq(new.user_id),
                    feedback_submissions::created_at.eq(now),
                    feedback_submissions::updated_at.eq(now),
                ))
                .returning(SubmissionRow::as_select())
                .get_result(conn)?;

            for (attachment_id, key, byte_size, attachment) in &prepared {
                diesel::insert_into(feedback_attachments::table)
                    .values((
                        feedback_attachments::id.eq(*attachment_id),
                        feedback_attachments::submission_id.eq(id),
                        feedback_attachments::kind.eq(attachment.kind.as_str()),
                        feedback_attachments::storage_path.eq(key),
                        feedback_attachments::content_type.eq(attachment.kind.content_type()),
                        feedback_attachments::byte_size.eq(*byte_size),
                        feedback_attachments::entries_kept.eq(attachment.entries_kept),
                        feedback_attachments::entries_dropped.eq(attachment.entries_dropped),
                        feedback_attachments::redaction_count.eq(attachment.redaction_count),
                        feedback_attachments::created_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            Ok(row)
        })
        .map_err(|e| format!("Failed to record the submission: {e}"))?;

    Ok(row)
}

/// The world and system a submission carries, or neither.
///
/// Membership is checked here rather than by refusing in the resolver, because
/// the contract's answer to "a world the caller is not a member of" is to drop
/// the field and accept the submission.
fn world_context(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
) -> Result<(Option<Uuid>, Option<String>), String> {
    use crate::schema::{world_members, worlds};

    let world: Option<(Uuid, Option<String>)> = worlds::table
        .filter(worlds::id.eq(world_id))
        .select((worlds::id, worlds::game_system_id))
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to read the world: {e}"))?;

    let Some((world_id, game_system_id)) = world else {
        return Ok((None, None));
    };

    let member: i64 = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .count()
        .get_result(conn)
        .map_err(|e| format!("Failed to check world membership: {e}"))?;
    let owner: i64 = worlds::table
        .filter(worlds::id.eq(world_id))
        .filter(worlds::created_by.eq(user_id))
        .count()
        .get_result(conn)
        .map_err(|e| format!("Failed to check world ownership: {e}"))?;

    if member == 0 && owner == 0 {
        return Ok((None, None));
    }
    Ok((Some(world_id), game_system_id))
}

/// Everything one account has submitted, newest first.
///
/// **Filtered on the caller's `user_id`, and there is no argument by which
/// another account's rows could be reached** (FR-022). Adding one would be a
/// different feature with a different review, and the absence of a parameter
/// is what makes that checkable.
pub fn mine(conn: &mut PgConnection, user_id: Uuid) -> Result<Vec<SubmissionRow>, String> {
    feedback_submissions::table
        .filter(feedback_submissions::user_id.eq(user_id))
        .order(feedback_submissions::created_at.desc())
        .select(SubmissionRow::as_select())
        .load(conn)
        .map_err(|e| format!("Failed to list your submissions: {e}"))
}

pub fn load(conn: &mut PgConnection, id: Uuid) -> Result<Option<SubmissionRow>, String> {
    feedback_submissions::table
        .filter(feedback_submissions::id.eq(id))
        .select(SubmissionRow::as_select())
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to load the submission: {e}"))
}

/// One submission's attachment rows, oldest first.
pub fn attachments_of(
    conn: &mut PgConnection,
    submission_id: Uuid,
) -> Result<Vec<AttachmentRow>, String> {
    feedback_attachments::table
        .filter(feedback_attachments::submission_id.eq(submission_id))
        .order(feedback_attachments::created_at.asc())
        .select(AttachmentRow::as_select())
        .load(conn)
        .map_err(|e| format!("Failed to load the attachments: {e}"))
}

/// Stop retrying an item that will never succeed (FR-019's "or is abandoned by
/// an operator"). Reversible, because an irreversible button is one nobody
/// presses.
pub fn abandon(conn: &mut PgConnection, id: Uuid, actor: Uuid) -> Result<bool, String> {
    set_state(
        conn,
        id,
        DeliveryState::Pending,
        DeliveryState::Abandoned,
        actor,
    )
}

/// Return an abandoned item to the queue, after fixing what was wrong. An
/// operator who has just corrected a credential should not have to ask the
/// submitter to send it again.
pub fn resume(conn: &mut PgConnection, id: Uuid, actor: Uuid) -> Result<bool, String> {
    set_state(
        conn,
        id,
        DeliveryState::Abandoned,
        DeliveryState::Pending,
        actor,
    )
}

fn set_state(
    conn: &mut PgConnection,
    id: Uuid,
    from: DeliveryState,
    to: DeliveryState,
    actor: Uuid,
) -> Result<bool, String> {
    let changed = diesel::update(
        feedback_submissions::table
            .filter(feedback_submissions::id.eq(id))
            .filter(feedback_submissions::delivery_state.eq(from.as_str())),
    )
    .set((
        feedback_submissions::delivery_state.eq(to.as_str()),
        feedback_submissions::updated_by.eq(actor),
        feedback_submissions::updated_at.eq(Utc::now().naive_utc()),
    ))
    .execute(conn)
    .map_err(|e| format!("Failed to change the delivery state: {e}"))?;
    Ok(changed == 1)
}

/// Record where an issue landed. Written in one statement with the state it
/// guards, so "delivered" and "we know where it is" cannot disagree.
pub fn record_delivered(
    conn: &mut PgConnection,
    id: Uuid,
    issue: &crate::repo_host::scoped::PostedIssue,
    now: NaiveDateTime,
) -> Result<(), String> {
    diesel::update(feedback_submissions::table.filter(feedback_submissions::id.eq(id)))
        .set((
            feedback_submissions::delivery_state.eq(DeliveryState::Delivered.as_str()),
            feedback_submissions::issue_url.eq(&issue.html_url),
            feedback_submissions::issue_number.eq(issue.number),
            feedback_submissions::issue_state.eq("open"),
            feedback_submissions::issue_state_checked_at.eq(now),
            feedback_submissions::updated_at.eq(now),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to record the delivery: {e}"))?;
    Ok(())
}

/// What a maintainer has since done with an issue: `open` or `closed`, and
/// nothing else (research § R14).
pub fn record_issue_state(
    conn: &mut PgConnection,
    id: Uuid,
    issue_state: &str,
    now: NaiveDateTime,
) -> Result<(), String> {
    diesel::update(feedback_submissions::table.filter(feedback_submissions::id.eq(id)))
        .set((
            feedback_submissions::issue_state.eq(issue_state),
            feedback_submissions::issue_state_checked_at.eq(now),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to record the issue's state: {e}"))?;
    Ok(())
}

/// Mark a submission's evidence gone. The attachment rows stay — they are what
/// lets the submission still say what was attached — and only their bytes go.
pub fn record_purged(
    conn: &mut PgConnection,
    submission_id: Uuid,
    now: NaiveDateTime,
) -> Result<(), String> {
    conn.transaction::<(), diesel::result::Error, _>(|conn| {
        diesel::update(
            feedback_attachments::table
                .filter(feedback_attachments::submission_id.eq(submission_id))
                .filter(feedback_attachments::purged_at.is_null()),
        )
        .set(feedback_attachments::purged_at.eq(now))
        .execute(conn)?;
        diesel::update(
            feedback_submissions::table.filter(feedback_submissions::id.eq(submission_id)),
        )
        .set(feedback_submissions::attachments_purged_at.eq(now))
        .execute(conn)?;
        Ok(())
    })
    .map_err(|e| format!("Failed to record the purge: {e}"))
}

fn connection(
    state: &AppState,
) -> Result<diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>, String> {
    state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())
}

/// How the rest of the server reaches the tracker.
///
/// Held by `AppState`, and empty in production — the host is built from the
/// settings as they resolve right now, so an operator correcting a client ID
/// does not have to restart (the reason `mail::MailSeam` gives at length). A
/// test puts one implementation in it and asserts what would have been sent
/// without sending it.
#[derive(Clone, Default)]
pub struct FeedbackSeam {
    override_host: Option<Arc<dyn host::IssueHost>>,
}

impl FeedbackSeam {
    /// Production. Nothing is held; the host comes from the settings.
    pub fn from_settings() -> Self {
        FeedbackSeam::default()
    }

    /// Tests, and only tests. The one construction that pins a host.
    pub fn overridden(host: Arc<dyn host::IssueHost>) -> Self {
        FeedbackSeam {
            override_host: Some(host),
        }
    }

    /// The host for the settings and the destination as they stand right now,
    /// or the declarations an operator has to set before there is one.
    pub fn host(
        &self,
        settings: &crate::settings::resolver::Settings,
        destination: Option<&DestinationRow>,
    ) -> Result<Arc<dyn host::IssueHost>, Vec<&'static str>> {
        if let Some(overridden) = &self.override_host {
            return Ok(Arc::clone(overridden));
        }
        let Some(destination) = destination else {
            // No destination row at all. Not an error, and not a lost
            // submission: FR-030 says an unconfigured instance still collects
            // feedback, it just cannot forward it yet.
            return Err(vec!["github_app.feedback.client_id"]);
        };
        use crate::repo_host::scoped::{AppScope, missing_keys, registration_for};
        match registration_for(AppScope::Feedback, settings) {
            Ok(app) => Ok(Arc::new(host::GitHubIssueHost::new(
                app,
                destination.clone(),
            ))),
            Err(problems) => Err(missing_keys(&problems)),
        }
    }
}

/// One lock for one table, shared by every test module in this feature.
///
/// **Two mutexes over one resource is not serialisation.** The suite was
/// flaking earlier today for exactly that reason elsewhere, and this feature
/// has two test files reading across `feedback_submissions` — the record's
/// listing tests and delivery's selection tests. They take this, not one each.
///
/// A `tokio::sync` mutex rather than a `std` one because these tests are async
/// and clippy rightly refuses a std guard held across an await.
#[cfg(test)]
pub(crate) static TEST_TABLE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Empty the feature's tables, for a test that reads across them.
///
/// [`schedule::due_now`] is bounded per tick and ordered oldest-first, so a
/// test asserting that *its* submission is due is otherwise asserting about the
/// twenty oldest rows the table happens to hold — which passes alone and fails
/// in a suite. Called under [`TEST_TABLE`], never anywhere else.
#[cfg(test)]
pub(crate) fn clear_for_test(conn: &mut PgConnection) {
    diesel::delete(feedback_submissions::table)
        .execute(conn)
        .expect("the feedback tables can be emptied between tests");
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod record_tests;
