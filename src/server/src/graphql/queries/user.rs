//! User queries for profile, worlds, tokens, policies, and data exports.

use async_graphql::Context;

use crate::graphql::*;
use crate::users::export_user_data_payload;

#[derive(Default)]
pub struct UserQuery;

#[async_graphql::Object]
impl UserQuery {
    async fn me(&self, ctx: &Context<'_>) -> GraphQLResult<Option<GraphQLUser>> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let user = tokio::task::spawn_blocking(move || {
            use crate::schema::users;
            use crate::models::User;
            use diesel::prelude::*;
            users::table
                .filter(users::id.eq(user_id))
                .select(User::as_select())
                .first::<User>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load user"))?;

        Ok(user.map(GraphQLUser::from))
    }

    async fn game_systems(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLGameSystem>> {
        let state = app_state(ctx)?;
        load_game_systems(state)
            .await
            .map(|items| items.into_iter().map(GraphQLGameSystem::from).collect())
    }

    async fn my_worlds(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_worlds(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorld::from).collect())
    }

    async fn world(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_visible_world_by_id(state, auth_user.user_id, auth_user.is_admin, id)
            .await
            .map(|item| item.map(GraphQLWorld::from))
    }

    async fn my_world_tokens(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorldToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_tokens(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorldToken::from).collect())
    }

    async fn world_token(
        &self,
        ctx: &Context<'_>,
        token_id: String,
    ) -> GraphQLResult<Option<GraphQLWorldToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_token_by_id(state, auth_user.user_id, token_id)
            .await
            .map(|item| item.map(GraphQLWorldToken::from))
    }

    async fn my_world_events(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorldEvent>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_events(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorldEvent::from).collect())
    }

    async fn world_event(
        &self,
        ctx: &Context<'_>,
        event_id: i64,
    ) -> GraphQLResult<Option<GraphQLWorldEvent>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_event_by_id(state, auth_user.user_id, event_id)
            .await
            .map(|item| item.map(GraphQLWorldEvent::from))
    }

    async fn my_policies(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLPolicy>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_policies(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLPolicy::from).collect())
    }

    async fn policy(
        &self,
        ctx: &Context<'_>,
        policy_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLPolicy>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_policy_by_id(state, auth_user.user_id, policy_id)
            .await
            .map(|item| item.map(GraphQLPolicy::from))
    }

    async fn export_my_data(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLExportMyDataPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        export_user_data_payload(state, auth_user.user_id)
            .await
            .map(GraphQLExportMyDataPayload::from)
            .map_err(Error::new)
    }
}
