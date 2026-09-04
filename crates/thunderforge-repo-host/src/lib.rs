//! Arranging access to a repository a user granted us, as pure functions.
//!
//! Nothing here performs I/O, reads a clock, or requires a registered
//! application to exist. The server owns the one HTTP call this capability
//! makes — exchanging a signed assertion for a short-lived installation
//! credential — and the cache that credential lands in; this crate owns every
//! decision made around them.
//!
//! That split is not tidiness, and it is worth being explicit about what it
//! buys. The rules that actually matter here are arithmetic and validation:
//! *is this cached credential close enough to expiry that we should refresh
//! it*, and *does this grant cover exactly the one repository the user chose*.
//! Both are cheap to get subtly wrong and expensive to discover wrong in
//! production. Keeping them on this side of the boundary means
//! `cargo test -p thunderforge_repo_host` exercises them against generated
//! input with no network and no GitHub App configured, which is the
//! difference between a rule being tested and a rule being hoped for.
//! `thunderforge-axum-oauth` sets the precedent; see
//! `specs/034-lore-git-sync/research.md` R5a.
//!
//! # Where the host boundary physically is
//!
//! [`RepositoryCredential`] is the boundary FR-004c describes. It carries a
//! token and an expiry and *nothing that names a host* — no installation
//! identifier, no application id, no host-shaped enum. Everything downstream
//! of the grant (the connection record, path mapping, commit synthesis,
//! divergence detection) can use one without knowing how it was obtained,
//! because there is nothing in the type to branch on.
//!
//! The host-specific residue that genuinely must survive the grant — the
//! installation reference the token exchange needs — lives behind
//! [`RepoHost::Grant`], an associated type callers hold and hand back without
//! ever looking inside. A caller cannot read an installation id out of it
//! because the neutral surface gives it no way to name one.
//!
//! # Time
//!
//! Every timestamp in this crate is [`UnixSeconds`], a plain `u64` of seconds
//! since the Unix epoch. That is a deliberate choice over `chrono`,
//! `time`, or `SystemTime`:
//!
//! - It adds no dependency. The whole point of this crate is that it is cheap
//!   to compile and test; a date-time library would be carried by every
//!   consumer to express a number this crate only ever compares and adds to.
//! - It is `proptest`-generable directly, so the refresh-window arithmetic can
//!   be tested across the boundaries that break it (a clock past the expiry, a
//!   margin larger than the lifetime, a value near `u64::MAX`) simply by
//!   generating `u64`s.
//! - It cannot represent a local time zone, which is correct: none of these
//!   values has one. They are instants.
//!
//! Unsigned rather than signed is also on purpose. A negative instant is not a
//! thing this crate can encounter — the epoch is decades past — and choosing
//! `u64` means every subtraction has to be written as a `saturating_sub` or a
//! comparison, which is exactly the arithmetic that would otherwise wrap and
//! silently declare an expired credential fresh.

pub mod github;
pub mod jwt;
pub mod token;

/// Seconds since the Unix epoch. See the module documentation for why this,
/// and not a date-time type.
pub type UnixSeconds = u64;

/// A credential for a connected repository, and when it stops working.
///
/// This is the type FR-004c is about. It deliberately has no host field, no
/// installation reference and no provenance: a consumer holding one can push a
/// commit with it and can tell whether it is stale, and can learn nothing else.
///
/// It is also deliberately not `Serialize`, not `Display` and not derived
/// `Debug`. FR-035 requires that these never appear in logs, and the reliable
/// way to enforce that is to make the obvious ways of printing one either
/// impossible or redacted, rather than to rely on every future call site
/// remembering. [`RepositoryCredential::token`] exists and is the single place
/// where reading the secret is an explicit act.
#[derive(Clone, PartialEq, Eq)]
pub struct RepositoryCredential {
    token: String,
    expires_at: UnixSeconds,
}

impl RepositoryCredential {
    /// Build a credential, refusing an empty token.
    ///
    /// An empty token deserializes perfectly cleanly and would be sent as a
    /// bearer token one step later, turning a broken exchange response into an
    /// unexplained 401 from the host at push time — far from the thing that
    /// actually went wrong. Rejecting it here keeps the failure next to its
    /// cause.
    pub fn new(token: impl Into<String>, expires_at: UnixSeconds) -> Result<Self, RepoHostError> {
        let token = token.into();
        if token.is_empty() {
            return Err(RepoHostError::EmptyCredential);
        }
        Ok(Self { token, expires_at })
    }

    /// The secret itself. Every call site is a deliberate disclosure.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// The instant after which the host will stop honouring this token.
    pub fn expires_at(&self) -> UnixSeconds {
        self.expires_at
    }

    /// Has this credential already lapsed at `now`?
    ///
    /// Distinct from [`token::needs_refresh`], which asks the more useful
    /// question — *is it close enough to expiry that we should not start work
    /// with it*. This one is the bare fact, and exists mostly so a caller
    /// diagnosing a rejected push can tell "the token had expired" apart from
    /// "the host rejected a live token", which are different problems with
    /// different answers.
    pub fn is_expired_at(&self, now: UnixSeconds) -> bool {
        now >= self.expires_at
    }
}

