//! The first administrator's password, checked where it is stored.
//!
//! The wizard's account step says "at least 12 characters" and refuses to
//! submit less. `/authentication/setup/basic` used to check only that the
//! password was not empty, so the rule was a property of one web page: a
//! request made any other way created the account that owns the instance with
//! a one-character password.
//!
//! The handler is driven directly. The refusal comes before the bootstrap code
//! is looked at, which is what lets this run against a database that finished
//! setup long ago — and is asserted, because a refusal that depended on the
//! setup state would not be testable here at all.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use tower_cookies::Cookies;

use super::*;
use crate::auth::operator_acknowledgement::OperatorAcknowledgementRequest;
use crate::test_support::test_app_state;
use thunderforge_axum_auth_core::password::PASSWORD_MIN_LEN;

async fn attempt(username: &str, password: &str) -> (StatusCode, String, &'static str) {
    let (status, Json(body)) = admin_setup_basic(
        Cookies::default(),
        ClientDescription::unknown(),
        State(test_app_state()),
        Json(AdminSetupBasicRequest {
            admin_code: "not-a-real-code".to_string(),
            username: username.to_string(),
            email: "first-admin@example.org".to_string(),
            password: password.to_string(),
            operator_acknowledgement: OperatorAcknowledgementRequest {
                terms_version_id: "any".to_string(),
            },
        }),
    )
    .await;
    (status, body.message, body.status)
}

#[tokio::test]
async fn a_short_first_administrator_password_is_refused_by_the_server() {
    let short = "a".repeat(PASSWORD_MIN_LEN - 1);
    let (status, message, code) = attempt("firstadmin", &short).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{message}");
    assert_eq!(code, "invalid_request");
    assert!(
        message.contains(&PASSWORD_MIN_LEN.to_string()),
        "the refusal must name the rule that was broken: {message}"
    );
}

/// A password that meets the rule gets past it — to the bootstrap code, which
/// here is wrong or long since consumed. Anything but the password refusal.
#[tokio::test]
async fn a_long_enough_password_is_not_refused_for_its_length() {
    let long = "a".repeat(PASSWORD_MIN_LEN);
    let (status, message, code) = attempt("firstadmin", &long).await;

    assert_ne!(code, "invalid_request", "{status}: {message}");
    assert!(!message.contains("Password must be"), "{message}");
}
