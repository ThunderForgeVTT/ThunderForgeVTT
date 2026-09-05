//! Placing and moving the tokens on a scene.

use async_graphql::{Context, Error, Result as GraphQLResult};

use super::*;

// World token input types moved to input_types.rs (Phase 4.9.Z Step 3)

#[derive(Default)]
pub struct WorldTokenMutation;

#[async_graphql::Object]
impl WorldTokenMutation {
    async fn create_world_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWorldTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = uuid::Uuid::now_v7().to_string();
        let world_id = input.world_id;
        let label = input.label;
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let z = input.z.unwrap_or(0.0);
        let health = input.health;
        let max_health = input.max_health;

        let created_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;

            diesel::insert_into(world_tokens::table)
                .values((
                    world_tokens::id.eq(&token_id),
                    world_tokens::world_id.eq(world_id),
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::schema_version.eq(1),
                    world_tokens::created_at.eq(now),
                    world_tokens::updated_at.eq(now),
                    world_tokens::created_by.eq(user_id),
                    world_tokens::updated_by.eq(user_id),
                ))
                .returning(crate::models::WorldToken::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create world token"))?;

        // Phase 4.9.B.2: Touch last_seen on mutation
        if let Err(e) =
            crate::session::touch_last_seen(state.db_pool.clone(), user_id, input.world_id).await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(created_token))
    }

    async fn upsert_world_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpsertWorldTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input
            .token_id
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let world_id = input.world_id;
        let label = input.label.clone();
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let z = input.z.unwrap_or(0.0);
        let health = input.health;
        let max_health = input.max_health;

        // 🎮📤 Phase 4.6: Circular flow begins - client sent mutation
        eprintln!(
            "[Phase4.6📤] upsertToken mutation received: token_id={}, pos=({},{},{})",
            token_id, x, y, z
        );

        // Combine all DB operations into a single spawn_blocking call
        let token_id_clone = token_id.clone();
        let (upserted_token, event_id) = tokio::task::spawn_blocking(move || {
            use crate::schema::{world_events, world_tokens};
            use diesel::prelude::*;

            // 1. UPSERT token
            let upserted = diesel::insert_into(world_tokens::table)
                .values((
                    world_tokens::id.eq(&token_id_clone),
                    world_tokens::world_id.eq(world_id),
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::schema_version.eq(1),
                    world_tokens::created_at.eq(now),
                    world_tokens::updated_at.eq(now),
                    // 🔐 ADR-010: Server assigns ownership from auth context
                    world_tokens::created_by.eq(user_id),
                    world_tokens::updated_by.eq(user_id),
                ))
                .on_conflict(world_tokens::id)
                .do_update()
                .set((
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::updated_by.eq(user_id),
                    world_tokens::updated_at.eq(now),
                ))
                .returning(crate::models::WorldToken::as_returning())
                .get_result(&mut conn)?;

            // 2. Record world_event for audit trail
            let token_event_payload = serde_json::json!({
                "token_id": token_id_clone,
                "x": x,
                "y": y,
                "z": z,
                "label": label,
                "health": health,
                "max_health": max_health,
            });

            let event_id = diesel::insert_into(world_events::table)
                .values((
                    world_events::world_id.eq(world_id),
                    // Event code 1 = token_event
                    world_events::event_code.eq(1),
                    world_events::token_event.eq(token_event_payload),
                    world_events::schema_version.eq(1),
                    world_events::created_at.eq(now),
                    world_events::updated_at.eq(now),
                    world_events::created_by.eq(user_id),
                    world_events::updated_by.eq(user_id),
                ))
                .returning(world_events::id)
                .get_result::<i64>(&mut conn)?;

            // 3. Trigger pg_notify for backplane broadcast
            diesel::sql_query("SELECT pg_notify('world_events_channel', $1)")
                .bind::<diesel::sql_types::Text, _>(event_id.to_string())
                .execute(&mut conn)?;

            Ok::<_, diesel::result::Error>((upserted, event_id))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| Error::new(format!("Database operation failed: {}", e)))?;

        eprintln!(
            "[Phase4.6✅] upsertToken complete: token_id={}, event_id={}, broadcasted",
            token_id, event_id
        );

        // Phase 4.9.B.2: Touch last_seen on mutation
        if let Err(e) =
            crate::session::touch_last_seen(state.db_pool.clone(), user_id, world_id).await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(upserted_token))
    }

    async fn move_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLMoveTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input.token_id;
        let x = input.x;
        let y = input.y;
        let z = input.z.unwrap_or(0.0);

        let moved_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;

            diesel::update(
                world_tokens::table
                    .filter(world_tokens::id.eq(&token_id))
                    .filter(world_tokens::created_by.eq(user_id)),
            )
            .set((
                world_tokens::x.eq(x),
                world_tokens::y.eq(y),
                world_tokens::z.eq(z),
                world_tokens::updated_by.eq(user_id),
                world_tokens::updated_at.eq(now),
            ))
            .returning(crate::models::WorldToken::as_returning())
            .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to move token"))?;

        // Phase 4.9.B.2: Touch last_seen on mutation
        if let Err(e) =
            crate::session::touch_last_seen(state.db_pool.clone(), user_id, moved_token.world_id)
                .await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(moved_token))
    }

    async fn delete_world_token(&self, ctx: &Context<'_>, token_id: String) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;
            diesel::delete(
                world_tokens::table
                    .filter(world_tokens::id.eq(&token_id))
                    .filter(world_tokens::created_by.eq(user_id)),
            )
            .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete token"))?;

        Ok(deleted > 0)
    }
}
