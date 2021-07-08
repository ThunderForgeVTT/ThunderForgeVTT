use crate::utils::url_base;
use rocket::fairing::AdHoc;
use rocket::http::CookieJar;

/// Retrieve the user's ID, if any.
#[get("/<id>")]
fn get_user(cookies: &CookieJar<'_>, id: String) -> Option<String> {
    cookies
        .get_private("session")
        .map(|crumb| format!("User ID: {} - {}", crumb.value(), id))
}

// #[patch("/user")]

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Manage Users", |rocket| async {
        rocket.mount(url_base("user"), routes![get_user])
    })
}
