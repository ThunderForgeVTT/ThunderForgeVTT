use async_graphql::{Context, MergedObject, Schema, SimpleObject, Subscription};
use diesel::prelude::*;
use diesel::{PgConnection, r2d2::ConnectionManager, r2d2::Pool};
use futures_util::Stream; // Import Stream
use std::time::Duration; // Import Duration
use tokio_stream::wrappers::IntervalStream; // Import IntervalStream

use crate::auth_middleware::AuthenticatedUser;
use crate::models::{User, World};
use crate::schema::worlds::dsl::*;

// To expose the User struct in GraphQL
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLUser {
    id: uuid::Uuid,
    username: String,
    email: String,
}

impl From<User> for GraphQLUser {
    fn from(user: User) -> Self {
        GraphQLUser {
            id: user.id,
            username: user.username,
            email: user.email,
        }
    }
}

// To expose the World struct in GraphQL
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorld {
    id: uuid::Uuid,
    name: String,
}

impl From<World> for GraphQLWorld {
    fn from(world: World) -> Self {
        GraphQLWorld {
            id: world.id,
            name: world.name,
        }
    }
}

#[derive(Default)]
pub struct HealthcheckQuery;

#[async_graphql::Object]
impl HealthcheckQuery {
    /// Returns `true` to indicate the service is running.
    async fn healthcheck(&self) -> bool {
        true
    }
}

#[derive(Default)]
pub struct UserQuery;

#[async_graphql::Object]
impl UserQuery {
    /// Retrieves the currently authenticated user.
    async fn me(&self, ctx: &Context<'_>) -> Option<GraphQLUser> {
        let pool = ctx
            .data::<Pool<ConnectionManager<PgConnection>>>()
            .expect("Can't get DB pool");
        let auth_user = ctx.data::<AuthenticatedUser>().ok()?;
        let mut conn = pool.get().expect("Can't get DB connection");

        crate::schema::users::dsl::users
            .filter(crate::schema::users::dsl::id.eq(auth_user.user_id))
            .select(crate::models::User::as_select())
            .first::<crate::models::User>(&mut conn)
            .optional()
            .expect("Error loading user")
            .map(GraphQLUser::from)
    }
}

#[derive(Default)]
pub struct WorldMutation;

#[async_graphql::Object]
impl WorldMutation {
    /// Creates a new world for the user.
    async fn create_world(
        &self,
        ctx: &Context<'_>,
        world_name: String,
    ) -> async_graphql::Result<GraphQLWorld> {
        let pool = ctx
            .data::<Pool<ConnectionManager<PgConnection>>>()
            .expect("Can't get DB pool");
        let mut conn = pool.get().expect("Can't get DB connection");

        let new_world = World {
            id: uuid::Uuid::now_v7(),
            name: world_name,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        diesel::insert_into(worlds)
            .values(&new_world)
            .execute(&mut conn)?;

        Ok(GraphQLWorld::from(new_world))
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Returns an incrementing number every second.
    async fn tick(&self) -> impl Stream<Item = i32> {
        let mut value = 0;
        tokio_stream::StreamExt::map(
            IntervalStream::new(tokio::time::interval(Duration::from_secs(1))),
            move |_| {
                value += 1;
                value
            },
        )
    }
}

#[derive(MergedObject, Default)]
pub struct QueryRoot(HealthcheckQuery, UserQuery);

#[derive(MergedObject, Default)]
pub struct MutationRoot(WorldMutation);

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;
