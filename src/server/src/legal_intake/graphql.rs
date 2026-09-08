//! The intake form's mutation, and the operator's queue.
//!
//! # One unauthenticated mutation, and what guards it
//!
//! `submitLegalEnquiry` is the only write in this product that takes prose
//! from somebody with no account. Three things stand between it and abuse, and
//! none of them is optional:
//!
//! 1. **The rate limiter**, keyed on the caller the public transport derives —
//!    the same limiter and the same identity spec 035 uses for anonymous share
//!    reads, so there is one answer to "who is this" rather than two.
//! 2. **Length ceilings on every field**, because a text column with no
//!    ceiling is a way to fill a disk through a contact form.
//! 3. **Refusals that never echo the submission**, so it cannot be made to
//!    reflect arbitrary text back to whoever called it.
//!
//! # Reading is administrators-only, and says so in one place
//!
//! An enquiry contains a name, an address and whatever the person chose to
//! write, which may be a great deal about themselves in a privacy request.
//! Every read here goes through `admin_user`.

use async_graphql::{Context, Enum, Object, Result as GraphQLResult, SimpleObject};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use super::{EnquiryKind, EnquiryStatus, validate};
use crate::graphql::{admin_user, anonymous::caller_id, app_state, share_rate_limit};
use crate::models::{LegalEnquiry, NewLegalEnquiry};
use crate::schema::legal_enquiries;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "LegalEnquiryKind")]
pub enum GraphQLEnquiryKind {
    Terms,
    Privacy,
}

impl From<GraphQLEnquiryKind> for EnquiryKind {
    fn from(value: GraphQLEnquiryKind) -> Self {
        match value {
            GraphQLEnquiryKind::Terms => EnquiryKind::Terms,
            GraphQLEnquiryKind::Privacy => EnquiryKind::Privacy,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "LegalEnquiryStatus")]
pub enum GraphQLEnquiryStatus {
    Open,
    Acknowledged,
    Closed,
}

impl From<GraphQLEnquiryStatus> for EnquiryStatus {
    fn from(value: GraphQLEnquiryStatus) -> Self {
        match value {
            GraphQLEnquiryStatus::Open => EnquiryStatus::Open,
            GraphQLEnquiryStatus::Acknowledged => EnquiryStatus::Acknowledged,
            GraphQLEnquiryStatus::Closed => EnquiryStatus::Closed,
        }
    }
}

/// One enquiry, as an administrator sees it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "LegalEnquiry")]
pub struct GraphQLLegalEnquiry {
    pub id: Uuid,
    pub kind: String,
    pub submitter_name: String,
    pub submitter_contact: String,
    pub subject: String,
    pub body: String,
    pub status: String,
    pub submitted_by: Option<String>,
    pub created_at: String,
    pub handled_by: Option<String>,
    pub handled_at: Option<String>,
    pub resolution_note: Option<String>,
}

impl From<LegalEnquiry> for GraphQLLegalEnquiry {
    fn from(row: LegalEnquiry) -> Self {
        GraphQLLegalEnquiry {
            id: row.id,
            kind: row.kind,
            submitter_name: row.submitter_name,
            submitter_contact: row.submitter_contact,
            subject: row.subject,
            body: row.body,
            status: row.status,
            submitted_by: row.submitted_by.map(|id| id.to_string()),
            created_at: row.created_at.to_string(),
            handled_by: row.handled_by.map(|id| id.to_string()),
            handled_at: row.handled_at.map(|t| t.to_string()),
            resolution_note: row.resolution_note,
        }
    }
}

/// What the form is told after a submission.
///
/// Deliberately carries no id and no row: the person who filed it has no
/// account and no way to read it back, so handing them an identifier would
/// imply a status page that does not exist. What they get is the acknowledgement
/// and the reassurance that a reply goes to the address they gave.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "LegalEnquiryReceipt")]
pub struct GraphQLEnquiryReceipt {
    pub accepted: bool,
    pub message: String,
}

#[derive(Default)]
pub struct LegalEnquiryQuery;

#[Object]
impl LegalEnquiryQuery {
    /// The intake queue, oldest first, narrowable by kind and status.
    /// Administrators only.
    async fn legal_enquiries(
        &self,
        ctx: &Context<'_>,
        kind: Option<GraphQLEnquiryKind>,
        status: Option<GraphQLEnquiryStatus>,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLLegalEnquiry>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        let mut query = legal_enquiries::table.into_boxed();
        if let Some(kind) = kind {
            query = query.filter(legal_enquiries::kind.eq(EnquiryKind::from(kind).as_db_str()));
        }
        if let Some(status) = status {
            query =
                query.filter(legal_enquiries::status.eq(EnquiryStatus::from(status).as_db_str()));
        }

        // Oldest first: this is a queue worked front to back, not an audit
        // trail read newest first. The person waiting longest is the one to
        // answer next.
        let rows = query
            .order(legal_enquiries::created_at.asc())
            .limit(limit.unwrap_or(100).into())
            .select(LegalEnquiry::as_select())
            .load::<LegalEnquiry>(&mut conn)?;

        Ok(rows.into_iter().map(GraphQLLegalEnquiry::from).collect())
    }

