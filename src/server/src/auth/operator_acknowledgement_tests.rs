//! T082 / FR-041: setup cannot produce an administrator without an
//! acknowledgement, and only the operator statement's own words count as one.

use super::*;
use crate::test_support::test_app_state;

/// A version of the operator statement is accepted only if this instance
/// archived it — and a sharing-terms version, real as it is, is the wrong
/// words.
#[tokio::test]
async fn only_an_archived_operator_version_is_an_acknowledgement() {
    let state = test_app_state();
    crate::legal::ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");
    let mut conn = state.db_pool.get().expect("conn");

    assert!(
        is_operator_version_sync(&mut conn, &crate::legal::operator_statement().version_id)
            .expect("read"),
    );
    assert!(
        !is_operator_version_sync(&mut conn, &crate::legal::sharing_terms().version_id)
            .expect("read"),
        "a real version of a different document is not an acknowledgement of this one",
    );
    assert!(
        !is_operator_version_sync(&mut conn, "operator-responsibilities@0000000000000000")
            .expect("read"),
    );
}

/// Structural, the way the sharing gate's non-null argument is: both setup
/// requests fail to parse without the acknowledgement, so no handler can be
/// written that forgets to ask for it.
#[test]
fn neither_setup_path_parses_without_an_acknowledgement() {
    use crate::auth::types::{AdminSetupBasicRequest, AdminSetupOAuthStartRequest};

    let basic = serde_json::json!({
        "admin_code": "code",
        "username": "founder",
        "email": "founder@realdomain.org",
        "password": "a long passphrase",
    });
    assert!(serde_json::from_value::<AdminSetupBasicRequest>(basic.clone()).is_err());
    let mut acknowledged = basic;
    acknowledged["operator_acknowledgement"] = serde_json::json!({ "terms_version_id": "v" });
    assert!(serde_json::from_value::<AdminSetupBasicRequest>(acknowledged).is_ok());

    let oauth = serde_json::json!({
        "admin_code": "code",
        "redirect_uri": "https://realdomain.org/callback",
    });
    assert!(serde_json::from_value::<AdminSetupOAuthStartRequest>(oauth.clone()).is_err());
    let mut acknowledged = oauth;
    acknowledged["operator_acknowledgement"] = serde_json::json!({ "terms_version_id": "v" });
    assert!(serde_json::from_value::<AdminSetupOAuthStartRequest>(acknowledged).is_ok());
}
