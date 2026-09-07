//! The request and response shapes the authentication routes speak.

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    pub(crate) identifier: String,
    pub(crate) password: String,
    pub(crate) two_factor_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegisterRequest {
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) password: String,
    /// Spec 035 (FR-016): an instance invitation, when the visitor arrived by
    /// one. Optional — an open instance needs none, and a closed one refuses
    /// regardless.
    #[serde(default)]
    pub(crate) invitation_code: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct OAuthResolveRequest {
    /// Spec 035 (FR-016): an instance invitation carried through the provider
    /// round trip, when the visitor arrived by one.
    #[serde(default)]
    pub(crate) invitation_code: Option<String>,
    pub(crate) provider_key: String,
    pub(crate) provider_user_id: String,
    pub(crate) provider_email: Option<String>,
    pub(crate) access_token: Option<String>,
    pub(crate) refresh_token: Option<String>,
    pub(crate) token_expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthLinkConfirmRequest {
    pub(crate) challenge_id: uuid::Uuid,
    pub(crate) password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TwoFactorSetupStartRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct TwoFactorSetupStartResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    pub(crate) otpauth_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TwoFactorSetupConfirmRequest {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) code: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TwoFactorVerifyRequest {
    pub(crate) challenge_id: uuid::Uuid,
    pub(crate) code: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminTwoFactorRequirementRequest {
    pub(crate) required_for_all_users: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminUserTwoFactorRequiredRequest {
    pub(crate) required: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthStartQuery {
    pub(crate) redirect_uri: String,
    pub(crate) return_to: Option<String>,
    /// Spec 035 (FR-016): `/invite/{code}` starts the provider flow with this
    /// set, so redemption survives the round trip.
    pub(crate) invitation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthCallbackQuery {
    pub(crate) code: Option<String>,
    pub(crate) state: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthTokenExchangeRequest {
    pub(crate) code: String,
    pub(crate) state: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupStatusResponse {
    pub(crate) setup_required: bool,
    pub(crate) setup_completed: bool,
    pub(crate) configured_oauth_providers: Vec<SetupOAuthProvider>,
    /// Spec 035 (FR-003): the instance's admission policy, so the signed-out
    /// surface offers only routes that will actually work.
    ///
    /// This is the *only* thing about access exposed to an unauthenticated
    /// caller. No user count, no invitation codes, no indication that any
    /// particular invitation or account exists.
    pub(crate) access_policy: String,
    /// Ships as a constant `false` until US3 exists. Present now so the
    /// front-end shape does not change when it does.
    pub(crate) accepting_access_requests: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupOAuthProvider {
    pub(crate) provider_key: String,
    pub(crate) display_name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminSetupBasicRequest {
    pub(crate) admin_code: String,
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminSetupOAuthStartRequest {
    pub(crate) admin_code: String,
    pub(crate) redirect_uri: String,
    pub(crate) username: Option<String>,
    pub(crate) return_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminSetupOAuthStartResponse {
    pub(crate) authorization_url: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct OAuthResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    pub(crate) challenge_id: Option<uuid::Uuid>,
    pub(crate) login_two_factor_challenge_id: Option<uuid::Uuid>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SessionStateResponse {
    pub(crate) authenticated: bool,
    pub(crate) user: PublicUser,
    pub(crate) session_expires_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthSessionResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    pub(crate) session: Option<SessionStateResponse>,
    pub(crate) login_two_factor_challenge_id: Option<uuid::Uuid>,
    pub(crate) requires_email_verification: bool,
}

pub(crate) struct OAuthAuthorizationContext {
    pub(crate) provider: OAuthProvider,
    pub(crate) session: OAuthAuthorizationSession,
}

pub(crate) struct AdminBootstrapOAuthContext {
    pub(crate) provider: OAuthProvider,
    pub(crate) session: AdminBootstrapOAuthSession,
}

pub(crate) enum ResolveOutcome {
    ProviderNotFound,
    LinkedUser(uuid::Uuid),
    /// Spec 035: an account was **created** by this call, not merely linked.
    /// Distinguished from `LinkedUser` so the caller can record an invitation
    /// redemption against it — and so that releasing an unused invitation is
    /// possible when the flow linked to an existing account instead.
    ProvisionedUser(uuid::Uuid),
    PasswordRequired(uuid::Uuid),
    NoMatchingUser,
    /// Spec 035 / ADR-072: the instance's policy refuses to admit this
    /// identity. No account was created and no session is issued.
    ///
    /// Never constructed today: the pre-flight gate in `oauth.rs` returns
    /// early, so the blocking closure cannot yield it. It exists so that
    /// `resolve_oauth_login`'s caller keeps a place to put the refusal if the
    /// decision ever moves inside the closure, and `oauth.rs` matches it for
    /// the same reason. `expect` rather than `allow` so that the day it *is*
    /// constructed, the unfulfilled expectation says the note above is stale.
    #[expect(dead_code, reason = "matched for exhaustiveness; see oauth.rs")]
    NotAdmitted(String),
}

pub(crate) enum LinkConfirmOutcome {
    ChallengeInvalid,
    ChallengeExpired,
    PasswordMismatch,
    LinkConflict,
    Linked(uuid::Uuid),
}
