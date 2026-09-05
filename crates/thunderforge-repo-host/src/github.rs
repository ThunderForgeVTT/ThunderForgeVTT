//! The GitHub adapter: the one place in the workspace that names a host.
//!
//! This module is the whole reason for the crate boundary (research R5a).
//! FR-004a asks that the host-specific parts be *pointed at* rather than
//! claimed, and a module is the strongest form of pointing available:
//! everything outside `thunderforge-repo-host` cannot reach a GitHub concept
//! even by accident, because it cannot name the types.
//!
//! What lives here is exactly the host-shaped half of the four steps in
//! [`RepoHost`](crate::RepoHost): the URL a user is handed off to, the shape
//! of the installation payload that comes back, the endpoint the assertion is
//! traded at. What does not live here is any HTTP — see
//! `src/server/src/repo_host.rs`.
//!
//! # The two permissions, and why the second one is not an oversight
//!
//! [`REQUESTED_PERMISSIONS`] asks for **contents: write** and **issues:
//! write**.
//!
//! The first is obvious — the feature mirrors lore into a repository, which
//! means writing files.
//!
//! The second is required by **FR-036e**, and it deserves the paragraph
//! FR-036e gives it. FR-036 says to request the narrowest access the feature
//! needs; issue-write is wider than mirroring files needs, and asking for it
//! is a deliberate trade rather than a slip. It exists because of **FR-040b**:
//! when a moderation action disables content that was mirrored to a *publicly
//! visible* repository, the platform must lodge an issue on that repository
//! saying it has disabled the content on its own systems, stopped exporting
//! it, and no longer associates itself with what remains. That is the entire
//! extent of the action — FR-040c forbids deleting, editing or force-pushing
//! anything — and it is not possible at all without this permission.
//!
//! A disassociation the product cannot perform is a commitment it should not
//! make. So the permission is asked for up front, with its reason attached to
//! it in the same value ([`crate::GrantedPermission`]), so that the consent
//! screen FR-036e requires cannot show the ask without showing the why.

use serde::Deserialize;

use crate::{
    ConnectedRepository, GrantHandoff, GrantedPermission, RepoHost, RepoHostError,
    RepositoryCredential, TokenExchange, UnixSeconds, jwt, token,
};

/// What the user is asked to grant, and why — in the order a consent screen
/// should read them.
///
/// See the module documentation for the argument behind the second entry.
pub const REQUESTED_PERMISSIONS: &[GrantedPermission] = &[
    GrantedPermission {
        id: "contents:write",
        summary: "Read and write the files in this repository",
        reason: "This is how your world's lore is mirrored: entries are written as \
                 files and committed to the repository you choose.",
    },
    GrantedPermission {
        id: "issues:write",
        summary: "Open issues on this repository",
        reason: "If content that was mirrored here is ever disabled by a moderation \
                 action and this repository is public, we open a single issue saying \
                 we have disabled it on our side and no longer associate ourselves \
                 with the copy here. We never delete, edit, or force-push anything in \
                 your repository, and we never reproduce the content in that issue.",
    },
];

/// The host GitHub serves the installation hand-off from.
pub const DEFAULT_WEB_BASE: &str = "https://github.com";

/// The host the token exchange is performed against.
pub const DEFAULT_API_BASE: &str = "https://api.github.com";

/// The installation reference: the opaque half of a grant.
///
/// A newtype rather than a bare `u64` so that FR-004c is enforced by the type
/// system and not by discipline. A consumer holds one of these, persists it
/// via `Display`, reads it back via `FromStr`, and hands it to
/// [`RepoHost::token_exchange`] — and at no point can it read the number,
/// because the field is private and there is no accessor. "No component
/// beyond the grant may read an installation identifier" stops being a rule
/// somebody has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstallationRef(u64);

impl std::fmt::Display for InstallationRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for InstallationRef {
    type Err = RepoHostError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse()
            .map(InstallationRef)
            .map_err(|_| RepoHostError::MalformedResponse("installation reference".into()))
    }
}

