//! Spec 048 FR-038, FR-038a, FR-038b: a refused use of content the world has
//! not adopted is reported to the Game Master, as facts, and never when the
//! client was only stale.
//!
//! The guard refuses (`guard.rs`), and the refusal carries the piece and the
//! character in its extensions. Each resolver hands its result to
//! [`on_refusal`], which reads them back and records the attempt.
//!
//! - **Stale** (FR-038b): the client sends `x-tf-last-event`, the newest world
//!   event it has applied. If the piece has a decision (event 41) newer than
//!   that, or the header is missing, the client may just not have caught up:
//!   nothing is stored, and the suppression is logged on the
//!   `thunderforge.unadopted_use_attempts` target. A piece with no decision
//!   at all was never delivered to anyone, so no client is stale about it,
//!   and it is always reported.
//! - **Window**: one report per (character, piece) per ten minutes. A later
//!   attempt in the window is counted on the row and posts nothing.
//! - **The report** is a GM-only chat message that says what happened and not
//!   why (FR-038a).

use async_graphql::{Context, Error, Result as GraphQLResult, Value as GqlValue};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::guard::NOT_ADOPTED_CODE;
use super::{NewUnadoptedUseAttempt, StagedContent};
use crate::models::NewChatMessage;
use crate::schema::{
    users, world_actors, world_chat_messages, world_events, world_staged_content,
    world_unadopted_use_attempts as attempts,
};
use crate::state::AppState;
use crate::world_events::{
    EVENT_CODE_CHAT_MESSAGE, EVENT_CODE_STAGED_CONTENT_DECIDED, record_world_event,
};

/// The header the web client sends with every GraphQL request.
pub const LAST_EVENT_HEADER: &str = "x-tf-last-event";

/// The newest world event the caller's client has applied, from
/// `x-tf-last-event`. Put into the request's data by the HTTP handler.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LastEvent(pub Option<i64>);

impl LastEvent {
    /// Read from the header's value; anything unreadable counts as missing.
    pub fn from_header(value: Option<&str>) -> Self {
        Self(
            value
                .and_then(|v| v.trim().parse::<i64>().ok())
                .filter(|id| *id >= 0),
        )
    }
}

/// How long one report covers.
pub const WINDOW_MINUTES: i64 = 10;

/// The telemetry target the outcomes are logged on.
const TARGET: &str = "thunderforge.unadopted_use_attempts";

/// What became of an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The first in its window: recorded and posted to the GM.
    Reported { chat_message_id: Uuid },
    /// Within a window already reported: counted only.
    Counted,
    /// The client may only have been stale: nothing stored.
    SuppressedStale,
}

/// The facts, in the order FR-038a names them.
pub fn report_sentence(player: &str, piece: &str, character: &str, at: NaiveDateTime) -> String {
    format!(
        "{player} tried to use {piece} on {character} at {}. It came in with the character and has not been adopted.",
        at.format("%Y-%m-%d %H:%M UTC")
    )
}

/// The newest decision on a piece, if it has one.
fn latest_decision(conn: &mut PgConnection, piece: &StagedContent) -> QueryResult<Option<i64>> {
    world_events::table
        .filter(world_events::world_id.eq(piece.world_id))
        .filter(world_events::event_code.eq(EVENT_CODE_STAGED_CONTENT_DECIDED))
        .filter(
            diesel::dsl::sql::<diesel::sql_types::Bool>("token_event->>'stagedId' = ")
                .bind::<diesel::sql_types::Text, _>(piece.id.to_string()),
        )
        .select(diesel::dsl::max(world_events::id))
        .first::<Option<i64>>(conn)
}

