//! `genieWishLog` — what the table asked for with its wishes.
//!
//! Its own file rather than another block in `queries.rs`, which is already
//! at the repo's 1000-line ceiling: the wish log is a self-contained read,
//! with its own type, its own visibility rule and its own tests.

use async_graphql::{Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use super::schema::world_genie_sessions;
use thunderforge_server::auth::world_membership::require_world_member;
use thunderforge_server::state::AppState;

// ============================================================================
// genieWishLog — what the table asked for, and got (FR-014)
// ============================================================================

/// One wish that was spent, and the Wish Effect the Game Master narrated.
#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGenieWishEntry {
    /// The audit row's id, as a string — `world_events.id` is a bigint and
    /// this is only ever a React key and a dedupe handle.
    pub id: String,
    pub session_id: Uuid,
    /// What was asked for, in the Game Master's own words (FR-014).
    pub narrative_effect: String,
    /// What the pool stood at *after* this wish — so a reader can see the
    /// three wishes counting down without recomputing them.
    pub wishes_remaining: i32,
    pub spent_by: Uuid,
    /// The Game Master's display name, so the list reads as people rather
    /// than ids. `None` if the account is gone.
    pub spent_by_name: Option<String>,
    pub spent_at: chrono::NaiveDateTime,
}

/// Testable core of `GenieSessionQuery::genie_wish_log`.
///
/// # Why this reads the audit trail rather than a table of its own
///
/// `spendWish` has always written the Wish Effect — into the `world_events`
/// row it records for the live nudge (`mutations/clocks.rs`'s `spend_wish_impl`,
/// `"narrative_effect": narrative_effect`). What was missing was any way to
/// read it back: the mutation returns the session, the session carries only
/// `wishesRemaining`, and so the one thing FR-014 is *about* — the narrated
/// effect the Game Master typed — was written down and then never shown to
/// anybody. The owner's report, 2026-09-15: a wish is spent and the list of
/// what was asked for is empty.
///
/// `world_events` is the audit trail, not a cache: rows are kept for the life
/// of the world (they are deleted only with their author's account,
/// `src/server/src/users/mod.rs`). So the history already exists, correctly,
/// and a new table would be a second copy of it. This query is the read that
/// was missing, not a new place to write.
///
/// Any world member may read it, for the same reason `genie_session_impl`
/// lets them read the pool: FR-013 makes the Wish Pool shared state, visible
/// to every connected player and the GM. A wish is spent by group agreement;
/// a list of them the party cannot see would be a strange thing to agree to.
pub async fn genie_wish_log_impl(
    state: &AppState,
    user_id: Uuid,
    session_id: Uuid,
) -> GraphQLResult<Vec<GraphQLGenieWishEntry>> {
    use diesel::dsl::sql;
    use diesel::sql_types::Bool;
    use thunderforge_server::schema::{users, world_events};
    use thunderforge_server::world_events::EVENT_CODE_GENIE_SESSION_STATE;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || -> Result<Vec<GraphQLGenieWishEntry>, String> {
        let world_id = world_genie_sessions::table
            .filter(world_genie_sessions::id.eq(session_id))
            .select(world_genie_sessions::world_id)
            .first::<Uuid>(&mut conn)
            .map_err(|_| "Genie session not found".to_string())?;

        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You must be a member of this world".to_string())?;

        // Narrowed in SQL to this world's wish-spend events; the session is
        // matched in Rust below, because the payload is a free-form jsonb
        // column and binding a uuid into a `->>` comparison buys nothing
        // over comparing the handful of rows this returns.
        let rows = world_events::table
            .filter(world_events::world_id.eq(world_id))
            .filter(world_events::event_code.eq(EVENT_CODE_GENIE_SESSION_STATE))
            .filter(sql::<Bool>(
                "token_event->>'kind' = 'wish_pool' AND token_event->>'action' = 'wish_spent'",
            ))
            .order(world_events::created_at.asc())
            .select((
                world_events::id,
                world_events::token_event,
                world_events::created_by,
                world_events::created_at,
            ))
            .load::<(i64, Option<serde_json::Value>, Uuid, chrono::NaiveDateTime)>(&mut conn)
            .map_err(|e| format!("Failed to load the wish log: {e}"))?;

        let mut entries = Vec::new();
        for (id, payload, created_by, created_at) in rows {
            let Some(payload) = payload else { continue };
            // A payload whose session is not this one, or that predates the
            // field, is skipped rather than rendered blank: an entry that
            // says nothing about what was asked for is worse than no entry.
            if payload.get("session_id").and_then(|v| v.as_str())
                != Some(session_id.to_string().as_str())
            {
                continue;
            }
            let Some(narrative_effect) = payload.get("narrative_effect").and_then(|v| v.as_str())
            else {
                continue;
            };

            let spent_by_name: Option<String> = users::table
                .filter(users::id.eq(created_by))
                .select(users::username)
                .first(&mut conn)
                .optional()
                .ok()
                .flatten();

            entries.push(GraphQLGenieWishEntry {
                id: id.to_string(),
                session_id,
                narrative_effect: narrative_effect.to_string(),
                wishes_remaining: payload
                    .get("wishes_remaining")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32,
                spent_by: created_by,
                spent_by_name,
                spent_at: created_at,
            });
        }

        Ok(entries)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::mutations::{
        advance_doom_clock_impl, spend_wish_impl, start_genie_session_impl, StartGenieSessionInput,
    };
    use thunderforge_server::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// The defect, stated as a test (owner, 2026-09-15): a wish was spent
    /// and the list of what was asked for showed nothing. `spendWish` wrote
    /// the Wish Effect into the audit trail all along; nothing read it back.
    #[tokio::test]
    async fn a_spent_wish_says_what_was_asked_for() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 6,
            },
        )
        .await
        .unwrap();

        assert!(
            genie_wish_log_impl(&state, owner_id, session.id)
                .await
                .unwrap()
                .is_empty(),
            "a fresh session has asked for nothing yet"
        );

        spend_wish_impl(
            &state,
            owner_id,
            false,
            session.id,
            "Undo the collapse of the stair".to_string(),
        )
        .await
        .unwrap();
        spend_wish_impl(
            &state,
            owner_id,
            false,
            session.id,
            "Reveal the sigil under the ash".to_string(),
        )
        .await
        .unwrap();

        let log = genie_wish_log_impl(&state, owner_id, session.id)
            .await
            .unwrap();
        assert_eq!(log.len(), 2, "both wishes must be listed");
        assert_eq!(
            log.iter()
                .map(|entry| entry.narrative_effect.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Undo the collapse of the stair",
                "Reveal the sigil under the ash"
            ],
            "oldest first — the table reads its wishes in the order it spent them"
        );
        assert_eq!(
            log.iter()
                .map(|entry| entry.wishes_remaining)
                .collect::<Vec<_>>(),
            vec![2, 1],
            "each entry carries the pool as it stood after that wish"
        );
        assert_eq!(log[0].spent_by, owner_id);
        assert_eq!(log[0].session_id, session.id);
    }

    /// FR-013 makes the Wish Pool shared state. A player who is part of the
    /// group agreement to spend a wish must be able to read what it bought.
    #[tokio::test]
    async fn the_wish_log_is_readable_by_a_player_and_refused_to_an_outsider() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let stranger_id = insert_test_user(&mut conn);
        drop(conn);

        let session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 6,
            },
        )
        .await
        .unwrap();
        spend_wish_impl(
            &state,
            owner_id,
            false,
            session.id,
            "Turn the sandstorm aside".to_string(),
        )
        .await
        .unwrap();

        let as_player = genie_wish_log_impl(&state, player_id, session.id)
            .await
            .unwrap();
        assert_eq!(as_player.len(), 1);
        assert_eq!(as_player[0].narrative_effect, "Turn the sandstorm aside");

        assert!(
            genie_wish_log_impl(&state, stranger_id, session.id)
                .await
                .is_err(),
            "someone who is not in the world must not read its wishes"
        );
    }

    /// One world, two sessions across two game nights: the second night's
    /// log must not inherit the first night's wishes. The audit trail is
    /// world-scoped, so this is exactly the filter that could go wrong.
    #[tokio::test]
    async fn a_new_session_starts_with_an_empty_wish_log() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let first = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 4,
            },
        )
        .await
        .unwrap();
        spend_wish_impl(
            &state,
            owner_id,
            false,
            first.id,
            "Last night's wish".to_string(),
        )
        .await
        .unwrap();

        // Fill the Doom Clock to end the night, then start the next one.
        advance_doom_clock_impl(&state, owner_id, false, first.id, 4)
            .await
            .unwrap();
        let second = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 4,
            },
        )
        .await
        .unwrap();

        assert!(
            genie_wish_log_impl(&state, owner_id, second.id)
                .await
                .unwrap()
                .is_empty(),
            "a new session must not show the previous session's wishes"
        );
        assert_eq!(
            genie_wish_log_impl(&state, owner_id, first.id)
                .await
                .unwrap()
                .len(),
            1,
            "and the previous session must keep its own"
        );
    }
}
