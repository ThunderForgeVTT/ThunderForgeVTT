//! The operator's view of mail: whether it works, proof that it works, and
//! what this instance has failed to send.
//!
//! # No field returns a message body. Ever.
//!
//! Not for a test message, not for an administrator, not in a diagnostic
//! (FR-016, contracts/mail.md rule 1). [`GraphQLOutboxEntry`] has no body
//! field and the absence is not an oversight — the test at the bottom of this
//! file reads the generated SDL and asserts it, so adding one is a failing
//! test rather than a review someone has to catch.
//!
//! # Why this lives here and not in `graphql/`
//!
//! The same reason `settings/graphql.rs` gives: the transport, the outbox, the
//! sender and the surface that renders them are read together, and adding to
//! mail touches one directory. The roots merge it by name like any other
//! member — two lines in `graphql.rs`, listed at the bottom of this file.
//!
//! # Why `sendTestMail` is rate limited as well as administrators-only
//!
//! It sends a message to an arbitrary address on request. Without both, that
//! is an open relay with a login page. The limiter is the one spec 035 already
//! uses for anonymous share reads rather than a second one, so this product
//! has one idea of what rate limiting looks like.

use async_graphql::{Context, Enum, Object, Result as GraphQLResult, SimpleObject};
use uuid::Uuid;

use super::outbox::{self, OutboxRow, OutboxState, PURPOSE_TEST};
use super::schedule;
use crate::graphql::{admin_user, app_state, share_rate_limit};
use crate::state::AppState;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "OutboxState")]
pub enum GraphQLOutboxState {
    Queued,
    Blocked,
    Sending,
    Sent,
    Failed,
}

impl From<OutboxState> for GraphQLOutboxState {
    fn from(value: OutboxState) -> Self {
        match value {
            OutboxState::Queued => GraphQLOutboxState::Queued,
            OutboxState::Blocked => GraphQLOutboxState::Blocked,
            OutboxState::Sending => GraphQLOutboxState::Sending,
            OutboxState::Sent => GraphQLOutboxState::Sent,
            OutboxState::Failed => GraphQLOutboxState::Failed,
        }
    }
}

impl From<GraphQLOutboxState> for OutboxState {
    fn from(value: GraphQLOutboxState) -> Self {
        match value {
            GraphQLOutboxState::Queued => OutboxState::Queued,
            GraphQLOutboxState::Blocked => OutboxState::Blocked,
            GraphQLOutboxState::Sending => OutboxState::Sending,
            GraphQLOutboxState::Sent => OutboxState::Sent,
            GraphQLOutboxState::Failed => OutboxState::Failed,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "MailAvailability")]
pub struct GraphQLMailAvailability {
    pub ready: bool,
    /// Setting keys that are unset, by name. Never a value.
    pub missing: Vec<String>,
    /// What is limited while mail is unavailable, in sentences.
    pub limited_features: Vec<String>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TestMailResult")]
pub struct GraphQLTestMailResult {
    pub delivered: bool,
    /// On failure: what to fix. Never a password or any fragment of one
    /// (FR-014).
    pub reason: Option<String>,
    /// The row this attempt left behind, so an operator can find it later
    /// whether it worked or not.
    pub outbox_id: Uuid,
}

/// One message, as an operator may see it.
///
/// Recipient, purpose, state, attempts, timestamps and the failure reason —
/// and nothing else. `subject` is present only for the instance's own test
/// message, because a subject about a person is content.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "OutboxEntry")]
pub struct GraphQLOutboxEntry {
    pub id: Uuid,
    pub purpose: String,
    pub to_address: String,
    pub subject: Option<String>,
    pub state: GraphQLOutboxState,
    pub attempts: i32,
    pub last_attempt_at: Option<String>,
    pub next_attempt_at: Option<String>,
    pub last_failure_reason: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
}

impl GraphQLOutboxEntry {
    pub fn from_row(state: &AppState, row: &OutboxRow) -> GraphQLOutboxEntry {
        GraphQLOutboxEntry {
            id: row.id,
            purpose: row.purpose.clone(),
            to_address: row.to_address.clone(),
            subject: row.subject_if_disclosable(state),
            state: row.state().unwrap_or(OutboxState::Failed).into(),
            attempts: row.attempts,
            last_attempt_at: row.last_attempt_at.map(|t| t.to_string()),
            next_attempt_at: row.next_attempt_at.map(|t| t.to_string()),
            last_failure_reason: row.last_failure_reason.clone(),
            sent_at: row.sent_at.map(|t| t.to_string()),
            created_at: row.created_at.to_string(),
        }
    }
}

#[derive(Default)]
pub struct MailQuery;

#[Object]
impl MailQuery {
    /// Whether this instance can send mail, and what to set if it cannot.
    /// Administrators only.
    async fn mail_availability(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLMailAvailability> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let availability = super::transport_now(state).await?.availability();

        Ok(GraphQLMailAvailability {
            ready: availability.is_ready(),
            missing: availability
                .missing()
                .iter()
                .map(|k| k.to_string())
                .collect(),
            limited_features: availability.limited_features(),
        })
    }

