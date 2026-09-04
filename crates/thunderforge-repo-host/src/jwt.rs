//! The application assertion: claims, and an RS256 signature over them.
//!
//! An installed application does not have a password. It proves who it is by
//! signing a short-lived JWT with the private key the operator registered, and
//! trades that assertion for an installation credential. This module builds
//! the claims and signs them, and does nothing else.
//!
//! # Why the time is a parameter
//!
//! Every function here takes `now` rather than reading the clock. That single
//! decision is what makes the validity window testable: "an assertion is
//! backdated by a minute", "its lifetime never exceeds the host's ceiling",
//! "arithmetic near the top of the range refuses rather than wraps" are all
//! assertions about a pure function of `now`, and a module that called
//! `SystemTime::now()` internally could only be tested by waiting.
//!
//! It also removes a whole category of test flake. A test that signs at the
//! instant a second ticks over is a test that fails on some runs and not
//! others, and nobody ever believes that failure the first three times.
//!
//! # Why the assertion is backdated
//!
//! [`CLOCK_SKEW_BACKDATE_SECS`] moves `iat` a minute into the past. The host
//! rejects an assertion issued in its own future, and our clock and theirs
//! disagree by some small amount we cannot measure. Backdating trades a minute
//! of the assertion's usable life — which is spent inside a single HTTPS round
//! trip anyway — for immunity to the most common cause of a working instance
//! suddenly being unable to authenticate at all.

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::{RepoHostError, UnixSeconds};

/// The longest span a host will accept between `iat` and `exp`.
///
/// Ten minutes is the documented ceiling. It is expressed here so the refusal
/// is ours and legible, rather than arriving as an opaque 401 from the host
/// after a deploy changed a configuration value.
pub const MAX_JWT_LIFETIME_SECS: u64 = 600;

/// How far `iat` is moved into the past. See the module documentation.
pub const CLOCK_SKEW_BACKDATE_SECS: u64 = 60;

/// The lifetime used when a caller has no opinion.
///
/// Eight minutes from `now`, which with the one-minute backdate leaves a
/// minute of headroom under [`MAX_JWT_LIFETIME_SECS`]. Sitting at the ceiling
/// would mean any future change to the backdate silently produces assertions
/// the host refuses; the headroom is cheap because an assertion is used once,
/// immediately.
pub const DEFAULT_JWT_LIFETIME_SECS: u64 = 480;

/// The claims of an application assertion.
///
/// Three fields, which is all the host reads. Anything else would be sent,
/// signed, and ignored — and a signed claim nobody validates is a claim that
/// invites someone later to believe it means something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppJwtClaims {
    /// Issued at — backdated by [`CLOCK_SKEW_BACKDATE_SECS`].
    pub iat: UnixSeconds,
    /// Expires at — `now + lifetime`, never more than
    /// [`MAX_JWT_LIFETIME_SECS`] after `iat`.
    pub exp: UnixSeconds,
    /// Issuer: the registered application's identifier.
    pub iss: String,
}

/// Build the claims for an assertion issued at `now`.
///
/// `lifetime_secs` is measured forward from `now`, not from the backdated
/// `iat`, because that is the span a caller actually reasons about — "this
/// assertion is good for the next eight minutes". The ceiling check is applied
/// to `exp - iat`, which is the span the host measures, so the two cannot
/// drift apart if the backdate ever changes.
///
/// Errors rather than clamping. A lifetime outside the range is a
/// configuration mistake, and quietly using a different number than was asked
/// for is how a mistake survives to production without anybody noticing.
pub fn build_claims(
    app_id: &str,
    now: UnixSeconds,
    lifetime_secs: u64,
) -> Result<AppJwtClaims, RepoHostError> {
    if app_id.trim().is_empty() {
        return Err(RepoHostError::MissingAppId);
    }

    let iat = now.saturating_sub(CLOCK_SKEW_BACKDATE_SECS);
    let exp = now
        .checked_add(lifetime_secs)
        .ok_or(RepoHostError::ClockOutOfRange { now })?;

    // `exp - iat` cannot underflow: `iat <= now <= exp`.
    let span = exp - iat;
    if lifetime_secs == 0 || span > MAX_JWT_LIFETIME_SECS {
        return Err(RepoHostError::JwtLifetimeOutOfRange {
            requested: span,
            limit: MAX_JWT_LIFETIME_SECS,
        });
    }

    Ok(AppJwtClaims {
        iat,
        exp,
        iss: app_id.to_string(),
    })
}

/// Parse a PEM-encoded RSA private key into a signing key.
///
/// Separated from signing so the operator's configuration can be validated
/// once, at startup, instead of at the moment a Game Master presses "connect".
/// FR-036c asks for exactly that posture — an instance should know its
/// registration is unusable before a user discovers it.
pub fn encoding_key_from_pem(private_key_pem: &[u8]) -> Result<EncodingKey, RepoHostError> {
    EncodingKey::from_rsa_pem(private_key_pem)
        .map_err(|e| RepoHostError::InvalidPrivateKey(e.to_string()))
}

/// Sign claims with a key that has already been parsed.
///
/// RS256 is not a choice this crate gets to make — it is what the host
/// verifies with — so the algorithm is fixed here rather than being a
/// parameter. A configurable algorithm would only ever be configured wrong.
pub fn sign_claims(key: &EncodingKey, claims: &AppJwtClaims) -> Result<String, RepoHostError> {
    jsonwebtoken::encode(&Header::new(Algorithm::RS256), claims, key)
        .map_err(|e| RepoHostError::Signing(e.to_string()))
}

/// Build and sign an assertion in one step, from a PEM key.
///
/// A convenience for callers holding a key they have not parsed — chiefly
/// tests. Production callers should parse the key once via
/// [`encoding_key_from_pem`] and keep it: re-parsing per request does RSA key
/// decoding on every token refresh for no benefit.
pub fn sign_app_jwt(
    private_key_pem: &[u8],
    app_id: &str,
    now: UnixSeconds,
    lifetime_secs: u64,
) -> Result<String, RepoHostError> {
    let claims = build_claims(app_id, now, lifetime_secs)?;
    let key = encoding_key_from_pem(private_key_pem)?;
    sign_claims(&key, &claims)
}
