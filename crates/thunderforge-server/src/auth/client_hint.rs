//! The one place a request's `User-Agent` is allowed to matter.
//!
//! Spec 036 FR-005 wants a session to be recognisable by its owner, and the
//! only thing the browser volunteers that is any use for that is its
//! `User-Agent`. That header is also a fingerprint, so it is read here, turned
//! into a coarse phrase from a fixed vocabulary by
//! `thunderforge_axum_auth_core::client_description`, and then dropped. The
//! raw header never reaches the database, a log line or a response.
//!
//! It is an extractor rather than a `HeaderMap` parameter on each handler so
//! that a handler cannot accidentally hold the raw header while it is at it.

use axum::extract::FromRequestParts;
use axum::http::header::USER_AGENT;
use axum::http::request::Parts;
use std::convert::Infallible;
use thunderforge_axum_auth_core::client_description::describe_client;

/// A coarse name for the client that made this request — "Firefox on Linux" —
/// or `None` when the request did not look like a browser we recognise.
///
/// Sessions created from a request that carries one are recognisable in the
/// session list; sessions created without one render as an unrecognised
/// client, which is honest and still revocable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ClientDescription(pub(crate) Option<String>);

impl ClientDescription {
    /// A session created with no request behind it. Only the tests need this
    /// today — every production path that issues a session has a browser at
    /// the other end of it, and `#[cfg(test)]` is what keeps that true: a
    /// handler reaching for this would stop compiling.
    #[cfg(test)]
    pub(crate) fn unknown() -> Self {
        Self(None)
    }

    pub(crate) fn into_inner(self) -> Option<String> {
        self.0
    }
}

impl<S> FromRequestParts<S> for ClientDescription
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .and_then(describe_client),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    async fn extract(user_agent: Option<&str>) -> Option<String> {
        let mut builder = Request::builder().uri("/");
        if let Some(agent) = user_agent {
            builder = builder.header(USER_AGENT, agent);
        }
        let (mut parts, ()) = builder.body(()).expect("a request builds").into_parts();
        ClientDescription::from_request_parts(&mut parts, &())
            .await
            .expect("the extractor never rejects")
            .into_inner()
    }

    #[tokio::test]
    async fn a_browser_request_is_described() {
        assert_eq!(
            extract(Some(
                "Mozilla/5.0 (X11; Linux x86_64; rv:129.0) Gecko/20100101 Firefox/129.0"
            ))
            .await
            .as_deref(),
            Some("Firefox on Linux"),
        );
    }

    /// A missing or unrecognised header is not an error. Refusing to create a
    /// session because a client did not identify itself would be a new way to
    /// fail to sign in, which is the opposite of what spec 036 is for.
    #[tokio::test]
    async fn a_request_without_a_recognised_agent_is_not_refused() {
        assert_eq!(extract(None).await, None);
        assert_eq!(extract(Some("curl/8.8.0")).await, None);
    }
}
