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
    /// Spec 041 FR-001a: one flow, three entrances, and they do not all have
    /// the same thing to prove with. Account settings and first-run setup send
    /// a username and password; a sign-in that requires enrolment sends the
    /// login challenge it was just handed, because at that moment the person
    /// has a correct password and no session. Exactly one of the two is
    /// accepted — `authorise_enrolment` refuses both together and neither at
    /// all.
    #[serde(default)]
    pub(crate) username: Option<String>,
    #[serde(default)]
    pub(crate) password: Option<String>,
    #[serde(default)]
    pub(crate) challenge_id: Option<uuid::Uuid>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TwoFactorSetupStartResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    pub(crate) otpauth_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TwoFactorSetupConfirmRequest {
    /// As `TwoFactorSetupStartRequest`: exactly one of a username and password
    /// or a login challenge.
    #[serde(default)]
    pub(crate) username: Option<String>,
    #[serde(default)]
    pub(crate) password: Option<String>,
    #[serde(default)]
    pub(crate) challenge_id: Option<uuid::Uuid>,
    pub(crate) code: String,
}

/// Confirmation's reply. `status`/`message` are what the previous
/// `OAuthResponse` shape said and are unchanged, so an existing client keeps
/// working; the recovery fields are added because spec 041 FR-006 makes
/// confirmation the moment the codes are issued, and this body is the only
/// place they will ever exist outside a hash.
#[derive(Debug, Serialize)]
pub(crate) struct TwoFactorSetupConfirmResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) confirmed_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_notice: Option<String>,
    /// FR-020: present, and true, only when a login challenge authorised this
    /// enrolment — in which case the sign-in the person was already doing is
    /// finished here and a session cookie came back with this body. The
    /// settings entrance already had a session and gets no field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signed_in: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TwoFactorVerifyRequest {
    pub(crate) challenge_id: uuid::Uuid,
    /// Spec 041 US2 (FR-007): a challenge takes an authenticator code **or** a
    /// recovery code, and the handler refuses a request carrying both before
    /// it evaluates either — a client sending both is asking for two chances
    /// counted as one attempt.
    #[serde(default)]
    pub(crate) code: Option<String>,
    #[serde(default)]
    pub(crate) recovery_code: Option<String>,
}

/// The login challenge's reply. `status`/`message` are unchanged from the
/// `OAuthResponse` it used to return; the two recovery fields are added
/// because FR-011 says a person running low is told **at the moment it
/// matters** — the sign-in they just completed — rather than only if they
/// happen to visit a settings page.
///
/// A refusal carries neither field. How many codes an account has left is not
/// something a failed attempt gets to learn (FR-018).
#[derive(Debug, Serialize)]
pub(crate) struct TwoFactorVerifyResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_remaining: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_low: Option<bool>,
}

/// Spec 041 US2 (FR-010): `POST /authentication/2fa/recovery-codes`. Exactly
/// one of the two fields, for the same reason the challenge takes one.
#[derive(Debug, Deserialize)]
pub(crate) struct RecoveryCodesRegenerateRequest {
    #[serde(default)]
    pub(crate) code: Option<String>,
    #[serde(default)]
    pub(crate) recovery_code: Option<String>,
}

/// Spec 041 US4 (FR-012, FR-014): `POST /authentication/2fa/disable`.
///
/// Password **and** possession, which is exactly what adding a factor cost.
/// The session alone is not enough: it proves the password was held at
/// sign-in, which may have been days ago on a machine now in somebody else's
/// hands, and removing the second factor is the one action that makes every
/// future sign-in cheaper.
#[derive(Debug, Deserialize)]
pub(crate) struct TwoFactorDisableRequest {
    pub(crate) password: String,
    #[serde(default)]
    pub(crate) code: Option<String>,
    #[serde(default)]
    pub(crate) recovery_code: Option<String>,
}

/// The only shape that ever carries recovery-code plaintext, and it carries it
/// exactly once — from the response that issues a set. Nothing reads these
/// values back out of the server afterwards, because nothing can (FR-009).
#[derive(Debug, Serialize)]
pub(crate) struct RecoveryCodesResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_notice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_remaining: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_low: Option<bool>,
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
    /// Spec 040 (FR-002, FR-003, FR-009): every `RequiredAtSetup` declaration,
    /// resolved — so the wizard is driven by `settings::registry` and not by a
    /// hard-coded list of steps. Empty once setup is complete; see
    /// `admin_setup::setup_status` for why an anonymous caller on a running
    /// instance is not told which of its settings are unset.
    pub(crate) required_settings: Vec<crate::auth::setup_requirements::RequiredSetting>,
    /// Spec 040 FR-002a. The predicate is 040's; the enrolment flow is 041's.
    pub(crate) second_factor_confirmed: bool,
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

/// Spec 041 FR-005: an account's own second factor, as its owner sees it.
#[derive(Debug, Serialize)]
pub(crate) struct TwoFactorStatusResponse {
    pub(crate) status: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) message: String,
    pub(crate) enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) confirmed_at: Option<String>,
    pub(crate) enrolment_pending: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_codes_remaining: Option<i64>,
    pub(crate) recovery_codes_low: bool,
}
