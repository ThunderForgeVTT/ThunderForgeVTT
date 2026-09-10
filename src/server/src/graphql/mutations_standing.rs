//! The appeal, and the two decisions only an administrator makes.
//!
//! Spec 039 US7, `contracts/standing-and-termination.md`.
//!
//! # What is deliberately absent
//!
//! No mutation that sets a strike count, and no "disable this account" button.
//! Standing is derived; an administrator changes it by resolving a case, which
//! is the surface that already exists, and disablement is a consequence of the
//! counting. A button would be a second way to reach the same state with
//! different evidence behind it.

use async_graphql::{Context, Object};
use chrono::Utc;
use uuid::Uuid;

use crate::graphql::helpers::authenticated_user_even_if_disabled;
use crate::graphql::queries::standing::GraphQLTermination;
use crate::graphql::{Error, GraphQLResult, admin_user, app_state};
use crate::moderation::standing::{self, AppealDecision, Ladder};

/// Long enough to make a case; short enough that it is a statement, not a
/// document dump.
const APPEAL_MAX_CHARS: usize = 5_000;

#[derive(Default)]
pub struct StandingMutation;

#[Object]
impl StandingMutation {
    /// File an appeal against a disablement (FR-031). One per decision.
    ///
    /// **Callable by a disabled account** — it is one of the two remedies.
    /// Filing it pauses the deletion if it is still open when the window ends
    /// (FR-034), and writes nothing to the download (FR-032).
    async fn file_appeal(
        &self,
        ctx: &Context<'_>,
        statement: String,
    ) -> GraphQLResult<GraphQLTermination> {
        let state = app_state(ctx)?;
        let user = authenticated_user_even_if_disabled(ctx)?;
        let statement = statement.trim().to_string();
        if statement.is_empty() {
            return Err(Error::new(
                "An appeal needs a statement: say why the decision is wrong.",
            ));
        }
        if statement.chars().count() > APPEAL_MAX_CHARS {
            return Err(Error::new(format!(
                "An appeal is at most {APPEAL_MAX_CHARS} characters."
            )));
        }

        let account_id = user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let filed = tokio::task::spawn_blocking(move || {
            standing::file_appeal_sync(&mut conn, account_id, &statement, Utc::now())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;
        Ok(GraphQLTermination::from(&filed))
    }

    /// Decide an appeal. Admin only.
    ///
    /// `overturnedCaseId` names the strike the appeal overturned; omitted, it
    /// is the most recent — the one that crossed the threshold.
    async fn resolve_appeal(
        &self,
        ctx: &Context<'_>,
        account_id: Uuid,
        upheld: bool,
        note: Option<String>,
        overturned_case_id: Option<Uuid>,
    ) -> GraphQLResult<GraphQLTermination> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let resolved_by = admin.user_id;
        let decision = AppealDecision {
            upheld,
            note: note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
            overturned_case: overturned_case_id,
        };

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let decided = tokio::task::spawn_blocking(move || {
            standing::resolve_appeal_sync(
                &mut conn,
                account_id,
                resolved_by,
                &decision,
                Ladder::from_env(),
                Utc::now(),
            )
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;
        Ok(GraphQLTermination::from(&decided))
    }

    /// Carry out a window that waits for a person (FR-040's default). Admin
    /// only; refused before the date, while an appeal is open, and for the
    /// instance's last administrator.
    async fn execute_termination(
        &self,
        ctx: &Context<'_>,
        account_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            standing::execute_by_administrator_sync(&mut conn, account_id, Utc::now())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;
        Ok(true)
    }
}
