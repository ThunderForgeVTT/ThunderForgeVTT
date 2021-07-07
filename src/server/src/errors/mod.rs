use rocket::fairing::AdHoc;
use rocket::serde::{json::Json, Serialize};

#[derive(Serialize)]
pub struct InternalServerError {
    pub(crate) code: u16,
    pub(crate) message: String,
}

#[catch(500)]
fn internal_server_error() -> Json<InternalServerError> {
    Json(InternalServerError {
        code: 500,
        message: "An internal server error has occurred!".to_string(),
    })
}

#[derive(Serialize)]
pub struct NotFoundError {
    pub(crate) code: u16,
    pub(crate) message: String,
}

#[catch(404)]
fn not_found() -> Json<NotFoundError> {
    Json(NotFoundError {
        code: 404,
        message: "Page Not Found!".to_string(),
    })
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Errors", |rocket| async {
        rocket.register("/", catchers![not_found, internal_server_error])
    })
}
