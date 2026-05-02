use crate::schema::users;
use crate::state::AppState;
use axum::{
    routing::post,
    Router,
};
use axum::extract::State;
use diesel::prelude::*;
use tower_cookies::{Cookies, Cookie};
use thunderforge_core::auth::Credentials;

use argon2::{Argon2, PasswordHash, PasswordVerifier};

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: uuid::Uuid,
    pub username: String,
    pub password: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/authentication/basic", post(basic_authentication))
        .route("/authentication/logout", post(logout))
}

async fn basic_authentication(
    cookies: Cookies,
    State(state): State<AppState>,
    credentials: String,
) -> &'static str {
    println!("{}", &credentials);
    let cred = Credentials::from(credentials);

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::username.eq(&cred.username))
            .first::<User>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    match user {
        Some(user) => {
            let parsed_hash = PasswordHash::new(&user.password).expect("Invalid hash in db");
            if Argon2::default().verify_password(cred.password.as_bytes(), &parsed_hash).is_ok() {
                let mut cookie = Cookie::new("session", "123");
                cookie.set_path("/");
                cookies.private(&state.key).add(cookie);
                "success"
            } else {
                "failure"
            }
        }
        None => "failure",
    }
}

async fn logout(cookies: Cookies, State(state): State<AppState>) {
    cookies.private(&state.key).remove(Cookie::new("session", ""));
}
