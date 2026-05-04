use crate::models::User;
use crate::schema::users;
use crate::state::AppState;
use diesel::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PublicUser {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<User> for PublicUser {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            is_admin: user.is_admin,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

pub async fn load_public_user(state: &AppState, user_id: uuid::Uuid) -> Result<PublicUser, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map(PublicUser::from)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to load user".to_string())
}
