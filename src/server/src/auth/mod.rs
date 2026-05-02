use crate::state::AppState;
use axum::{
    routing::post,
    Router,
};
use axum::extract::State;
use tower_cookies::{Cookies, Cookie};
use thunderforge_core::auth::Credentials;

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
    if cred.authenticate().await {
        let mut cookie = Cookie::new("session", "123");
        cookie.set_path("/");
        cookies.private(&state.key).add(cookie);
    }
    "success"
}

async fn logout(cookies: Cookies, State(state): State<AppState>) {
    cookies.private(&state.key).remove(Cookie::new("session", ""));
}
