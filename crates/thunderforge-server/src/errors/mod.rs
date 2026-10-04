use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;

pub async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "code": 404, "message": "Not Found" })),
    )
}
