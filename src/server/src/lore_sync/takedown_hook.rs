//! What a takedown does to a world's mirror.
//!
//! # Why this is a separate module rather than lines inside the takedown
//!
//! Because of FR-040d, which is the whole shape of it: **a failure here must
//! not block or reverse the takedown.** The content has been disabled; that
//! part is done and must stay done whether or not a repository is reachable,
//! whether or not the grant still works, and whether or not the host is
//! having a bad day.
//!
//! Written as a function that returns a report and never an error, so there is
//! nothing for the takedown path to `?` on. A caller that cannot fail is a
//! caller that cannot accidentally undo the thing it was called after.
//!
//! # What it actually does
//!
//! Two things, both bounded:
//!
//! 1. **Stops the content being carried outward.** Nothing is needed for this
//!    — the next pass plans from the world, and `moderation::filter_visible`
//!    already excludes a disabled entry (FR-015). It is listed because "we did
//!    nothing and that was correct" is worth writing down where someone will
//!    look for the code that does something.
//! 2. **Records a public withdrawal**, where the repository is public
//!    (FR-040b) — and records that it deliberately did not, where it is
//!    private (FR-040c).

use chrono::NaiveDate;
use diesel::prelude::*;
use uuid::Uuid;

use crate::lore_sync::disassociate::{self, Outcome};
use crate::models::LoreRepositoryConnection;
use crate::schema::{lore_repository_connections, worlds};

/// What happened to a world's mirror when its content was disabled.
#[derive(Debug)]
pub enum MirrorResponse {
    /// The world has no connection. The common case, and not a problem.
    NotConnected,
    /// A withdrawal was attempted, and this is how it went.
    Attempted(Outcome),
    /// The connection exists but could not be read. Recorded rather than
    /// swallowed: an administrator needs to know an obligation went
    /// unattempted, and a takedown that silently skipped one is worse than a
    /// takedown that says it could not.
    CouldNotRead(String),
}

/// Respond to a takedown on behalf of a world's repository mirror.
///
/// Never returns an error, by design (FR-040d). Every failure is a value the
/// caller may record and ignore.
pub async fn on_content_disabled(
    state: &crate::AppState,
    world_id: Uuid,
    moderation_action_id: Uuid,
    disabled_on: NaiveDate,
) -> MirrorResponse {
    let Ok(mut conn) = state.db_pool.get() else {
        return MirrorResponse::CouldNotRead("no database connection was available".to_string());
    };

    let loaded = lore_repository_connections::table
        .filter(lore_repository_connections::world_id.eq(world_id))
        .select(LoreRepositoryConnection::as_select())
        .first::<LoreRepositoryConnection>(&mut conn)
        .optional();

    let connection = match loaded {
        Ok(Some(c)) => c,
        Ok(None) => return MirrorResponse::NotConnected,
        Err(e) => return MirrorResponse::CouldNotRead(e.to_string()),
    };

    // The world's own name, because FR-036j and the disassociation body both
    // want something its owner chose rather than anything derived from what
    // the world holds.
    let world_name = worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::name)
        .first::<String>(&mut conn)
        .unwrap_or_else(|_| "a ThunderForge world".to_string());

    let outcome = disassociate::disassociate_after_takedown(
        &mut conn,
        &connection,
        moderation_action_id,
        &world_name,
        disabled_on,
    )
    .await;

    MirrorResponse::Attempted(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    /// The common case. A world with no repository is not a failure, and must
    /// not be reported as one — a takedown log full of "no connection" would
    /// bury the entries that need a human.
    #[tokio::test]
    async fn a_world_with_no_connection_is_not_a_problem() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);

        let response = on_content_disabled(
            &state,
            world,
            Uuid::now_v7(),
            chrono::Utc::now().date_naive(),
        )
        .await;

        assert!(
            matches!(response, MirrorResponse::NotConnected),
            "{response:?}"
        );
    }

    /// **FR-040d.** The obligation is to attempt and to say plainly when the
    /// attempt failed — never to fail the takedown. This asserts the shape
    /// that makes that structurally true: there is nothing here a caller can
    /// propagate.
    #[tokio::test]
    async fn a_connected_world_reports_rather_than_erroring() {
        use crate::schema::lore_repository_connections as c;
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);

        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(c::table)
            .values(LoreRepositoryConnection {
                id: Uuid::now_v7(),
                world_id: world,
                host_kind: "test".into(),
                installation_ref: "0".into(),
                repository_ref: format!("owner/{}", Uuid::now_v7()),
                branch: "main".into(),
                directory: "lore".into(),
                incoming_enabled: false,
                notice_acknowledged_at: Some(now),
                state: "working".into(),
                state_reason: None,
                // Private, so nothing is lodged and nothing is attempted over
                // the network — the assertion is about the shape, not the host.
                repository_is_public: Some(false),
                visibility_checked_at: Some(now),
                deactivated_at: None,
                deactivated_reason: None,
                last_synced_at: None,
                last_written_commit: None,
                created_by: owner,
                updated_by: owner,
                created_at: now,
                updated_at: now,
            })
            .execute(&mut conn)
            .expect("insert connection");

        let response = on_content_disabled(
            &state,
            world,
            Uuid::now_v7(),
            chrono::Utc::now().date_naive(),
        )
        .await;

        match response {
            MirrorResponse::Attempted(Outcome::SkippedPrivate) => {}
            other => panic!("expected a recorded skip on a private repository, got {other:?}"),
        }
    }
}