/// Redacted on purpose: FR-035 says a credential must never appear in a log,
/// and a derived `Debug` would put one in the first `tracing` call that
/// formatted a struct containing it.
impl std::fmt::Debug for RepositoryCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositoryCredential")
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// The one repository a connection covers.
///
/// Host-neutral by construction: an owner and a name is how every repository
/// host this feature could plausibly reach names a repository, and it is the
/// only host-shaped fact the rest of the system is allowed to know
/// (FR-004b — the seam begins after the grant, and this is what comes out of
/// it alongside the credential).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedRepository {
    pub owner: String,
    pub name: String,
    /// Whether the host reported the repository as publicly visible **at the
    /// moment of the grant**.
    ///
    /// FR-040a is emphatic that this is an observation and not a guarantee —
    /// visibility can be changed at the host at any time without telling us —
    /// so it is recorded here as what was seen, and every surface that shows
    /// it must describe it that way.
    pub public: bool,
}

impl ConnectedRepository {
    /// `owner/name`, the form both humans and hosts use.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// One permission the user is being asked to grant, and why.
///
/// The `reason` field is not decoration. FR-036 requires that the user be
/// shown what access is being granted before they grant it, and FR-036e
/// requires that the *second* permission — the ability to open an issue —
/// be shown **with its reason**, because it is wider than "write the files we
/// mirror" and a user who is not told why would be right to be suspicious.
/// Carrying the reason in the same value as the permission is what stops a
/// consent screen from listing one without the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantedPermission {
    /// The host's own name for the permission, for an operator reading the
    /// host's audit log.
    pub id: &'static str,
    /// What it allows, in the user's words.
    pub summary: &'static str,
    /// Why this feature asks for it.
    pub reason: &'static str,
}

/// Everything the server needs to perform the token exchange, and nothing it
/// needs to understand.
///
/// The server POSTs to `url` with `assertion` as its bearer credential and
/// hands the response body back to [`RepoHost::credential_from_exchange`]. It
/// does not parse the response, does not know what an installation is, and
/// does not construct the URL — which is what keeps `src/server` from growing
/// host knowledge one convenience at a time.
#[derive(Clone, PartialEq, Eq)]
pub struct TokenExchange {
    pub url: String,
    /// The signed application assertion, presented as a bearer token.
    pub assertion: String,
}

/// Redacted for the same reason [`RepositoryCredential`]'s is: the assertion
/// is a live signing artefact for the whole instance, not just one connection.
impl std::fmt::Debug for TokenExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenExchange")
            .field("url", &self.url)
            .field("assertion", &"<redacted>")
            .finish()
    }
}

/// Where a user is sent to grant access, and what they will be asked for.
///
/// The permissions travel with the URL rather than being looked up separately,
/// so a consent screen physically cannot render the hand-off without having
/// the list FR-036/FR-036e require it to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantHandoff {
    pub url: String,
    pub permissions: &'static [GrantedPermission],
}

/// A repository host this instance can arrange access to.
///
/// The trait exists because `lore_sync` is this capability's first consumer
/// and not its owner (research R5a): everything a consumer needs is expressed
/// here in host-neutral terms, so a second host — or a second consumer — costs
/// an implementation rather than an excavation.
///
/// The flow is four steps, and the caller supplies the effects between them:
///
/// 1. [`RepoHost::grant_handoff`] — build the URL the user is sent to, with
///    the permission list they must be shown first.
/// 2. [`RepoHost::validate_grant`] — read what the host says came back, refuse
///    anything broader than one repository, and yield the opaque
///    [`RepoHost::Grant`] plus the neutral [`ConnectedRepository`].
/// 3. [`RepoHost::token_exchange`] — turn that grant and a current time into a
///    request the caller performs.
/// 4. [`RepoHost::credential_from_exchange`] — turn the response body into a
///    [`RepositoryCredential`], after which nothing downstream needs this
///    trait again.
pub trait RepoHost {
    /// Whatever this host needs in order to ask for a credential later.
    ///
    /// Opaque on purpose. On GitHub this is an installation identifier, and
    /// FR-004c says no component past the grant may read one; making it an
    /// associated type means a consumer *cannot*, because it has no name for
    /// the inside of the value it is holding.
    ///
    /// The bounds are the minimum a caller needs to store one and hand it
    /// back: `Display` to persist it and `FromStr` to load it. Neither lets a
    /// caller interpret it — a stored grant is a token the caller returns, not
    /// a fact it reads.
    type Grant: Clone + std::fmt::Display + std::str::FromStr;

    /// The permissions this host implementation will ask the user for.
    ///
    /// FR-036e's second permission must appear here with its reason. A host
    /// implementation that returns only "write contents" is a host on which
    /// FR-040b's public disassociation cannot be performed, and the product
    /// should not make a commitment it cannot keep.
    fn requested_permissions(&self) -> &'static [GrantedPermission];

