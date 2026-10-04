//! Every message this instance means to send, whether or not it can be sent.
//!
//! # The rule this module exists to make true
//!
//! **Nothing calls [`MailTransport::send`](super::MailTransport::send) except
//! [`super::schedule`].** A feature that wants to tell somebody something
//! calls [`enqueue`], which writes a durable row *before* any transport is
//! consulted. That ordering is what makes FR-015 — "with no mail configured
//! the instance must not silently discard messages" — true by construction
//! rather than by everybody remembering to check.
//!
//! # Why `blocked` is not `failed`
//!
//! They are different sentences to an operator and they have different fixes.
//! `blocked` is "this instance has nowhere to send yet, and configuring mail
//! releases it". `failed` is "it was tried, the retry curve ran out, and it is
//! not going". Collapsing them would make an instance that has never had a
//! mail server indistinguishable from one whose mail server refuses it, and
//! the first of those is a setup step while the second is an incident.
//!
//! # Why the stored message is encrypted
//!
//! A retry has to send the message that failed, not a re-rendered
//! approximation of it — re-rendering later would make the retry a *different*
//! message. So the text is stored, and it is stored as ciphertext
//! (`crypto.rs`) because FR-016 says the contents of somebody's message must
//! not reach anyone it was not for, and a stored plaintext body is one
//! `SELECT *` in a diagnostic away from that being false.
//!
//! Stated honestly, as research.md § D5 does: this defends the admin surface,
//! the logs and a leaked backup. It does not defend against the operator, who
//! holds `THUNDERFORGE_SECRET` and the database. FR-016 is a requirement about
//! the product's surfaces, and this is what the product can actually promise.

use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::schedule::next_attempt_after;
use super::{Availability, DeliveryFailure, OutgoingMessage};
use crate::schema::mail_outbox;
use crate::settings::resolver::{decrypt, encrypt};
use crate::state::AppState;

/// The five states, and the transitions between them.
///
/// An enum rather than bare strings so a typo is a compile error here and a
/// `CHECK` violation in the database, which are two independent chances to
/// catch the same mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxState {
    Queued,
    Blocked,
    Sending,
    Sent,
    Failed,
}

impl OutboxState {
    pub fn as_str(self) -> &'static str {
        match self {
            OutboxState::Queued => "queued",
            OutboxState::Blocked => "blocked",
            OutboxState::Sending => "sending",
            OutboxState::Sent => "sent",
            OutboxState::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<OutboxState> {
        match value {
            "queued" => Some(OutboxState::Queued),
            "blocked" => Some(OutboxState::Blocked),
            "sending" => Some(OutboxState::Sending),
            "sent" => Some(OutboxState::Sent),
            "failed" => Some(OutboxState::Failed),
            _ => None,
        }
    }

    /// Whether an operator asking for this to be tried again should be obeyed.
    /// A sent message is never re-sent (contracts/mail.md rule 6) and a
    /// message already in flight is not started twice.
    pub fn may_be_retried(self) -> bool {
        matches!(
            self,
            OutboxState::Blocked | OutboxState::Failed | OutboxState::Queued
        )
    }
}

/// A message to send. What a calling feature builds.
pub struct NewOutboxMessage {
    /// Why this exists — `"test"` today, and whatever specs 035, 037 and 039
    /// name later. It is what an operator reads in the list, so it is a word
    /// about the message and never about the recipient.
    pub purpose: String,
    pub to_address: String,
    pub subject: String,
    pub body_text: String,
    /// The person who caused it, when a person did. Null is "the instance".
    pub created_by: Option<Uuid>,
}

/// One row, as the server reads it back. Note what a caller gets and what it
/// has to ask for: the ciphertext columns are here because the sender needs
/// them, and [`super::graphql`] never projects them.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = mail_outbox)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OutboxRow {
    pub id: Uuid,
    pub purpose: String,
    pub to_address: String,
    pub subject_encrypted: String,
    pub body_encrypted: String,
    pub state: String,
    pub attempts: i32,
    pub last_attempt_at: Option<NaiveDateTime>,
    pub next_attempt_at: Option<NaiveDateTime>,
    pub last_failure_reason: Option<String>,
    pub sent_at: Option<NaiveDateTime>,
    pub created_by: Option<Uuid>,
    pub created_at: NaiveDateTime,
}

impl OutboxRow {
    pub fn state(&self) -> Option<OutboxState> {
        OutboxState::parse(&self.state)
    }

    /// The message as the transport wants it. Decrypting is a deliberate,
    /// named step: the only two callers are the sender and nothing else.
    pub fn decrypt(&self, state: &AppState) -> Result<OutgoingMessage, String> {
        Ok(OutgoingMessage {
            to: self.to_address.clone(),
            subject: decrypt(state, &self.subject_encrypted)?,
            body_text: decrypt(state, &self.body_encrypted)?,
        })
    }

