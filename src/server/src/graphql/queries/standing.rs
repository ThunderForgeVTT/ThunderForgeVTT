//! Where an account stands, and what it has been told.
//!
//! Spec 039 US5, `contracts/standing-and-termination.md`.
//!
//! `myStanding` is FR-029 and it is not decorative: FR-028 says nobody may
//! reach the third strike having never been told about the first two, and a
//! page that answers "where do I stand" at any moment is half of keeping that
//! promise. `myNotices` is the other half.

use async_graphql::{Context, Json, Object, SimpleObject};
use uuid::Uuid;

use crate::graphql::helpers::authenticated_user_even_if_disabled;
use crate::graphql::types::ModerationEntityType;
use crate::graphql::{Error, GraphQLResult, admin_user, app_state};
use crate::models::AccountTermination;
use crate::moderation::standing::{Standing, is_due_for_deletion};

/// One strike: what it was, and when it stops counting.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "Strike")]
pub struct GraphQLStrike {
    pub case_id: Uuid,
    pub entity_type: ModerationEntityType,
    pub entity_id: Uuid,
    pub world_id: Uuid,
    pub recorded_at: String,
    /// When this strike stops counting, from the existing lookback.
    pub ages_out_at: String,
}

/// Everything here is derived on read — there is no stored count to go stale.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "Standing")]
pub struct GraphQLStanding {
    /// Oldest first.
    pub strikes: Vec<GraphQLStrike>,
    pub strike_count: i32,
    /// The rungs in force on this instance, so a person can see how many
    /// remain before each consequence.
    pub warn_at: i32,
    pub suspend_publishing_at: i32,
    pub threshold: i32,
    pub warned: bool,
    pub may_publish: bool,
    pub disabled: bool,
    /// The open window, if any (US7).
    pub termination: Option<GraphQLTermination>,
}

/// A termination window, as the person and an administrator see it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "Termination")]
pub struct GraphQLTermination {
    pub opened_at: String,
    pub deletion_due_at: String,
    pub strike_count_at_open: i32,
    /// `"none"` | `"open"` | `"upheld"` | `"rejected"`.
    pub appeal_state: String,
    /// True when this instance requires a person to decide, not a timer.
    pub requires_human: bool,
    /// False only for the instance's last administrator (FR-039): the window
    /// exists, a person must decide, and the account is not disabled.
    pub disables_account: bool,
    pub appeal_statement: Option<String>,
    pub appeal_filed_at: Option<String>,
    pub appeal_resolved_at: Option<String>,
    pub appeal_note: Option<String>,
    /// Past its date with no appeal open — ready for the end the instance is
    /// set to give it.
    pub due: bool,
}

impl From<&AccountTermination> for GraphQLTermination {
    fn from(t: &AccountTermination) -> Self {
        Self {
            opened_at: t.opened_at.to_rfc3339(),
            deletion_due_at: t.deletion_due_at.to_rfc3339(),
            strike_count_at_open: t.strike_count_at_open,
            appeal_state: t.appeal_state.clone(),
            requires_human: t.requires_human,
            disables_account: t.disables_account,
            appeal_statement: t.appeal_statement.clone(),
            appeal_filed_at: t.appeal_filed_at.map(|at| at.to_rfc3339()),
            appeal_resolved_at: t.appeal_resolved_at.map(|at| at.to_rfc3339()),
            appeal_note: t.appeal_note.clone(),
            due: is_due_for_deletion(t, chrono::Utc::now()),
        }
    }
}

/// What a person was told. The words are the client's to render from `kind`
/// and `payload`; they are not stored, so a wording fix reaches every notice.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "AccountNotice")]
pub struct GraphQLAccountNotice {
    pub id: Uuid,
    pub kind: String,
    pub subject_ref: Option<Json<serde_json::Value>>,
    pub payload: Option<Json<serde_json::Value>>,
    pub created_at: String,
    pub read_at: Option<String>,
}

fn to_graphql(standing: Standing) -> GraphQLResult<GraphQLStanding> {
    let ladder = standing.ladder;
    let strike_count = standing.strike_count() as i32;
    let warned = standing.warned();
    let termination = standing.termination.as_ref().map(GraphQLTermination::from);
    let strikes = standing
        .strikes
        .into_iter()
        .map(|strike| {
            // Every strike was written through `ModerationEntityType::as_db_str`,
            // so an unknown value is somebody editing the table by hand — worth
            // an error rather than a strike quietly missing from the list.
            let entity_type = ModerationEntityType::from_db_str(&strike.entity_type)
                .ok_or_else(|| Error::new("A strike names content of an unknown kind"))?;
            Ok(GraphQLStrike {
                case_id: strike.case_id,
                entity_type,
                entity_id: strike.entity_id,
                world_id: strike.world_id,
                recorded_at: strike.recorded_at.to_rfc3339(),
                ages_out_at: ladder.ages_out_at(strike.recorded_at).to_rfc3339(),
            })
        })
        .collect::<GraphQLResult<Vec<_>>>()?;

    Ok(GraphQLStanding {
        strikes,
        strike_count,
        warn_at: ladder.warn_at as i32,
        suspend_publishing_at: ladder.suspend_publishing_at as i32,
        threshold: ladder.threshold as i32,
        warned,
        may_publish: standing.may_publish,
        disabled: standing.disabled,
        termination,
    })
}

/// How many notices a person gets when they do not say.
const MY_NOTICES_DEFAULT: i32 = 50;
const MY_NOTICES_MAX: i32 = 200;

#[derive(Default)]
pub struct StandingQuery;

#[Object]
impl StandingQuery {
    /// The caller's own standing (FR-029). Reachable by a **disabled** account:
    /// the person has to be able to see the window and the date.
    async fn my_standing(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLStanding> {
        let state = app_state(ctx)?;
        let user = authenticated_user_even_if_disabled(ctx)?;
        let standing = crate::moderation::standing::standing_of(state, user.user_id)
            .await
            .map_err(Error::new)?;
        to_graphql(standing)
    }

    /// Anyone's standing. Admin-only, for the reason `moderationCase` is.
    async fn account_standing(
        &self,
        ctx: &Context<'_>,
        account_id: Uuid,
    ) -> GraphQLResult<GraphQLStanding> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let standing = crate::moderation::standing::standing_of(state, account_id)
            .await
            .map_err(Error::new)?;
        to_graphql(standing)
    }

    /// What the caller has been told about their own account, newest first
    /// (FR-028).
    async fn my_notices(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLAccountNotice>> {
        let state = app_state(ctx)?;
        let user = authenticated_user_even_if_disabled(ctx)?;
        let limit = limit.unwrap_or(MY_NOTICES_DEFAULT).clamp(1, MY_NOTICES_MAX);

        let rows = crate::notices::for_account(state, user.user_id, i64::from(limit))
            .await
            .map_err(Error::new)?;
        Ok(rows
            .into_iter()
            .map(|notice| GraphQLAccountNotice {
                id: notice.id,
                kind: notice.kind,
                subject_ref: notice.subject_ref.map(Json),
                payload: notice.payload.map(Json),
                created_at: notice.created_at.to_rfc3339(),
                read_at: notice.read_at.map(|at| at.to_rfc3339()),
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "standing_tests.rs"]
mod standing_tests;
