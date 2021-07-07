use crate::config::Config;
use crate::utils::url_base;
use rocket::fairing::AdHoc;
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar};
use rocket::response::{Body, Flash, Redirect};
use rocket::State;
use thunderforge_core::auth::Credentials;

#[post("/basic", data = "<credentials>")]
async fn basic_authentication(
    credentials: String,
    cookies: &CookieJar<'_>,
    config: &State<Config>,
) -> &'static str {
    println!("{}", &credentials);
    let cred = Credentials::from(credentials);
    if cred.authenticate().await {
        cookies.add_private(Cookie::new("session", "123"))
    }
    "success"
}

/// Retrieve the user's ID, if any.
#[get("/user_id")]
fn get_user_id(cookies: &CookieJar<'_>) -> Option<String> {
    cookies
        .get_private("user_id")
        .map(|crumb| format!("User ID: {}", crumb.value()))
}

/// Remove the `user_id` cookie.
#[post("/logout")]
fn logout(cookies: &CookieJar<'_>) -> Flash<Redirect> {
    cookies.remove_private(Cookie::named("user_id"));
    Flash::success(Redirect::to("/"), "Successfully logged out.")
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Manage Authentication", |rocket| async {
        rocket.mount(
            url_base("authentication"),
            routes![basic_authentication, get_user_id, logout],
        )
    })
}