    /// Build the URL that hands the user off to the host to grant access.
    ///
    /// `state` is the caller's opaque anti-forgery value, echoed back by the
    /// host on return.
    fn grant_handoff(&self, state: &str) -> Result<GrantHandoff, RepoHostError>;

    /// Read the host's description of what was granted.
    ///
    /// This is where FR-036a is enforced: a grant covering more than the one
    /// repository the user chose is refused outright rather than narrowed
    /// after the fact, because a grant we hold and promise not to use is still
    /// a grant we hold.
    fn validate_grant(
        &self,
        body: &str,
    ) -> Result<(Self::Grant, ConnectedRepository), RepoHostError>;

    /// Describe the request that turns a grant into a live credential.
    ///
    /// `now` is passed in rather than read, which is the whole reason this is
    /// testable: the assertion's validity window is a pure function of the
    /// time the caller supplies.
    fn token_exchange(
        &self,
        grant: &Self::Grant,
        now: UnixSeconds,
    ) -> Result<TokenExchange, RepoHostError>;

    /// Parse the exchange response into the neutral credential type.
    ///
    /// Total: any byte sequence produces `Ok` or `Err`, never a panic. The
    /// body comes from someone else's server, and a host returning an HTML
    /// error page with a 200 must not take the process down.
    fn credential_from_exchange(&self, body: &str) -> Result<RepositoryCredential, RepoHostError>;
}

/// Why arranging repository access failed.
///
/// One enum for the whole crate rather than one per module: these all surface
/// at the same two places — the operator's configuration diagnostic (FR-036c)
/// and the Game Master's connection screen — and splitting them would mean
/// three conversions on the way to a single message.
///
/// **No variant carries a token, an assertion, or a private key**, and none
/// should ever be given one. FR-035 forbids a credential appearing in an error
/// message, and an error type is the most likely place for one to leak,
/// because errors get logged by default and successes do not.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoHostError {
    /// This instance's operator has not registered an application.
    ///
    /// FR-036b: this is not a broken feature, it is an absent one, and the
    /// surface that reports it must say so. Distinguished from every other
    /// variant for exactly that reason.
    #[error("this instance has no repository integration configured")]
    NotConfigured,

    /// The configured private key is not a usable RSA key.
    ///
    /// Detected when the application is constructed rather than at the moment
    /// a Game Master first tries to connect — FR-036c's "report whether the
    /// registration is usable" is only worth anything if the check happens
    /// before a user is standing in front of it. The detail is the library's,
    /// and describes the key's *shape*; it never contains key material.
    #[error("the configured application private key is not a usable RSA key: {0}")]
    InvalidPrivateKey(String),

    /// The application identifier is missing or blank.
    #[error("the configured application identifier is empty")]
    MissingAppId,

    /// The requested assertion lifetime is outside what the host will accept.
    #[error("an assertion valid for {requested}s was requested; the host's limit is {limit}s")]
    JwtLifetimeOutOfRange { requested: u64, limit: u64 },

    /// The supplied clock value cannot have a validity window built around it.
    ///
    /// Only reachable within a few minutes of `u64::MAX`, i.e. never in
    /// practice — but it is the alternative to a wrapping addition producing
    /// an assertion whose expiry is before its issue time, which a host would
    /// reject with a message nobody could interpret.
    #[error("clock value {now} is too far in the future to build an assertion around")]
    ClockOutOfRange { now: UnixSeconds },

    /// Signing failed after the key parsed.
    #[error("could not sign the application assertion: {0}")]
    Signing(String),

    /// The host's response was not the JSON this exchange expects.
    #[error("the repository host returned a response this exchange cannot read: {0}")]
    MalformedResponse(String),

    /// The host returned a credential whose token is the empty string.
    #[error("the repository host returned an empty credential")]
    EmptyCredential,

    /// The expiry timestamp could not be read.
    #[error("could not read the expiry timestamp {value:?}: {reason}")]
    UnreadableExpiry { value: String, reason: &'static str },

    /// The grant covers a number of repositories other than one.
    ///
    /// FR-036a. Zero is as much a failure as five: a connection with no
    /// repository is a connection that will fail on its first push, and
    /// failing here says why.
    #[error("the grant covers {count} repositories; a connection must cover exactly one")]
    GrantNotSingleRepository { count: usize },

    /// The grant covers every repository in the account.
    ///
    /// Its own variant rather than a large `count`, because the host reports
    /// this as a *selection mode* and not as a list — an account-wide grant
    /// may well arrive with an empty `repositories` array, and reading that as
    /// "zero repositories" would describe the most dangerous case with the
    /// mildest message.
    #[error("the grant covers every repository in the account, which this feature refuses")]
    GrantCoversAllRepositories,

    /// The hand-off state contains characters that cannot go in a URL as-is.
    ///
    /// See [`github::GitHubApp::grant_handoff`] for why this is a refusal
    /// rather than an escape.
    #[error("the grant hand-off state contains characters that cannot be placed in a URL")]
    UnsafeHandoffState,
}