    /// How many are open, per kind, so the admin nav can carry a count without
    /// loading the bodies.
    async fn open_legal_enquiry_counts(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLEnquiryCount>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        let rows: Vec<(String, i64)> = legal_enquiries::table
            .filter(legal_enquiries::status.eq(EnquiryStatus::Open.as_db_str()))
            .group_by(legal_enquiries::kind)
            .select((legal_enquiries::kind, diesel::dsl::count_star()))
            .load(&mut conn)?;

        Ok(rows
            .into_iter()
            .map(|(kind, open)| GraphQLEnquiryCount { kind, open })
            .collect())
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "LegalEnquiryCount")]
pub struct GraphQLEnquiryCount {
    pub kind: String,
    pub open: i64,
}

#[derive(Default)]
pub struct LegalEnquiryMutation;

#[Object]
impl LegalEnquiryMutation {
    /// File a terms dispute or a privacy request. **Deliberately does not
    /// require an account** — see the module header.
    async fn submit_legal_enquiry(
        &self,
        ctx: &Context<'_>,
        kind: GraphQLEnquiryKind,
        submitter_name: String,
        submitter_contact: String,
        subject: String,
        body: String,
    ) -> GraphQLResult<GraphQLEnquiryReceipt> {
        let state = app_state(ctx)?;

        // Before anything is read or written. An unauthenticated write that
        // validates first is a way to probe validation for free.
        if !share_rate_limit::allow_request(&format!("legal-enquiry:{}", caller_id(ctx))) {
            return Err(async_graphql::Error::new(
                share_rate_limit::rate_limited_message(),
            ));
        }

        let problems = validate(&submitter_name, &submitter_contact, &subject, &body);
        if !problems.is_empty() {
            return Err(async_graphql::Error::new(problems.join(" ")));
        }

        // Recorded when the person happens to be signed in, and absent
        // otherwise. Never required, and never used to decide whether the
        // enquiry is accepted.
        let submitted_by = ctx
            .data_opt::<crate::auth_middleware::AuthenticatedUser>()
            .map(|user| user.user_id);

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        diesel::insert_into(legal_enquiries::table)
            .values(NewLegalEnquiry {
                id: Uuid::now_v7(),
                kind: EnquiryKind::from(kind).as_db_str().to_string(),
                submitter_name: submitter_name.trim().to_string(),
                submitter_contact: submitter_contact.trim().to_string(),
                subject: subject.trim().to_string(),
                body: body.trim().to_string(),
                submitted_by,
            })
            .execute(&mut conn)?;

        Ok(GraphQLEnquiryReceipt {
            accepted: true,
            message: format!(
                "This has been recorded and will be answered at {}.",
                submitter_contact.trim()
            ),
        })
    }

    /// Move an enquiry along, with a note. Administrators only.
    ///
    /// One mutation for both transitions rather than `acknowledge` and
    /// `close`: they differ only in the status written, and two mutations
    /// would be two places for the provenance to be forgotten.
    async fn set_legal_enquiry_status(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        status: GraphQLEnquiryStatus,
        resolution_note: Option<String>,
    ) -> GraphQLResult<GraphQLLegalEnquiry> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        let status = EnquiryStatus::from(status);
        // Returning to `open` clears the provenance, because the constraint
        // holds that a handled row has both a handler and a time — and an
        // enquiry reopened by somebody is not one that was resolved.
        let (handled_by, handled_at) = match status {
            EnquiryStatus::Open => (None, None),
            _ => (Some(admin.user_id), Some(Utc::now().naive_utc())),
        };

        let row = diesel::update(legal_enquiries::table.find(id))
            .set((
                legal_enquiries::status.eq(status.as_db_str()),
                legal_enquiries::handled_by.eq(handled_by),
                legal_enquiries::handled_at.eq(handled_at),
                legal_enquiries::resolution_note.eq(resolution_note),
            ))
            .returning(LegalEnquiry::as_returning())
            .get_result::<LegalEnquiry>(&mut conn)
            .optional()?
            .ok_or_else(|| async_graphql::Error::new("There is no such enquiry."))?;

        Ok(GraphQLLegalEnquiry::from(row))
    }
}