    /// The subject, for the one case an operator may see it: a message the
    /// instance generated about itself. A subject about a person is content
    /// (data-model.md § 3), so this returns `None` for every other purpose
    /// rather than leaving the caller to remember.
    pub fn subject_if_disclosable(&self, state: &AppState) -> Option<String> {
        if self.purpose != PURPOSE_TEST {
            return None;
        }
        decrypt(state, &self.subject_encrypted).ok()
    }
}

/// The purpose an operator's own test message is filed under, and the only
/// purpose whose subject any surface will show.
pub const PURPOSE_TEST: &str = "test";

/// How many attempts a message gets before it is `failed`. The backoff curve's
/// own length — a message that has been refused for the whole of it is not
/// going to be accepted on the tenth try, and a queue that retries forever is
/// a queue that never tells anybody anything is wrong.
pub const MAX_ATTEMPTS: i32 = super::schedule::BACKOFF_LENGTH as i32;

/// What a message's state should become when an attempt fails.
///
/// Pure, and separate from the row it will be written to, for the reason
/// `lore_sync/schedule.rs` gives about selection: a rule enforced inside a
/// spawned task is a rule nothing can test. Here that rule is "a permanent
/// refusal is not retried, and a transient one is retried on the curve until
/// the curve runs out".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureOutcome {
    pub state: OutboxState,
    pub next_attempt_at: Option<NaiveDateTime>,
}

pub fn outcome_after_failure(
    attempts: i32,
    failure: &DeliveryFailure,
    now: NaiveDateTime,
) -> FailureOutcome {
    if !failure.retryable || attempts >= MAX_ATTEMPTS {
        return FailureOutcome {
            state: OutboxState::Failed,
            next_attempt_at: None,
        };
    }
    FailureOutcome {
        state: OutboxState::Queued,
        next_attempt_at: Some(next_attempt_after(now, attempts)),
    }
}

/// The state a message is written in, given what this instance can do right
/// now. The one place "we cannot send yet" becomes a row rather than a
/// discarded message.
pub fn initial_state(availability: &Availability) -> (OutboxState, Option<String>) {
    match availability {
        Availability::Ready => (OutboxState::Queued, None),
        Availability::Unconfigured { missing } => (
            OutboxState::Blocked,
            Some(super::Unconfigured::new(missing.clone()).reason()),
        ),
    }
}

