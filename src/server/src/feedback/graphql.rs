//! The submitter's surface and the operator's, in one file.
//!
//! # Why this lives here and not in `graphql/`
//!
//! The same reason `settings/graphql.rs` and `mail/graphql.rs` give: the
//! record, the delivery pass and the surface that renders them are read
//! together, and adding to feedback touches one directory. The roots merge it
//! by name like any other member — two lines in `graphql.rs`.
//!
//! # The two surfaces are separate objects, deliberately
//!
//! [`FeedbackQuery`] answers only about the caller's **own** submissions.
//! There is no argument by which one account could read another's, and adding
//! one would be a different feature with a different review (FR-022). The
//! operator's view is [`FeedbackAdminQuery`], with an administrator check —
//! not a wider version of the same field with a flag.
//!
//! # `submitFeedback` records and returns
//!
//! It writes the submission, its attachment rows and its stored objects, and
//! it **does not call the destination**. FR-018 is that rule and nothing else,
//! which is also why this mutation cannot fail because GitHub is down.

use async_graphql::{
    Context, Enum, ErrorExtensions, InputObject, Object, Result as GraphQLResult, SimpleObject,
};
use base64::Engine as _;
use base64::engine::general_purpose;
use uuid::Uuid;

use super::deliver::FailureReason;
use super::{AttachmentKind, DeliveryState, Kind, NewAttachment, NewSubmission, rate_limit};
use crate::graphql::{admin_user, app_state, authenticated_user};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FeedbackKind")]
pub enum GraphQLFeedbackKind {
    FeatureRequest,
    Issue,
    General,
}

impl From<GraphQLFeedbackKind> for Kind {
    fn from(value: GraphQLFeedbackKind) -> Self {
        match value {
            GraphQLFeedbackKind::FeatureRequest => Kind::FeatureRequest,
            GraphQLFeedbackKind::Issue => Kind::Issue,
            GraphQLFeedbackKind::General => Kind::General,
        }
    }
}

