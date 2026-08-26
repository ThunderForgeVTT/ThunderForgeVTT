//! Play-view Chat: world-scoped messages, persisted in
//! `world_chat_messages` and broadcast on the existing `world_events` bus.
//!
//! No new transport. The alternative considered was a dedicated
//! message-passing server (e.g. `message-io`) alongside axum; that would
//! have meant a second socket with its own auth story, no world-membership
//! authorization, and no persistence — and chat needs a history table
//! either way. `world_events` + `pg_notify` already fans out to the
//! `worldEventsCreated(worldId)` subscription every world member holds
//! open, and already carries walls, lights, tokens, scene launches and
//! Genie session state. Chat is one more event code on it
//! (`EVENT_CODE_CHAT_MESSAGE`), exactly as spec 018 did for Genie state.
//!
//! Authorization:
//! - Sending and reading both require `require_world_member` — chat is
//!   world-private, and a non-member gets nothing.
//! - `gm_only` messages are filtered out **server-side** in
//!   `world_chat_messages_impl` for non-GM callers. The client never
//!   receives them and so cannot leak them by forgetting to hide them.
//! - The broadcast payload deliberately carries only `{ "messageId" }`,
//!   never the body. Every member's subscription sees every event on the
//!   world channel, so putting a GM-only body in the payload would hand it
//!   to exactly the clients it is hidden from. Clients refetch instead,
//!   and the refetch re-applies the `gm_only` filter.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{is_dm_of_world, require_world_member};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ChatMessage, NewChatMessage};
use crate::schema::{users, world_chat_messages};
use crate::state::AppState;
use crate::world_events::{record_world_event, EVENT_CODE_CHAT_MESSAGE};

/// Upper bound on one message. Long enough for a paragraph of narration,
/// short enough that the backscroll query stays cheap and a single client
/// cannot wedge the world's event stream with a megabyte of text.
const MAX_BODY_CHARS: usize = 4000;

/// Default and maximum backscroll depth. The Chat panel renders the tail
/// of the log, not the whole history.
const DEFAULT_HISTORY_LIMIT: i64 = 100;
const MAX_HISTORY_LIMIT: i64 = 500;

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLChatMessage {
    pub id: Uuid,
    pub world_id: Uuid,
    pub scene_id: Option<Uuid>,
    pub author_user_id: Uuid,
    pub author_label: String,
    pub body: String,
    pub gm_only: bool,
    pub created_at: chrono::NaiveDateTime,
}

impl From<ChatMessage> for GraphQLChatMessage {
    fn from(row: ChatMessage) -> Self {
        GraphQLChatMessage {
            id: row.id,
            world_id: row.world_id,
            scene_id: row.scene_id,
            author_user_id: row.author_user_id,
            author_label: row.author_label,
            body: row.body,
            gm_only: row.gm_only,
            created_at: row.created_at,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct SendChatMessageInput {
    pub world_id: Uuid,
    pub scene_id: Option<Uuid>,
    pub body: String,
    /// GM-only whisper. Requested by the client but *verified* server-side:
    /// a non-GM asking for `gmOnly: true` is rejected rather than silently
    /// downgraded, so a confused client never quietly posts to everyone
    /// something the sender believed was private.
    pub gm_only: Option<bool>,
}

/// Trims and length-checks a message body.
///
/// Returns the trimmed body, or an error describing why it is unusable.
/// Split out from the mutation so the rules are unit-testable without a
/// database.
fn validate_body(body: &str) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("Message cannot be empty".to_string());
    }
    // Counted in `chars`, not bytes: a message of emoji or non-Latin script
    // would otherwise be rejected at a quarter of the visible length.
    if trimmed.chars().count() > MAX_BODY_CHARS {
        return Err(format!("Message cannot exceed {MAX_BODY_CHARS} characters"));
    }
    Ok(trimmed.to_string())
}

/// Clamps a caller-supplied history limit into the allowed range.
fn resolve_history_limit(requested: Option<i32>) -> i64 {
    match requested {
        None => DEFAULT_HISTORY_LIMIT,
        Some(n) if n <= 0 => DEFAULT_HISTORY_LIMIT,
        Some(n) => (n as i64).min(MAX_HISTORY_LIMIT),
    }
}

pub async fn send_chat_message_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: SendChatMessageInput,
) -> GraphQLResult<GraphQLChatMessage> {
    let body = validate_body(&input.body).map_err(Error::new)?;
    let world_id = input.world_id;
    let scene_id = input.scene_id;
    let wants_gm_only = input.gm_only.unwrap_or(false);

    if wants_gm_only && !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the GM may send a GM-only message"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let message = tokio::task::spawn_blocking(move || -> Result<ChatMessage, String> {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You are not a member of this world".to_string())?;

        // Captured once, at send time — `author_label` is denormalized so
        // history keeps reading correctly after a rename.
        let author_label = users::table
            .filter(users::id.eq(user_id))
            .select(users::username)
            .first::<String>(&mut conn)
            .map_err(|e| format!("Failed to load author: {e}"))?;

        let new_message = NewChatMessage {
            id: Uuid::now_v7(),
            world_id,
            scene_id,
            author_user_id: user_id,
            author_label,
            body,
            gm_only: wants_gm_only,
        };

        let message = diesel::insert_into(world_chat_messages::table)
            .values(&new_message)
            .returning(ChatMessage::as_returning())
            .get_result::<ChatMessage>(&mut conn)
            .map_err(|e| format!("Failed to send message: {e}"))?;

        // Id only — see this module's doc comment on why the body never
        // rides the bus.
        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_CHAT_MESSAGE,
            Some(serde_json::json!({ "messageId": message.id })),
            user_id,
        );

        Ok(message)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(message.into())
}