    /// The outbox, newest first. Administrators only.
    async fn mail_outbox(
        &self,
        ctx: &Context<'_>,
        state: Option<GraphQLOutboxState>,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLOutboxEntry>> {
        let app = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let mut conn = app
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        let rows = outbox::list(
            &mut conn,
            state.map(OutboxState::from),
            limit.unwrap_or(50).into(),
        )?;
        Ok(rows
            .iter()
            .map(|row| GraphQLOutboxEntry::from_row(app, row))
            .collect())
    }
}

#[derive(Default)]
pub struct MailMutation;

#[Object]
impl MailMutation {
    /// Send a message to the address given, using the settings as they resolve
    /// now, and report the outcome. The proof FR-013 asks for, before anything
    /// depends on delivery.
    ///
    /// It goes through the outbox like everything else, so its failure is
    /// recorded where every other failure is and the code path under test is
    /// the code path in production. A test that exercises a shortcut proves
    /// the shortcut.
    async fn send_test_mail(
        &self,
        ctx: &Context<'_>,
        to: String,
    ) -> GraphQLResult<GraphQLTestMailResult> {
        let app = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        if !share_rate_limit::allow_request(&format!("test-mail:{}", admin.user_id)) {
            return Err(async_graphql::Error::new(
                share_rate_limit::rate_limited_message(),
            ));
        }
        let to = to.trim().to_string();
        if !is_an_address(&to) {
            return Err(async_graphql::Error::new(
                "Give an email address of the form name@example.org to send the test to.",
            ));
        }

        let id = outbox::enqueue(
            app,
            outbox::NewOutboxMessage {
                purpose: PURPOSE_TEST.to_string(),
                to_address: to,
                subject: "ThunderForge: mail is configured".to_string(),
                body_text: TEST_BODY.to_string(),
                created_by: Some(admin.user_id),
            },
        )
        .await?;

        let outcome = schedule::attempt(app, id).await?;
        let mut conn = app
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        let row = outbox::load(&mut conn, id)?
            .ok_or_else(|| async_graphql::Error::new("The test message could not be read back"))?;

        Ok(GraphQLTestMailResult {
            delivered: outcome == OutboxState::Sent,
            reason: row.last_failure_reason.clone(),
            outbox_id: id,
        })
    }

    /// Try a blocked or failed message again now, rather than waiting for the
    /// backoff curve. Refused for a message that already went.
    async fn retry_outbox_message(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<GraphQLOutboxEntry> {
        let app = app_state(ctx)?;
        let _ = admin_user(ctx)?;

        let mut conn = app
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        let row = outbox::load(&mut conn, id)?
            .ok_or_else(|| async_graphql::Error::new("There is no such message in the outbox."))?;
        outbox::requeue_now(&mut conn, &row)?;
        drop(conn);

        schedule::attempt(app, id).await?;

        let mut conn = app
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        let row = outbox::load(&mut conn, id)?
            .ok_or_else(|| async_graphql::Error::new("There is no such message in the outbox."))?;
        Ok(GraphQLOutboxEntry::from_row(app, &row))
    }
}

/// The body of the operator's test message. It says what it is, because it
/// arrives in a mailbox with no context and somebody who did not press the
/// button may read it.
const TEST_BODY: &str = "This is a test message from a ThunderForge instance. \
Somebody with administrator access asked it to prove that mail delivery works. \
Nothing else has happened, and no action is needed.";

/// The shape check `sendTestMail` applies to an address before writing a row
/// for it. Deliberately not the settings validator: that one refuses reserved
/// TLDs, which is right for an address a copyright notice is served on and
/// wrong for the address a developer's local mail catcher listens as.
fn is_an_address(value: &str) -> bool {
    let mut parts = value.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptySubscription, Schema};

    use super::*;

    fn sdl() -> String {
        Schema::build(MailQuery, MailMutation, EmptySubscription)
            .finish()
            .sdl()
    }

    /// FR-016, mechanically and against the generated schema rather than
    /// against the struct: no read surface in this module returns a message
    /// body, under any spelling somebody might add one by.
    #[test]
    fn no_read_surface_returns_a_message_body() {
        let sdl = sdl();
        let entry = sdl
            .split("type OutboxEntry")
            .nth(1)
            .expect("OutboxEntry is in the schema")
            .split('}')
            .next()
            .expect("its fields");
        for forbidden in ["body", "bodyText", "content", "message", "html"] {
            assert!(
                !entry.to_ascii_lowercase().contains(forbidden),
                "OutboxEntry gained a `{forbidden}` field: {entry}"
            );
        }
    }

    /// The surface an operator reads is exactly the one data-model.md § 3
    /// lists. Asserted positively as well, so a field silently disappearing is
    /// also a failing test.
    #[test]
    fn the_outbox_entry_shows_what_an_operator_needs_and_no_more() {
        let sdl = sdl();
        for expected in [
            "toAddress",
            "purpose",
            "state",
            "attempts",
            "lastFailureReason",
            "sentAt",
        ] {
            assert!(sdl.contains(expected), "OutboxEntry lost `{expected}`");
        }
    }

    /// `missing` is a list of setting keys. If it ever became a list of
    /// values, this is the surface it would leak from.
    #[test]
    fn availability_reports_keys_and_sentences_rather_than_values() {
        let availability = super::super::Availability::Unconfigured {
            missing: vec!["mail.host", "mail.password"],
        };
        let rendered = GraphQLMailAvailability {
            ready: availability.is_ready(),
            missing: availability
                .missing()
                .iter()
                .map(|k| k.to_string())
                .collect(),
            limited_features: availability.limited_features(),
        };
        assert_eq!(rendered.missing, vec!["mail.host", "mail.password"]);
        assert!(!rendered.ready);
        assert!(!rendered.limited_features.is_empty());
    }

    #[test]
    fn an_address_is_checked_for_shape_but_not_for_a_reserved_domain() {
        assert!(is_an_address("operator@example.invalid"));
        assert!(is_an_address("someone@mail.operator.org"));
        assert!(!is_an_address("operator"));
        assert!(!is_an_address("operator@"));
        assert!(!is_an_address("@example.org"));
        assert!(!is_an_address("a@b@example.org"));
        assert!(!is_an_address("operator@localhost"));
    }
}
