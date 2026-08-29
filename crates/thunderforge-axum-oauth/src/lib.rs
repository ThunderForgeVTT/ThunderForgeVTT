//! The OAuth 2.0 authorization-code flow with PKCE, as pure functions.
//!
//! Nothing here performs I/O. The server owns the two HTTP calls the flow
//! makes (token exchange, userinfo) and the database rows it persists; this
//! crate owns every decision made around them, so those decisions can be
//! tested against generated input instead of against a provider account.
//!
//! [`provider_kind::ProviderKind`] is also the answer to "how do we not
//! forget to wire a provider up" — see that module.

pub mod authorize;
pub mod error;
pub mod pkce;
pub mod provider_kind;
pub mod state;
pub mod token;