/// The installation payload, as GitHub describes it on return from a grant.
///
/// Deliberately tolerant of unknown fields: GitHub's installation object is
/// large and changes, and none of the rest of it is any of this feature's
/// business.
#[derive(Debug, Clone, Deserialize)]
pub struct InstallationPayload {
    pub id: u64,
    /// `"all"` or `"selected"`. Its absence is treated as `"selected"` only
    /// after the repository list has been checked, never as permission.
    #[serde(default)]
    pub repository_selection: Option<String>,
    #[serde(default)]
    pub repositories: Vec<RepositoryPayload>,
}

/// One repository in an installation payload.
#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryPayload {
    /// `owner/name`.
    pub full_name: String,
    /// GitHub reports the inverse (`private`), and it is recorded here as
    /// visibility because that is the fact FR-037a and FR-040a are about, and
    /// a negation carried through four layers is a negation somebody
    /// eventually drops.
    #[serde(default)]
    pub private: bool,
}

/// A registered GitHub App, ready to arrange grants.
///
/// Construction validates the private key, so an instance learns its
/// registration is unusable at startup rather than when a Game Master presses
/// "connect" — the diagnostic posture FR-036c asks for, and the same one
/// spec 007 requires of a partially-configured OAuth provider.
pub struct GitHubApp {
    app_id: String,
    /// The App's URL slug, which is what the installation hand-off URL is
    /// built from. Distinct from `app_id`: GitHub uses the issuer identifier for the
    /// assertion's issuer and the slug for the web URL, and they are not
    /// interchangeable.
    app_slug: String,
    signing_key: jsonwebtoken::EncodingKey,
    web_base: String,
    api_base: String,
}

/// Redacted: the struct holds a live RSA signing key for the whole instance,
/// and a derived `Debug` would put its bytes in the first `tracing` call that
/// formatted anything containing an app.
impl std::fmt::Debug for GitHubApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubApp")
            .field("app_id", &self.app_id)
            .field("app_slug", &self.app_slug)
            .field("signing_key", &"<redacted>")
            .field("web_base", &self.web_base)
            .field("api_base", &self.api_base)
            .finish()
    }
}

impl GitHubApp {
    /// Register the operator's application with this crate.
    ///
    /// Fails if the identifier or slug is blank, or if the private key is not
    /// a usable RSA key. Those are the three ways a registration is
    /// incomplete, and all three are answerable by the operator from the
    /// message alone.
    pub fn new(
        app_id: impl Into<String>,
        app_slug: impl Into<String>,
        private_key_pem: &[u8],
    ) -> Result<Self, RepoHostError> {
        let app_id = app_id.into();
        let app_slug = app_slug.into();
        if app_id.trim().is_empty() {
            return Err(RepoHostError::MissingAppId);
        }
        if app_slug.trim().is_empty() {
            return Err(RepoHostError::NotConfigured);
        }

        Ok(Self {
            signing_key: jwt::encoding_key_from_pem(private_key_pem)?,
            app_id,
            app_slug,
            web_base: DEFAULT_WEB_BASE.to_string(),
            api_base: DEFAULT_API_BASE.to_string(),
        })
    }

    /// Point this app at a GitHub Enterprise Server instead of github.com.
    ///
    /// Present because an operator running their own GitHub is an ordinary
    /// deployment, and because a hard-coded hostname is the kind of thing that
    /// is only ever discovered to be a problem by the person who cannot work
    /// around it.
    pub fn with_bases(mut self, web_base: impl Into<String>, api_base: impl Into<String>) -> Self {
        self.web_base = web_base.into().trim_end_matches('/').to_string();
        self.api_base = api_base.into().trim_end_matches('/').to_string();
        self
    }
}