impl From<Kind> for GraphQLFeedbackKind {
    fn from(value: Kind) -> Self {
        match value {
            Kind::FeatureRequest => GraphQLFeedbackKind::FeatureRequest,
            Kind::Issue => GraphQLFeedbackKind::Issue,
            Kind::General => GraphQLFeedbackKind::General,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FeedbackAttachmentKind")]
pub enum GraphQLAttachmentKind {
    Logs,
    Screenshot,
}

impl From<GraphQLAttachmentKind> for AttachmentKind {
    fn from(value: GraphQLAttachmentKind) -> Self {
        match value {
            GraphQLAttachmentKind::Logs => AttachmentKind::Logs,
            GraphQLAttachmentKind::Screenshot => AttachmentKind::Screenshot,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FeedbackDeliveryState")]
pub enum GraphQLDeliveryState {
    Pending,
    Delivered,
    Abandoned,
}

impl From<DeliveryState> for GraphQLDeliveryState {
    fn from(value: DeliveryState) -> Self {
        match value {
            DeliveryState::Pending => GraphQLDeliveryState::Pending,
            DeliveryState::Delivered => GraphQLDeliveryState::Delivered,
            DeliveryState::Abandoned => GraphQLDeliveryState::Abandoned,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FeedbackIssueState")]
pub enum GraphQLIssueState {
    Open,
    Closed,
}

/// The closed vocabulary FR-021 requires, as an enum and not a string.
///
/// **That is the point of it.** A free-text field carrying a host's response
/// body is one 401 payload away from disclosing a credential, and the failure
/// would be invisible in review because the string looked fine on the day it
/// was written. A closed vocabulary cannot leak.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FeedbackFailureReason")]
pub enum GraphQLFailureReason {
    NotConfigured,
    CredentialsRejected,
    DestinationNotFound,
    PermissionRefused,
    HostUnavailable,
    HostRateLimited,
    RejectedByHost,
    AttachmentsExpired,
}

impl From<FailureReason> for GraphQLFailureReason {
    fn from(value: FailureReason) -> Self {
        match value {
            FailureReason::NotConfigured => GraphQLFailureReason::NotConfigured,
            FailureReason::CredentialsRejected => GraphQLFailureReason::CredentialsRejected,
            FailureReason::DestinationNotFound => GraphQLFailureReason::DestinationNotFound,
            FailureReason::PermissionRefused => GraphQLFailureReason::PermissionRefused,
            FailureReason::HostUnavailable => GraphQLFailureReason::HostUnavailable,
            FailureReason::HostRateLimited => GraphQLFailureReason::HostRateLimited,
            FailureReason::RejectedByHost => GraphQLFailureReason::RejectedByHost,
            FailureReason::AttachmentsExpired => GraphQLFailureReason::AttachmentsExpired,
        }
    }
}

/// An attachment the person approved.
///
/// The bytes are inline because the review step showed exactly these bytes and
/// nothing may re-derive them server-side (research § R6).
#[derive(InputObject, Debug, Clone)]
#[graphql(name = "FeedbackAttachmentInput")]
pub struct GraphQLAttachmentInput {
    pub kind: GraphQLAttachmentKind,
    /// Base64. Logs are UTF-8 text; a screenshot is a PNG the client encoded.
    pub content: String,
    pub entries_kept: Option<i32>,
    pub entries_dropped: Option<i32>,
    pub redaction_count: Option<i32>,
}

#[derive(InputObject, Debug, Clone)]
#[graphql(name = "SubmitFeedbackInput")]
pub struct GraphQLSubmitFeedbackInput {
    pub kind: GraphQLFeedbackKind,
    /// What the person typed; never redacted, never rewritten.
    pub message: String,
    pub summary: Option<String>,
    pub screen_path: Option<String>,
    /// The game system is **not** accepted here — the server resolves it from
    /// the world, because a client that could name the system could name a
    /// different world's.
    pub world_id: Option<Uuid>,
    pub client_version: String,
    /// Coarse browser family and platform. Never the full User-Agent.
    pub browser: String,
    pub attachments: Vec<GraphQLAttachmentInput>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "FeedbackAttachmentSummary")]
pub struct GraphQLAttachmentSummary {
    pub id: Uuid,
    pub kind: GraphQLAttachmentKind,
    pub byte_size: i32,
    pub entries_kept: Option<i32>,
    pub entries_dropped: Option<i32>,
    pub redaction_count: i32,
    /// Set once this instance's copy has been removed. The row outlives its
    /// bytes.
    pub purged_at: Option<String>,
}

/// One submission, as the person who sent it sees it.
///
/// **No attachment bytes.** A summary carries sizes and counts; the bytes are
/// fetched over an asset route with a content type, matching every other asset
/// in this product.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "FeedbackSubmission")]
pub struct GraphQLFeedbackSubmission {
    pub id: Uuid,
    pub kind: GraphQLFeedbackKind,
    pub message: String,
    pub summary: Option<String>,
    pub created_at: String,
    /// `PENDING` until delivered. The person is never shown a failure: at the
    /// instance, which FR-018 makes the record, it did arrive.
    pub delivery_state: GraphQLDeliveryState,
    pub issue_url: Option<String>,
    pub issue_state: Option<GraphQLIssueState>,
    /// Shown with the state, never without it — a maintainer closes an issue
    /// without telling us.
    pub issue_state_checked_at: Option<String>,
    pub attachments: Vec<GraphQLAttachmentSummary>,
    /// When this instance's copies expire. The destination's copy is
    /// unaffected, which is what the person was told before they submitted.
    pub attachments_expire_at: String,
}

/// What the person must be told **before** they submit (FR-014).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "FeedbackDestinationNotice")]
pub struct GraphQLDestinationNotice {
    /// False when nothing is configured. Submission still works (FR-030).
    pub configured: bool,
    pub repository: Option<String>,
    /// Determined from the host, never assumed. Null means never successfully
    /// checked, and the safe rendering of an unknown destination is to warn
    /// rather than to reassure.
    pub is_public: Option<bool>,
    pub visibility_checked_at: Option<String>,
}

/// One undelivered submission, as an operator sees it.
///
/// Carries **no message, no attachment, and no host text** — this is a
/// diagnostic surface, and what a person wrote is theirs.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "UndeliveredFeedback")]
pub struct GraphQLUndeliveredFeedback {
    pub id: Uuid,
    pub kind: GraphQLFeedbackKind,
    pub created_at: String,
    pub delivery_state: GraphQLDeliveryState,
    pub attempt_count: i32,
    pub last_attempt_at: Option<String>,
    pub last_failure_reason: Option<GraphQLFailureReason>,
    pub next_attempt_after: Option<String>,
    /// The instance's copies expire here, delivered or not.
    pub attachments_expire_at: String,
}

fn timestamp(value: chrono::NaiveDateTime) -> String {
    format!("{}Z", value.and_utc().format("%Y-%m-%dT%H:%M:%S"))
}

fn view(
    row: &super::SubmissionRow,
    attachments: &[super::AttachmentRow],
) -> GraphQLFeedbackSubmission {
    GraphQLFeedbackSubmission {
        id: row.id,
        kind: row.kind().into(),
        message: row.message.clone(),
        summary: row.summary.clone(),
        created_at: timestamp(row.created_at),
        delivery_state: row.delivery_state().into(),
        issue_url: row.issue_url.clone(),
        issue_state: match row.issue_state.as_deref() {
            Some("open") => Some(GraphQLIssueState::Open),
            Some("closed") => Some(GraphQLIssueState::Closed),
            _ => None,
        },
        issue_state_checked_at: row.issue_state_checked_at.map(timestamp),
        attachments: attachments
            .iter()
            .filter(|a| a.submission_id == row.id)
            .map(|a| GraphQLAttachmentSummary {
                id: a.id,
                kind: match a.kind() {
                    Some(AttachmentKind::Screenshot) => GraphQLAttachmentKind::Screenshot,
                    _ => GraphQLAttachmentKind::Logs,
                },
                byte_size: a.byte_size.min(i64::from(i32::MAX)) as i32,
                entries_kept: a.entries_kept,
                entries_dropped: a.entries_dropped,
                redaction_count: a.redaction_count,
                purged_at: a.purged_at.map(timestamp),
            })
            .collect(),
        attachments_expire_at: timestamp(row.attachments_expire_at),
    }
}

#[derive(Default)]
pub struct FeedbackQuery;

#[Object]
impl FeedbackQuery {
    /// What this build is, for a report to name.
    ///
    /// Spec 037 T015. The client stamps its own version onto a submission and
    /// the server stamps this one — two numbers, because a bug reported
    /// against a client that has since been redeployed is a bug against a
    /// build nobody can identify afterwards.
    ///
    /// Authenticated but not administrator-only: the feedback dialog is open
    /// to every signed-in person and this is what it puts in the report.
    /// `option_env!` rather than a runtime lookup, so a build with no git SHA
    /// says "unknown build" rather than carrying a field that is sometimes
    /// missing.
    async fn server_version(&self, ctx: &Context<'_>) -> GraphQLResult<String> {
        let _ = crate::graphql::authenticated_user(ctx)?;
        Ok(super::server_version())
    }
    /// Everything the calling account has submitted, newest first.
    ///
    /// **Only the caller's own** (FR-022). There is no argument here by which
    /// another account's rows could be reached, and that absence is the
    /// enforcement.
    async fn my_submissions(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLFeedbackSubmission>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        let rows = super::mine(&mut conn, user.user_id)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let attachments = super::attachments_of(&mut conn, row.id)?;
            out.push(view(row, &attachments));
        }
        Ok(out)
    }

    /// The pre-submission notice. Requires a session; discloses no credential
    /// — the destination row has no credential column by design.
    async fn feedback_destination_notice(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLDestinationNotice> {
        let state = app_state(ctx)?;
        let _ = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        Ok(match super::destination(&mut conn)? {
            None => GraphQLDestinationNotice {
                configured: false,
                repository: None,
                is_public: None,
                visibility_checked_at: None,
            },
            Some(destination) => GraphQLDestinationNotice {
                configured: true,
                repository: Some(destination.repository_ref.clone()),
                is_public: destination.is_public,
                visibility_checked_at: destination.visibility_checked_at.map(timestamp),
            },
        })
    }
}

#[derive(Default)]
pub struct FeedbackAdminQuery;

#[Object]
impl FeedbackAdminQuery {
    /// Everything pending or abandoned, oldest first. Administrators only.
    ///
    /// FR-021: the reason for each, in a vocabulary that cannot carry a
    /// credential or a fragment of one.
    async fn undelivered_feedback(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLUndeliveredFeedback>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        let rows = super::deliver::undelivered(&mut conn)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let attempts = super::schedule::attempts_of(&mut conn, row.id)?;
            let latest = attempts.first();
            out.push(GraphQLUndeliveredFeedback {
                id: row.id,
                kind: row.kind().into(),
                created_at: timestamp(row.created_at),
                delivery_state: row.delivery_state().into(),
                attempt_count: attempts.len() as i32,
                last_attempt_at: latest.map(|(_, at, _)| timestamp(*at)),
                last_failure_reason: latest
                    .and_then(|(_, _, reason)| reason.as_deref())
                    .and_then(FailureReason::parse)
                    .map(Into::into),
                next_attempt_after: latest.map(|(attempt, at, _)| {
                    timestamp(super::schedule::next_attempt_after(*at, *attempt))
                }),
                attachments_expire_at: timestamp(row.attachments_expire_at),
            });
        }
        Ok(out)
    }
}

#[derive(Default)]
pub struct FeedbackMutation;

#[Object]
impl FeedbackMutation {
    /// Record a submission.
    ///
    /// Returns after the database write and **before any delivery attempt**
    /// (FR-018), so this never fails because the destination is down,
    /// unreachable or unconfigured.
    async fn submit_feedback(
        &self,
        ctx: &Context<'_>,
        input: GraphQLSubmitFeedbackInput,
    ) -> GraphQLResult<GraphQLFeedbackSubmission> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;

        if let Err(seconds) = rate_limit::allow(&user.user_id.to_string()) {
            return Err(
                async_graphql::Error::new(rate_limit::refusal(seconds)).extend_with(|_, ext| {
                    ext.set("code", rate_limit::RATE_LIMITED);
                    ext.set("retryAfterSeconds", seconds);
                }),
            );
        }

        let message = input.message.trim().to_string();
        if message.is_empty() {
            return Err(async_graphql::Error::new(
                "A submission needs something written in it.",
            ));
        }

        let submitter_email = submitter_email(state, user.user_id)?;
        let mut attachments = Vec::with_capacity(input.attachments.len());
        for attachment in &input.attachments {
            attachments.push(prepare(attachment, &submitter_email)?);
        }

        let row = super::record(
            state,
            NewSubmission {
                user_id: user.user_id,
                kind: input.kind.into(),
                message,
                summary: input
                    .summary
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                screen_path: input.screen_path.clone(),
                world_id: input.world_id,
                client_version: input.client_version.clone(),
                browser: input.browser.clone(),
                attachments,
            },
        )
        .await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        let stored = super::attachments_of(&mut conn, row.id)?;
        Ok(view(&row, &stored))
    }

    /// Stop retrying an item that will never succeed. Administrators only,
    /// and reversible.
    async fn abandon_feedback_delivery(
        &self,
        ctx: &Context<'_>,
        submission_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        Ok(super::abandon(&mut conn, submission_id, admin.user_id)?)
    }

    /// Return an abandoned item to the queue, after fixing what was wrong.
    async fn resume_feedback_delivery(
        &self,
        ctx: &Context<'_>,
        submission_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        Ok(super::resume(&mut conn, submission_id, admin.user_id)?)
    }
}

/// Validate one approved attachment and decode its bytes.
///
/// **The server refuses; it never rewrites.** A match on the shared rule set
/// is `FEEDBACK_CONTAINS_SECRET`, naming the kind found, and nothing is
/// written — an edit after approval would mean the person saw something other
/// than what was sent, which is the promise FR-012 exists to keep.
fn prepare(
    attachment: &GraphQLAttachmentInput,
    submitter_email: &str,
) -> GraphQLResult<NewAttachment> {
    let kind: AttachmentKind = attachment.kind.into();
    let raw = general_purpose::STANDARD
        .decode(attachment.content.trim())
        .map_err(|_| async_graphql::Error::new("An attachment was not valid base64."))?;

    if raw.len() > crate::storage::transcode::MAX_UPLOAD_BYTES {
        return Err(async_graphql::Error::new(
            "That attachment is larger than this instance accepts.",
        ));
    }

    if let Some(found) = super::redaction::refusal_for(&raw, submitter_email) {
        return Err(async_graphql::Error::new(format!(
            "That attachment still contains something that must not leave your \
                 browser ({found}). Nothing was sent, and what you wrote is kept."
        ))
        .extend_with(|_, ext| {
            ext.set("code", "FEEDBACK_CONTAINS_SECRET");
            ext.set("kind", found);
        }));
    }

    // A screenshot goes through the transcoder every image in this product
    // goes through, inheriting `MAX_UPLOAD_BYTES` and the `TooLarge` refusal
    // that is checked before any decode work. A log bundle is text and is
    // stored exactly as approved.
    let content = match kind {
        AttachmentKind::Logs => raw,
        AttachmentKind::Screenshot => {
            crate::storage::transcode::transcode_to_webp(&raw)
                .map_err(|e| {
                    async_graphql::Error::new(format!("That screenshot could not be read: {e}"))
                })?
                .webp_bytes
        }
    };

    Ok(NewAttachment {
        kind,
        content,
        entries_kept: attachment.entries_kept,
        entries_dropped: attachment.entries_dropped,
        redaction_count: attachment.redaction_count.unwrap_or(0),
    })
}

/// The submitter's own address, for the one redaction rule that cannot be a
/// constant (FR-013). Read from the row, never from the client.
fn submitter_email(state: &crate::state::AppState, user_id: Uuid) -> GraphQLResult<String> {
    use crate::schema::users;
    use diesel::prelude::*;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
    let email: Option<String> = users::table
        .filter(users::id.eq(user_id))
        .select(users::email)
        .first(&mut conn)
        .optional()
        .map_err(|_| async_graphql::Error::new("Failed to read the account"))?;
    Ok(email.unwrap_or_default())
}

#[cfg(test)]
mod prepare_tests {
    use super::*;
    use base64::Engine as _;

    fn attachment(content: &str) -> GraphQLAttachmentInput {
        GraphQLAttachmentInput {
            kind: GraphQLAttachmentKind::Logs,
            content: general_purpose::STANDARD.encode(content),
            entries_kept: None,
            entries_dropped: None,
            redaction_count: None,
        }
    }

    /// FR-012, and the guard `quickstart.md` asks to be broken on purpose:
    /// **the server refuses; it never rewrites.**
    ///
    /// Asserted on `prepare` rather than through the resolver because this is
    /// where the decision is made, and because the failure worth catching is
    /// somebody "being helpful" — stripping the secret and carrying on. Before
    /// this test existed that change left **every** test in the crate passing:
    /// the submission still succeeded, the stored payload still contained no
    /// secret, and the person had silently had their approved evidence edited.
    #[test]
    fn an_attachment_carrying_a_secret_is_refused_and_not_scrubbed() {
        let leaking =
            attachment("GET /api/worlds\nauthorization: Bearer abcdefgh12345678ABCDEFGH\n");
        let refused = prepare(&leaking, "someone@example.org")
            .expect_err("a bundle carrying a bearer token must be refused");

        let message = refused.message.clone();
        assert!(
            message.contains("bearer_token"),
            "the refusal does not name the kind it matched: {message}"
        );
        // And it never quotes the payload — an error message is one of the
        // places a credential gets logged next.
        assert!(!message.contains("abcdefgh12345678"), "{message}");
    }

    /// The false positive that would silently refuse every *correctly*
    /// filtered bundle: the client's own marker contains the words the bearer
    /// rule matches.
    #[test]
    fn a_correctly_redacted_bundle_is_accepted() {
        let clean = attachment("GET /api/worlds\nauthorization: [redacted: bearer token]\n");
        let prepared = prepare(&clean, "someone@example.org")
            .expect("a redacted bundle must not be refused for its own marker");
        assert_eq!(prepared.kind, AttachmentKind::Logs);
    }

    /// The submitter's own address is a rule too, and the only one that
    /// depends on who is asking rather than on the bytes alone.
    #[test]
    fn the_submitters_own_address_is_refused_for_them_and_nobody_else() {
        let carrying = attachment("signed in as archmage@example.org\n");
        assert!(
            prepare(&carrying, "archmage@example.org").is_err(),
            "a bundle carrying the submitter's own address was accepted"
        );
        assert!(
            prepare(&carrying, "someone-else@example.org").is_ok(),
            "another account's address was treated as this submitter's"
        );
    }
}
