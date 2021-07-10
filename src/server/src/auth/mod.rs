use crate::config::Config;
use crate::utils::url_base;
use rocket::fairing::AdHoc;
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

/// Remove the `user_id` cookie.
#[post("/logout")]
fn logout(cookies: &CookieJar<'_>) -> Flash<Redirect> {
    cookies.remove_private(Cookie::named("session"));
    Flash::success(Redirect::to("/"), "Successfully logged out.")
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Manage Authentication", |rocket| async {
        rocket.mount(
            url_base("authentication"),
            routes![basic_authentication, logout],
        )
    })
}