/// Write a message down. **The only way anything in this server sends mail.**
///
/// Returns the row's id, which is what a caller records if it wants to say
/// later what became of the message. Never fails because mail is
/// unconfigured — that is the whole point; it fails only when the database
/// does.
pub async fn enqueue(state: &AppState, message: NewOutboxMessage) -> Result<Uuid, String> {
    let settings = crate::settings::resolver::resolve_all(state).await?;
    let availability = state.mail.transport(&settings).availability();
    let (initial, reason) = initial_state(&availability);

    let subject_encrypted = encrypt(state, &message.subject)?;
    let body_encrypted = encrypt(state, &message.body_text)?;
    let id = Uuid::now_v7();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    diesel::insert_into(mail_outbox::table)
        .values((
            mail_outbox::id.eq(id),
            mail_outbox::purpose.eq(&message.purpose),
            mail_outbox::to_address.eq(&message.to_address),
            mail_outbox::subject_encrypted.eq(subject_encrypted),
            mail_outbox::body_encrypted.eq(body_encrypted),
            mail_outbox::state.eq(initial.as_str()),
            mail_outbox::last_failure_reason.eq(reason),
            mail_outbox::created_by.eq(message.created_by),
            mail_outbox::created_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to enqueue message: {e}"))?;

    Ok(id)
}

pub fn load(conn: &mut PgConnection, id: Uuid) -> Result<Option<OutboxRow>, String> {
    mail_outbox::table
        .filter(mail_outbox::id.eq(id))
        .select(OutboxRow::as_select())
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to load outbox message: {e}"))
}

/// Newest first, optionally one state only. The operator's list.
pub fn list(
    conn: &mut PgConnection,
    state_filter: Option<OutboxState>,
    limit: i64,
) -> Result<Vec<OutboxRow>, String> {
    let mut query = mail_outbox::table
        .select(OutboxRow::as_select())
        .order(mail_outbox::created_at.desc())
        .limit(limit.clamp(1, 200))
        .into_boxed();
    if let Some(state) = state_filter {
        query = query.filter(mail_outbox::state.eq(state.as_str()));
    }
    query
        .load(conn)
        .map_err(|e| format!("Failed to list the outbox: {e}"))
}

/// Claim a message for sending. Returns false if somebody else already has it.
///
/// The single-flight guard, and it is a conditional `UPDATE` rather than a read
/// followed by a write because those are two statements and a second sender
/// fits between them. `lore_sync` gets the same property from "the latest run
/// has a null outcome"; here the in-flight marker is the state itself.
pub fn mark_sending(conn: &mut PgConnection, id: Uuid, now: NaiveDateTime) -> Result<bool, String> {
    let claimed = diesel::update(
        mail_outbox::table
            .filter(mail_outbox::id.eq(id))
            .filter(mail_outbox::state.eq(OutboxState::Queued.as_str())),
    )
    .set((
        mail_outbox::state.eq(OutboxState::Sending.as_str()),
        mail_outbox::attempts.eq(mail_outbox::attempts + 1),
        mail_outbox::last_attempt_at.eq(now),
    ))
    .execute(conn)
    .map_err(|e| format!("Failed to claim outbox message: {e}"))?;
    Ok(claimed == 1)
}

/// It went. `sent_at` is written in the same statement as the state, so the
/// idempotence guard a retry checks can never disagree with the state it
/// guards.
pub fn record_success(conn: &mut PgConnection, id: Uuid, now: NaiveDateTime) -> Result<(), String> {
    diesel::update(mail_outbox::table.filter(mail_outbox::id.eq(id)))
        .set((
            mail_outbox::state.eq(OutboxState::Sent.as_str()),
            mail_outbox::sent_at.eq(now),
            mail_outbox::next_attempt_at.eq(None::<NaiveDateTime>),
            mail_outbox::last_failure_reason.eq(None::<String>),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to record a delivery: {e}"))?;
    Ok(())
}

/// It did not go. The reason is stored as the transport phrased it, which is
/// prose naming a setting and never a value ([`super::smtp::scrub`]).
pub fn record_failure(
    conn: &mut PgConnection,
    row: &OutboxRow,
    failure: &DeliveryFailure,
    now: NaiveDateTime,
) -> Result<FailureOutcome, String> {
    let outcome = outcome_after_failure(row.attempts, failure, now);
    diesel::update(mail_outbox::table.filter(mail_outbox::id.eq(row.id)))
        .set((
            mail_outbox::state.eq(outcome.state.as_str()),
            mail_outbox::next_attempt_at.eq(outcome.next_attempt_at),
            mail_outbox::last_failure_reason.eq(&failure.reason),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to record a delivery failure: {e}"))?;
    Ok(outcome)
}

/// The instance has nowhere to send this. Not a failure, and not a discard.
pub fn block(conn: &mut PgConnection, id: Uuid, availability: &Availability) -> Result<(), String> {
    let (_, reason) = initial_state(availability);
    diesel::update(mail_outbox::table.filter(mail_outbox::id.eq(id)))
        .set((
            mail_outbox::state.eq(OutboxState::Blocked.as_str()),
            mail_outbox::next_attempt_at.eq(None::<NaiveDateTime>),
            mail_outbox::last_failure_reason.eq(reason),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to block an outbox message: {e}"))?;
    Ok(())
}

/// Mail became available: everything that was waiting on a setting is now
/// waiting on a send. Returns how many were released, which is what the
/// operator's log line says.
///
/// This is here rather than in the settings write path deliberately: a release
/// that only happens when somebody edits a setting misses the instance whose
/// value arrived from the environment on the next deploy, and this is asked on
/// every tick.
pub fn release_blocked(conn: &mut PgConnection) -> Result<usize, String> {
    diesel::update(mail_outbox::table.filter(mail_outbox::state.eq(OutboxState::Blocked.as_str())))
        .set((
            mail_outbox::state.eq(OutboxState::Queued.as_str()),
            mail_outbox::next_attempt_at.eq(None::<NaiveDateTime>),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to release blocked messages: {e}"))
}

/// An operator asking for one message to be tried again now.
///
/// Refuses a message that already went — `sent_at` is the guard, and it is
/// checked against the row rather than against a state string somebody could
/// have written twice.
pub fn requeue_now(conn: &mut PgConnection, row: &OutboxRow) -> Result<(), String> {
    if row.sent_at.is_some() {
        return Err("That message has already been sent. It will not be sent again.".to_string());
    }
    let Some(state) = row.state() else {
        return Err("That message is in a state this instance does not recognise.".to_string());
    };
    if !state.may_be_retried() {
        return Err(
            "That message is being sent right now. Wait for the attempt to finish.".to_string(),
        );
    }
    diesel::update(mail_outbox::table.filter(mail_outbox::id.eq(row.id)))
        .set((
            mail_outbox::state.eq(OutboxState::Queued.as_str()),
            mail_outbox::next_attempt_at.eq(None::<NaiveDateTime>),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to requeue the message: {e}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "outbox_tests.rs"]
mod tests;