/// Record one refused attempt by `user_id` to use `staged_id` on `actor_id`.
pub fn record_attempt(
    conn: &mut PgConnection,
    user_id: Uuid,
    staged_id: Uuid,
    actor_id: Option<Uuid>,
    operation: &str,
    last_event: LastEvent,
    now: NaiveDateTime,
) -> QueryResult<Option<Outcome>> {
    let piece = world_staged_content::table
        .find(staged_id)
        .select(StagedContent::as_select())
        .first::<StagedContent>(conn)?;
    // A share names no character; the one it came in with stands in.
    let Some(actor_id) = actor_id.or(piece.first_actor_id) else {
        return Ok(None);
    };

    if let Some(decided) = latest_decision(conn, &piece)?
        && last_event.0.is_none_or(|seen| seen < decided)
    {
        tracing::info!(target: TARGET, result = "suppressed_stale", operation, %staged_id);
        return Ok(Some(Outcome::SuppressedStale));
    }

    conn.transaction(|conn| {
        let since = now - Duration::minutes(WINDOW_MINUTES);
        let open = attempts::table
            .filter(attempts::actor_id.eq(actor_id))
            .filter(attempts::staged_id.eq(staged_id))
            .filter(attempts::first_at.gt(since))
            .order(attempts::first_at.desc())
            .select(attempts::id)
            .for_update()
            .first::<Uuid>(conn)
            .optional()?;
        if let Some(id) = open {
            diesel::update(attempts::table.find(id))
                .set((
                    attempts::attempts.eq(attempts::attempts + 1),
                    attempts::last_at.eq(now),
                    attempts::updated_by.eq(user_id),
                ))
                .execute(conn)?;
            tracing::info!(target: TARGET, result = "counted", operation, %staged_id);
            return Ok(Some(Outcome::Counted));
        }

        let player = users::table
            .find(user_id)
            .select(users::username)
            .first::<String>(conn)?;
        let character = world_actors::table
            .find(actor_id)
            .select(world_actors::label)
            .first::<String>(conn)?;
        let message = NewChatMessage {
            id: Uuid::now_v7(),
            world_id: piece.world_id,
            scene_id: None,
            author_user_id: user_id,
            author_label: "ThunderForge".into(),
            body: report_sentence(&player, &piece.name, &character, now),
            gm_only: true,
        };
        diesel::insert_into(world_chat_messages::table)
            .values(&message)
            .execute(conn)?;
        diesel::insert_into(attempts::table)
            .values(NewUnadoptedUseAttempt {
                world_id: piece.world_id,
                actor_id,
                staged_id,
                user_id,
                operation,
                reported: true,
                chat_message_id: Some(message.id),
                created_by: user_id,
                updated_by: user_id,
            })
            .execute(conn)?;
        // Id only, as every chat message: the body never rides the bus.
        let _ = record_world_event(
            conn,
            piece.world_id,
            EVENT_CODE_CHAT_MESSAGE,
            Some(serde_json::json!({ "messageId": message.id })),
            user_id,
        );
        tracing::info!(target: TARGET, result = "reported", operation, %staged_id);
        Ok(Some(Outcome::Reported {
            chat_message_id: message.id,
        }))
    })
}

fn uuid_extension(error: &Error, key: &str) -> Option<Uuid> {
    match error.extensions.as_ref()?.get(key)? {
        GqlValue::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn is_not_adopted(error: &Error) -> bool {
    matches!(
        error.extensions.as_ref().and_then(|ext| ext.get("code")),
        Some(GqlValue::String(code)) if code == NOT_ADOPTED_CODE
    )
}

/// Report a `CONTENT_NOT_ADOPTED` refusal, then return the result as it was.
/// Reporting never changes what the caller is told: a failure to record is
/// logged, and the refusal stands.
pub async fn on_refusal<T>(
    ctx: &Context<'_>,
    state: &AppState,
    user_id: Uuid,
    operation: &'static str,
    result: GraphQLResult<T>,
) -> GraphQLResult<T> {
    let Err(error) = &result else {
        return result;
    };
    if !is_not_adopted(error) {
        return result;
    }
    let Some(staged_id) = uuid_extension(error, "stagedId") else {
        return result;
    };
    let actor_id = uuid_extension(error, "actorId");
    let last_event = ctx.data_opt::<LastEvent>().copied().unwrap_or_default();
    let Ok(mut conn) = state.db_pool.get() else {
        tracing::warn!(target: TARGET, "no connection to record an attempt");
        return result;
    };
    let recorded = tokio::task::spawn_blocking(move || {
        record_attempt(
            &mut conn,
            user_id,
            staged_id,
            actor_id,
            operation,
            last_event,
            Utc::now().naive_utc(),
        )
    })
    .await;
    if !matches!(recorded, Ok(Ok(_))) {
        tracing::warn!(target: TARGET, operation, %staged_id, "failed to record an attempt");
    }
    result
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