pub async fn world_chat_messages_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    limit: Option<i32>,
) -> GraphQLResult<Vec<GraphQLChatMessage>> {
    let is_gm = is_dm_of_world(state, user_id, is_admin, world_id).await?;
    let limit = resolve_history_limit(limit);

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let messages = tokio::task::spawn_blocking(move || -> Result<Vec<ChatMessage>, String> {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You are not a member of this world".to_string())?;

        // Newest-first with a LIMIT is what the index is for; the caller
        // reverses into reading order below. Selecting oldest-first would
        // mean scanning the whole world's history to find the tail.
        let mut query = world_chat_messages::table
            .filter(world_chat_messages::world_id.eq(world_id))
            .into_boxed();

        if !is_gm {
            query = query.filter(world_chat_messages::gm_only.eq(false));
        }

        query
            .order(world_chat_messages::created_at.desc())
            .limit(limit)
            .select(ChatMessage::as_select())
            .load::<ChatMessage>(&mut conn)
            .map_err(|e| format!("Failed to load messages: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    // Oldest-first for rendering.
    Ok(messages
        .into_iter()
        .rev()
        .map(GraphQLChatMessage::from)
        .collect())
}

#[derive(Default)]
pub struct ChatMutation;

#[async_graphql::Object]
impl ChatMutation {
    async fn send_chat_message(
        &self,
        ctx: &Context<'_>,
        input: SendChatMessageInput,
    ) -> GraphQLResult<GraphQLChatMessage> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        send_chat_message_impl(state, user.user_id, user.is_admin, input).await
    }
}

#[derive(Default)]
pub struct ChatQuery;

#[async_graphql::Object]
impl ChatQuery {
    /// This world's chat backscroll, oldest-first. GM-only messages are
    /// omitted entirely for non-GM callers.
    async fn world_chat_messages(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLChatMessage>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        world_chat_messages_impl(state, user.user_id, user.is_admin, world_id, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_body_trims_and_rejects_empty() {
        assert_eq!(validate_body("  hello  ").unwrap(), "hello");
        assert!(validate_body("").is_err());
        assert!(validate_body("   \n\t ").is_err());
    }

    #[test]
    fn validate_body_limit_counts_chars_not_bytes() {
        // A multi-byte message at exactly the limit must be accepted: each
        // of these is 4 bytes, so a byte-based check would reject it at a
        // quarter of the real length.
        let emoji = "🎲".repeat(MAX_BODY_CHARS);
        assert!(validate_body(&emoji).is_ok());

        let too_long = "🎲".repeat(MAX_BODY_CHARS + 1);
        assert!(validate_body(&too_long).is_err());
    }

    #[test]
    fn history_limit_clamps_to_bounds() {
        assert_eq!(resolve_history_limit(None), DEFAULT_HISTORY_LIMIT);
        assert_eq!(resolve_history_limit(Some(0)), DEFAULT_HISTORY_LIMIT);
        assert_eq!(resolve_history_limit(Some(-5)), DEFAULT_HISTORY_LIMIT);
        assert_eq!(resolve_history_limit(Some(25)), 25);
        assert_eq!(resolve_history_limit(Some(10_000)), MAX_HISTORY_LIMIT);
    }
}
