use rocket::fairing::AdHoc;
use rocket::serde::{json::Json, Serialize};

#[derive(Serialize)]
struct NotFoundError {
    code: u16,
    message: String,
}

#[catch(404)]
fn generic_not_found() -> Json<NotFoundError> {
    Json(NotFoundError {
        code: 404,
        message: "Page Not Found!".to_string(),
    })
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Errors", |rocket| async {
        rocket.register("/", catchers![generic_not_found])
    })
}