/// Is this a value that can be placed in a URL query without escaping?
///
/// The unreserved set of RFC 3986 §2.3. See
/// [`GitHubApp::grant_handoff`] for why this is a gate rather than an escaper.
fn is_url_safe_state(state: &str) -> bool {
    !state.is_empty()
        && state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
}

impl RepoHost for GitHubApp {
    type Grant = InstallationRef;

    fn requested_permissions(&self) -> &'static [GrantedPermission] {
        REQUESTED_PERMISSIONS
    }

    /// Build the installation hand-off URL.
    ///
    /// The `state` is **validated and refused**, not percent-encoded. That is
    /// a deliberate inversion of the usual advice, and the reasoning is:
    /// `state` is a value this platform generates for itself — a random
    /// anti-forgery token — so a `state` containing a `&` is not a user
    /// supplying awkward input, it is our own code having generated something
    /// it should not have. Escaping it would let that bug travel; refusing it
    /// stops it here, and it costs no dependency to do so. (This is the one
    /// place this crate would otherwise need a URL encoder, which is why the
    /// trade is worth naming.)
    fn grant_handoff(&self, state: &str) -> Result<GrantHandoff, RepoHostError> {
        if !is_url_safe_state(state) {
            return Err(RepoHostError::UnsafeHandoffState);
        }

        Ok(GrantHandoff {
            url: format!(
                "{}/apps/{}/installations/new?state={state}",
                self.web_base, self.app_slug
            ),
            permissions: REQUESTED_PERMISSIONS,
        })
    }

    /// Read the installation payload, and refuse anything broader than one
    /// repository.
    ///
    /// FR-036a in code. Three refusals, in the order that matters:
    ///
    /// 1. An account-wide selection (`"all"`) is refused first, before the
    ///    repository list is consulted at all. An account-wide grant can
    ///    legitimately arrive with an empty `repositories` array, and reading
    ///    that as "zero repositories" would describe the single most dangerous
    ///    outcome with the mildest error.
    /// 2. More than one repository is refused. The user was asked to connect
    ///    one world to one repository; a wider grant is not something to
    ///    narrow after the fact, because a grant we hold and promise not to
    ///    use is still a grant we hold.
    /// 3. Zero is refused too. A connection with no repository fails on its
    ///    first push, and failing here says why.
    fn validate_grant(
        &self,
        body: &str,
    ) -> Result<(Self::Grant, ConnectedRepository), RepoHostError> {
        let payload: InstallationPayload = serde_json::from_str(body)
            .map_err(|e| RepoHostError::MalformedResponse(e.to_string()))?;

        if payload
            .repository_selection
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("all"))
        {
            return Err(RepoHostError::GrantCoversAllRepositories);
        }

        let [repository] = payload.repositories.as_slice() else {
            return Err(RepoHostError::GrantNotSingleRepository {
                count: payload.repositories.len(),
            });
        };

        let (owner, name) = repository.full_name.split_once('/').ok_or_else(|| {
            RepoHostError::MalformedResponse(
                "repository full_name is not in owner/name form".into(),
            )
        })?;
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return Err(RepoHostError::MalformedResponse(
                "repository full_name is not in owner/name form".into(),
            ));
        }

        Ok((
            InstallationRef(payload.id),
            ConnectedRepository {
                owner: owner.to_string(),
                name: name.to_string(),
                public: !repository.private,
            },
        ))
    }

    fn token_exchange(
        &self,
        grant: &Self::Grant,
        now: UnixSeconds,
    ) -> Result<TokenExchange, RepoHostError> {
        let claims = jwt::build_claims(&self.app_id, now, jwt::DEFAULT_JWT_LIFETIME_SECS)?;
        Ok(TokenExchange {
            url: format!("{}/app/installations/{grant}/access_tokens", self.api_base),
            assertion: jwt::sign_claims(&self.signing_key, &claims)?,
        })
    }

    fn credential_from_exchange(&self, body: &str) -> Result<RepositoryCredential, RepoHostError> {
        token::parse_exchange_response(body)
    }
}
