//! Spec 088 (FR-010): a provider sign-in started from a world link is sign-in
//! only.
//!
//! A world link admits existing ThunderForge accounts and nothing else. The
//! sign-in page reached from `/join/<code>` offers no registration, and this
//! closes the other door: an OAuth identity with no account behind it, whose
//! flow began on a join page, is refused rather than provisioned. That holds
//! whatever the instance's access mode, `open` included, because the link is
//! the GM's invitation to a world and not the operator's invitation to the
//! instance.
//!
//! The mark is not stored separately. The authorize step already persists
//! `return_to` on the authorization session, and the callback reads it back
//! from there, so the browser cannot lift the mark between the two steps.

use axum::Json;
use axum::http::StatusCode;

use super::types::OAuthResponse;

/// What the visitor reads when a world link's sign-in would have created an
/// account.
pub(crate) const WORLD_LINK_OAUTH_MESSAGE: &str = "World links are for existing ThunderForge accounts. Ask the instance's administrator for an invitation.";

/// Whether a flow's `return_to` lands on a join page. The web app sends an
/// absolute URL; a bare path is read the same way.
pub(crate) fn is_world_link_return(return_to: Option<&str>) -> bool {
    let Some(return_to) = return_to.map(str::trim).filter(|v| !v.is_empty()) else {
        return false;
    };
    let path = if return_to.starts_with('/') && !return_to.starts_with("//") {
        return_to.to_string()
    } else {
        match url::Url::parse(return_to) {
            Ok(parsed) => parsed.path().to_string(),
            Err(_) => return false,
        }
    };
    path.starts_with("/join/")
}

/// The refusal: no account, no identity link, no session.
pub(crate) fn refused() -> (StatusCode, Json<OAuthResponse>) {
    // The `thunderforge.world_links.oauth_refused` counter waits on spec 086.
    super::error_response(
        StatusCode::FORBIDDEN,
        "world_link_sign_in_only",
        WORLD_LINK_OAUTH_MESSAGE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_join_page_is_a_world_link_return_however_it_is_written() {
        for yes in [
            "https://vtt.example.org/join/ABC123",
            "http://127.0.0.1:5200/join/abc?x=1",
            "/join/ABC123",
            "  /join/ABC123 ",
        ] {
            assert!(is_world_link_return(Some(yes)), "{yes}");
        }
    }

    #[test]
    fn anywhere_else_is_not() {
        for no in [
            None,
            Some(""),
            Some("/welcome"),
            Some("/joined"),
            Some("/world/abc/join/x"),
            Some("https://vtt.example.org/welcome?next=/join/ABC"),
            Some("//evil.example/join/ABC"),
            Some("not a url"),
        ] {
            assert!(!is_world_link_return(no), "{no:?}");
        }
    }
}
