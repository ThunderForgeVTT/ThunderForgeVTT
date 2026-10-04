//! A changed operator statement is shown to an administrator and acknowledged
//! again, rather than applied to them silently.
//!
//! Spec 039 T085, FR-044, `contracts/operator-acknowledgement.md`.
//!
//! First-run setup records the acknowledgement of the words that shipped
//! then. An upgrade that changes them mints a new version (the identity is the
//! hash of the words, ADR-076), and this is how an instance notices: the most
//! recent acknowledgement is of a version this build no longer ships. The
//! administrator's view says so, shows the words, and asks — one more
//! attestation, same table, same rules.

use async_graphql::{Context, Object, SimpleObject};

use crate::auth::operator_acknowledgement::{ACKNOWLEDGEMENT_REFUSED, latest_sync, record_sync};
use crate::graphql::queries::legal::{GraphQLAttestation, with_archived_terms};
use crate::graphql::{Error, GraphQLResult, admin_user, app_state};
use crate::publishing::AttestationInput;
use crate::state::AppState;

/// Where this instance's acknowledgement stands against the words it ships.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "OperatorAcknowledgementState")]
pub struct GraphQLOperatorAcknowledgementState {
    /// The most recent acknowledgement, of any version, in the words it was
    /// made to. Null only on an instance whose setup predates acknowledgement.
    pub acknowledgement: Option<GraphQLAttestation>,
    /// The operator statement this build ships — what to acknowledge now.
    pub current_version_id: String,
    /// False after an upgrade that changed the words, until acknowledged.
    pub is_current: bool,
}

async fn state_of(state: &AppState) -> GraphQLResult<GraphQLOperatorAcknowledgementState> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let latest = tokio::task::spawn_blocking(move || latest_sync(&mut conn))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to read the operator acknowledgement"))?;

    let current_version_id = crate::legal::operator_statement().version_id;
    let is_current = latest
        .as_ref()
        .is_some_and(|record| record.terms_version_id == current_version_id);
    let acknowledgement = with_archived_terms(state, latest.into_iter().collect())
        .await?
        .into_iter()
        .next();

    Ok(GraphQLOperatorAcknowledgementState {
        acknowledgement,
        current_version_id,
        is_current,
    })
}

#[derive(Default)]
pub struct OperatorAcknowledgementQuery;

#[Object]
impl OperatorAcknowledgementQuery {
    /// The acknowledgement this instance holds, and whether it is of the words
    /// this build ships. Admin only.
    async fn instance_operator_acknowledgement(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLOperatorAcknowledgementState> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        state_of(state).await
    }
}

#[derive(Default)]
pub struct OperatorAcknowledgementMutation;

#[Object]
impl OperatorAcknowledgementMutation {
    /// Acknowledge the operator statement this build ships (FR-044). Admin
    /// only.
    ///
    /// Only the **current** version is accepted — acknowledging words that are
    /// no longer in force would be a record of nothing. Acknowledging the
    /// current version twice is not an error; the instance already holds it.
    async fn acknowledge_operator_statement(
        &self,
        ctx: &Context<'_>,
        attestation: AttestationInput,
    ) -> GraphQLResult<GraphQLOperatorAcknowledgementState> {
        let state = app_state(ctx)?;
        let administrator = admin_user(ctx)?.user_id;
        let offered = attestation.terms_version_id.trim().to_string();
        if offered != crate::legal::operator_statement().version_id {
            return Err(Error::new(ACKNOWLEDGEMENT_REFUSED));
        }

        if !state_of(state).await?.is_current {
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;
            tokio::task::spawn_blocking(move || record_sync(&mut conn, administrator, &offered))
                .await
                .map_err(|_| Error::new("Failed to spawn blocking task"))?
                .map_err(|_| Error::new("Failed to record the acknowledgement"))?;
        }
        state_of(state).await
    }
}
