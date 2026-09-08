//! One GitHub application per subsystem, or one for all of them — resolved
//! field by field, with a record of where each value came from.
//!
//! # Why this is beside [`super`] rather than inside it
//!
//! Research § R2 is explicit that **one module speaks to the host**, and this
//! is a child of that module rather than a second one: `repo_host::scoped`.
//! It is a separate *file* only because `repo_host.rs` was already 723 lines
//! and `scripts/check-file-length.sh` caps a source file at 1,000 — a limit
//! that exists so a file full of logic stays testable, and splitting along
//! "the sync app's five constants" versus "any subsystem's application" is
//! the seam that was already there.
//!
//! # Resolution is per field, and that is the requirement
//!
//! FR-025 says the subsystem-specific value wins and the global one serves
//! what is left. Resolving whole *sets* would satisfy that sentence and break
//! its point: a feedback application with a slug and no key would silently
//! fall back to the global application's identity **and** the global
//! application's key, which is a third application neither the operator nor
//! the product ever chose. So each of the three fields resolves on its own and
//! [`ScopedApp::sources`] records which declaration supplied it, which is what
//! makes FR-029's "a half-configured application is never silently completed"
//! observable rather than asserted.
//!
//! # Where the values come from
//!
//! `crate::settings::registry` already declares all nine keys —
//! `github_app.{global,sync,feedback}.{client_id,slug,private_key}` — with
//! their environment variables and aliases, and `settings::resolver` already
//! implements "the environment beats the instance's store beats the default".
//! This module consumes those declarations; it does not read the environment
//! itself, and it declares nothing of its own. A tenth variable name appearing
//! here would be a variable no readiness surface knows about.
//!
//! # The private key still arrives in three forms
//!
//! The declaration for a private key names `..._PRIVATE_KEY` and aliases
//! `..._PRIVATE_KEY_FILE` and `..._PRIVATE_KEY_BASE64`, and the resolver hands
//! back whatever the variable in use contained — a path, base64, or the PEM.
//! Which of those it is, is decided by [`Resolved::fixed_by`], the variable
//! that actually set it, rather than by sniffing the value: sniffing is how
//! `"not a key at all"` turned out to be valid base64, which
//! [`super::normalise_pem`] documents at length.
//!
//! And it is parsed **here**, at resolution time, not at first use (FR-028) —
//! a key that is present but is not a key is the failure a presence check
//! calls "configured".

use base64::Engine as _;
use base64::engine::general_purpose;
use thunderforge_repo_host::github::GitHubApp;
use thunderforge_repo_host::{RepoHost, RepositoryCredential};

use crate::settings::resolver::{Resolved, Settings};

/// Which subsystem is asking. `Global` is not a subsystem — it is the
/// fallback every other scope draws on — but it resolves like one so that an
/// operator surface can report it the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppScope {
    Global,
    Sync,
    Feedback,
}

impl AppScope {
    pub fn as_str(self) -> &'static str {
        match self {
            AppScope::Global => "global",
            AppScope::Sync => "sync",
            AppScope::Feedback => "feedback",
        }
    }

    /// What an operator is told this application is used for. FR-026: the
    /// global application's *scope* is an explanation, not a field.
    pub fn serves(self) -> &'static str {
        match self {
            AppScope::Global => {
                "every subsystem that talks to GitHub — lore synchronisation \
                 and feedback today, and anything added later"
            }
            AppScope::Sync => "lore synchronisation",
            AppScope::Feedback => "feedback",
        }
    }
}

/// One of the three values an application is described by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    ClientId,
    Slug,
    PrivateKey,
}

impl Field {
    pub fn as_str(self) -> &'static str {
        match self {
            Field::ClientId => "client_id",
            Field::Slug => "slug",
            Field::PrivateKey => "private_key",
        }
    }
}

/// The declared setting key for one scope and one field.
///
/// A table rather than string concatenation, because a `&'static str` is what
/// the settings registry deals in and a `format!` here would produce a key
/// that no declaration matches the moment somebody renames one.
pub fn setting_key(scope: AppScope, field: Field) -> &'static str {
    match (scope, field) {
        (AppScope::Global, Field::ClientId) => "github_app.global.client_id",
        (AppScope::Global, Field::Slug) => "github_app.global.slug",
        (AppScope::Global, Field::PrivateKey) => "github_app.global.private_key",
        (AppScope::Sync, Field::ClientId) => "github_app.sync.client_id",
        (AppScope::Sync, Field::Slug) => "github_app.sync.slug",
        (AppScope::Sync, Field::PrivateKey) => "github_app.sync.private_key",
        (AppScope::Feedback, Field::ClientId) => "github_app.feedback.client_id",
        (AppScope::Feedback, Field::Slug) => "github_app.feedback.slug",
        (AppScope::Feedback, Field::PrivateKey) => "github_app.feedback.private_key",
    }
}

/// Where one field's value actually came from. FR-029 renders this verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSource {
    pub field: Field,
    /// The declaration that supplied it — the subsystem's own, or the global.
    pub key: &'static str,
    pub scope: AppScope,
    /// The environment variable that fixed it, when one did.
    pub fixed_by: Option<&'static str>,
}

/// A resolved application, and the record of how it was assembled.
pub struct ScopedApp {
    pub app: GitHubApp,
    pub scope: AppScope,
    pub sources: Vec<FieldSource>,
}

/// Why an application could not be resolved.
///
/// **No variant carries a value, a fragment of one, or its length** (FR-027,
/// FR-021). Every one names declarations an operator can go and set, which is
/// the same contract `settings::registry` keeps and the reason this reports
/// setting keys rather than inventing a second vocabulary of variable names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialProblem {
    Missing {
        field: Field,
        specific: &'static str,
        global: &'static str,
    },
    /// The value was a path, and the path could not be read. The path is named
    /// because an operator chose it and it is not the secret; the file's
    /// contents are never mentioned.
    UnreadableKeyFile {
        key: &'static str,
        path: String,
        detail: String,
    },
    /// `..._PRIVATE_KEY_BASE64` was set to something that is not base64.
    /// Reported against that variable rather than falling through, because the
    /// operator declared an encoding and deserves to be told it was wrong.
    UndecodableBase64 { key: &'static str, detail: String },
    /// Present, and not a key. The case a presence check calls "configured".
    UnreadableKey { key: &'static str, detail: String },
}

impl CredentialProblem {
    /// The setting key an operator should go and look at.
    pub fn key(&self) -> &'static str {
        match self {
            CredentialProblem::Missing { specific, .. } => specific,
            CredentialProblem::UnreadableKeyFile { key, .. }
            | CredentialProblem::UndecodableBase64 { key, .. }
            | CredentialProblem::UnreadableKey { key, .. } => key,
        }
    }

    /// What to do about it, naming declarations and never values.
    pub fn guidance(&self) -> String {
        match self {
            CredentialProblem::Missing {
                field,
                specific,
                global,
            } => format!(
                "Neither `{specific}` nor `{global}` is set, so this instance has no {} \
                 for that application. Set the first to give this subsystem its own \
                 application, or the second to share one.",
                match field {
                    Field::ClientId => "client ID",
                    Field::Slug => "URL slug",
                    Field::PrivateKey => "private key",
                }
            ),
            CredentialProblem::UnreadableKeyFile { key, path, detail } => {
                format!("`{key}` names the file {path}, which could not be read ({detail}).")
            }
            CredentialProblem::UndecodableBase64 { key, detail } => format!(
                "`{key}` was given in its base64 form and is not valid base64 ({detail}). \
                 It must be the PEM file encoded whole — for example `base64 -w0 app-key.pem`."
            ),
            CredentialProblem::UnreadableKey { key, detail } => format!(
                "`{key}` is set but could not be read as an RSA private key ({detail}). \
                 Accepted forms are a PEM, a path to one, or the PEM base64-encoded."
            ),
        }
    }
}

/// Every problem's guidance, as one sentence an operator can act on in a pass.
pub fn join_problems(problems: &[CredentialProblem]) -> String {
    problems
        .iter()
        .map(CredentialProblem::guidance)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The declarations to name when nothing is configured — keys only, never
/// values, which is what makes this safe to render on an operator surface.
pub fn missing_keys(problems: &[CredentialProblem]) -> Vec<&'static str> {
    problems.iter().map(CredentialProblem::key).collect()
}

/// One field, resolved from the subsystem's declaration or the global one.
///
/// Returns the resolved setting *and* which scope answered, because the second
/// is the half FR-029 is about.
fn field_of(
    settings: &Settings,
    scope: AppScope,
    field: Field,
) -> Option<(&Resolved, AppScope, &'static str)> {
    let specific = setting_key(scope, field);
    if let Some(resolved) = settings.get(specific).filter(|r| r.is_set()) {
        return Some((resolved, scope, specific));
    }
    if scope == AppScope::Global {
        return None;
    }
    let global = setting_key(AppScope::Global, field);
    settings
        .get(global)
        .filter(|r| r.is_set())
        .map(|resolved| (resolved, AppScope::Global, global))
}

/// Resolve the application one subsystem should act as.
///
/// Returns **every** problem rather than the first, so an operator fixes their
/// configuration in one pass instead of discovering the next gap after each
/// restart — the posture [`super::registration_from_env`] already takes.
pub fn registration_for(
    scope: AppScope,
    settings: &Settings,
) -> Result<ScopedApp, Vec<CredentialProblem>> {
    let mut problems = Vec::new();
    let mut sources = Vec::new();

    let mut plain = |field: Field, problems: &mut Vec<CredentialProblem>| -> Option<String> {
        match field_of(settings, scope, field) {
            Some((resolved, from, key)) => {
                sources.push(FieldSource {
                    field,
                    key,
                    scope: from,
                    fixed_by: resolved.fixed_by,
                });
                resolved.value.clone()
            }
            None => {
                problems.push(CredentialProblem::Missing {
                    field,
                    specific: setting_key(scope, field),
                    global: setting_key(AppScope::Global, field),
                });
                None
            }
        }
    };

    let client_id = plain(Field::ClientId, &mut problems);
    let slug = plain(Field::Slug, &mut problems);

    let key_pem = match field_of(settings, scope, Field::PrivateKey) {
        None => {
            problems.push(CredentialProblem::Missing {
                field: Field::PrivateKey,
                specific: setting_key(scope, Field::PrivateKey),
                global: setting_key(AppScope::Global, Field::PrivateKey),
            });
            None
        }
        Some((resolved, from, key)) => {
            sources.push(FieldSource {
                field: Field::PrivateKey,
                key,
                scope: from,
                fixed_by: resolved.fixed_by,
            });
            match private_key_bytes(resolved, key) {
                Ok(bytes) => Some(bytes),
                Err(problem) => {
                    problems.push(problem);
                    None
                }
            }
        }
    };

    let (Some(client_id), Some(slug), Some(key_pem)) = (client_id, slug, key_pem) else {
        return Err(problems);
    };

    // Parsed now, not at first use (FR-028).
    match GitHubApp::new(client_id, slug, &key_pem) {
        Ok(app) if problems.is_empty() => Ok(ScopedApp {
            app,
            scope,
            sources,
        }),
        Ok(_) => Err(problems),
        Err(e) => {
            problems.push(CredentialProblem::UnreadableKey {
                key: setting_key(scope, Field::PrivateKey),
                detail: e.to_string(),
            });
            Err(problems)
        }
    }
}

/// The PEM bytes behind a resolved private-key setting.
///
/// **The form is decided by the variable that set it, not by the value.** A
/// value that arrived through `..._PRIVATE_KEY_FILE` is a path; one that
/// arrived through `..._PRIVATE_KEY_BASE64` is base64 and a failure to decode
/// is reported against that variable; anything else — including a value stored
/// in the instance's own settings — goes through
/// [`super::normalise_pem`], which accepts a PEM, literal `\n` escapes, or
/// base64 *whose result is a PEM*.
fn private_key_bytes(resolved: &Resolved, key: &'static str) -> Result<Vec<u8>, CredentialProblem> {
    let raw = resolved.value.clone().unwrap_or_default();
    let raw = raw.trim();
    match resolved.fixed_by {
        Some(var) if var.ends_with("_PRIVATE_KEY_FILE") => {
            std::fs::read(raw).map_err(|e| CredentialProblem::UnreadableKeyFile {
                key,
                path: raw.to_string(),
                detail: e.to_string(),
            })
        }
        Some(var) if var.ends_with("_PRIVATE_KEY_BASE64") => {
            let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
            general_purpose::STANDARD.decode(&compact).map_err(|e| {
                CredentialProblem::UndecodableBase64 {
                    key,
                    detail: e.to_string(),
                }
            })
        }
        _ => Ok(super::normalise_pem(raw)),
    }
}

// ============================================================================
// The effects half, scoped: the calls delivery makes and lore sync does not.
// ============================================================================

/// A failed call, in the only terms this product is willing to keep.
///
/// It carries a status code and nothing else from the host. **No response
/// body, ever** — FR-021 forbids a reason that could disclose a credential or
/// a fragment of one, and a free-text field carrying a host body is one 401
/// payload away from breaking that, invisibly, on a day nobody is looking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFailure {
    /// `None` for a transport failure — no response arrived at all.
    pub status: Option<u16>,
    /// True when nothing left this process. Kept apart from a 5xx because the
    /// two are the same *for retry purposes* and different for diagnosis.
    pub transport: bool,
}

impl HostFailure {
    fn transport() -> Self {
        HostFailure {
            status: None,
            transport: true,
        }
    }

    fn status(code: u16) -> Self {
        HostFailure {
            status: Some(code),
            transport: false,
        }
    }

    /// Whether the host **may have acted** despite the failure.
    ///
    /// This is the whole of research § R4's second mechanism. A 4xx means the
    /// host declined and created nothing, so a retry creates directly; a 5xx
    /// or a transport failure means the request may have succeeded and the
    /// answer was lost, so a retry searches first.
    pub fn ambiguous(&self) -> bool {
        match self.status {
            None => true,
            Some(code) => code >= 500,
        }
    }
}

/// An issue that exists at the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostedIssue {
    pub number: i32,
    pub html_url: String,
}

const USER_AGENT: &str = "ThunderForgeVTT";

fn client() -> Result<reqwest::Client, HostFailure> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| HostFailure::transport())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Exchange an installation identifier for a short-lived credential, as the
/// application this scope resolved to.
///
/// The scoped sibling of [`super::installation_credential`], which is
/// hard-wired to the sync application because it calls
/// `registration_from_env()` itself. Not cached, for the reason that function
/// gives.
pub async fn installation_credential(
    app: &ScopedApp,
    installation_id: &str,
) -> Result<RepositoryCredential, HostFailure> {
    let reference: <GitHubApp as RepoHost>::Grant = installation_id
        .parse()
        .map_err(|_| HostFailure::status(404))?;

    let exchange = app
        .app
        .token_exchange(&reference, now_secs())
        .map_err(|_| HostFailure::status(401))?;

    let response = client()?
        .post(&exchange.url)
        .bearer_auth(&exchange.assertion)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| HostFailure::transport())?;

    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|_| HostFailure::transport())?;
    if !(200..300).contains(&status) {
        return Err(HostFailure::status(status));
    }

    app.app
        .credential_from_exchange(&body)
        .map_err(|_| HostFailure::status(status))
}

/// One JSON request, with the status code kept.
///
/// `super::open_issue` never checks `response.status()` — it parses the body
/// and infers failure from a missing `html_url`. For lore sync's occasional
/// disassociation notice that is survivable; for a retry loop it is not, since
/// a 403 secondary rate limit and a 422 validation error are permanently
/// different situations and a loop that cannot tell them apart either hammers
/// the host or abandons a submission that would have succeeded.
async fn json_call(request: reqwest::RequestBuilder) -> Result<serde_json::Value, HostFailure> {
    let response = request
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| HostFailure::transport())?;
    let status = response.status().as_u16();
    let body: serde_json::Value = response.json().await.unwrap_or(serde_json::Value::Null);
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(HostFailure::status(status))
    }
}

fn issue_from(value: &serde_json::Value) -> Option<PostedIssue> {
    Some(PostedIssue {
        number: value.get("number").and_then(|v| v.as_i64())? as i32,
        html_url: value.get("html_url").and_then(|v| v.as_str())?.to_string(),
    })
}

/// Create an issue, with its labels set in the same request.
///
/// Labels are part of the creation POST rather than a second call, so a
/// failure between creating and labelling cannot exist — FR-020 asks that each
/// kind be distinguishable at the destination without reading the body, and an
/// issue that exists unlabelled because the second call failed would not be.
pub async fn create_issue(
    app: &ScopedApp,
    credential: &RepositoryCredential,
    owner: &str,
    name: &str,
    title: &str,
    body: &str,
    labels: &[String],
) -> Result<PostedIssue, HostFailure> {
    let created = json_call(
        client()?
            .post(format!("{}/issues", app.app.repository_url(owner, name)))
            .bearer_auth(credential.token())
            .json(&serde_json::json!({
                "title": title,
                "body": body,
                "labels": labels,
            })),
    )
    .await?;

    issue_from(&created).ok_or_else(|| HostFailure::status(502))
}

/// Look for an issue this instance already created, by the key it wrote into
/// the body.
///
/// `Ok(None)` means the host answered and found nothing, which is what lets a
/// retry create with confidence. An error means the question could not be
/// asked, which is a different thing and is why this does not collapse both
/// into an `Option`.
pub async fn find_issue_by_key(
    app: &ScopedApp,
    credential: &RepositoryCredential,
    owner: &str,
    name: &str,
    delivery_key: &str,
) -> Result<Option<PostedIssue>, HostFailure> {
    let query = percent_encode(&format!("{delivery_key} repo:{owner}/{name}"));
    let found = json_call(
        client()?
            .get(format!("{}?q={query}", app.app.search_issues_url()))
            .bearer_auth(credential.token()),
    )
    .await?;

    Ok(found
        .get("items")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .find_map(issue_from))
}

/// One issue's current state — `open` or `closed`, and nothing else.
pub async fn issue_state(
    app: &ScopedApp,
    credential: &RepositoryCredential,
    owner: &str,
    name: &str,
    number: i32,
) -> Result<String, HostFailure> {
    let issue = json_call(
        client()?
            .get(format!(
                "{}/issues/{number}",
                app.app.repository_url(owner, name)
            ))
            .bearer_auth(credential.token()),
    )
    .await?;

    match issue.get("state").and_then(|v| v.as_str()) {
        Some("closed") => Ok("closed".to_string()),
        Some(_) => Ok("open".to_string()),
        None => Err(HostFailure::status(502)),
    }
}

/// Whether the destination repository is public, as the host answers today.
pub async fn repository_is_public(
    app: &ScopedApp,
    credential: &RepositoryCredential,
    owner: &str,
    name: &str,
) -> Result<bool, HostFailure> {
    let repo = json_call(
        client()?
            .get(app.app.repository_url(owner, name))
            .bearer_auth(credential.token()),
    )
    .await?;

    repo.get("private")
        .and_then(|v| v.as_bool())
        .map(|private| !private)
        .ok_or_else(|| HostFailure::status(502))
}

/// Commit one file to the attachment branch, creating the branch on first use.
///
/// A branch rather than the default branch (research § R15), so feedback
/// attachments never appear in the history of the code, never trigger a CI run,
/// and can be pruned by a maintainer independently of anything else. No new
/// permission is needed: `github::REQUESTED_PERMISSIONS` already asks for
/// `contents:write` for lore synchronisation.
///
/// Returns the blob's page URL. The *raw* URL a public repository can embed is
/// built by the caller from the same parts, because whether to embed or link
/// is a decision about visibility and belongs where visibility is known.
pub async fn put_file(
    app: &ScopedApp,
    credential: &RepositoryCredential,
    owner: &str,
    name: &str,
    branch: &str,
    path: &str,
    bytes: &[u8],
    message: &str,
) -> Result<String, HostFailure> {
    ensure_branch(app, credential, owner, name, branch).await?;

    let encoded = general_purpose::STANDARD.encode(bytes);
    json_call(
        client()?
            .put(format!(
                "{}/contents/{path}",
                app.app.repository_url(owner, name)
            ))
            .bearer_auth(credential.token())
            .json(&serde_json::json!({
                "message": message,
                "content": encoded,
                "branch": branch,
            })),
    )
    .await?;

    Ok(format!(
        "https://github.com/{owner}/{name}/blob/{branch}/{path}"
    ))
}

/// Percent-encode one query value.
///
/// Written here rather than pulled in, because `reqwest` is built in this
/// workspace with `default-features = false` and its `query` builder is not
/// among the features enabled — and adding a feature to a shared dependency to
/// encode one search string is a worse trade than eight lines. Unreserved
/// characters pass through; everything else, including the space and the colon
/// this query is made of, is escaped.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The raw URL for a file on the attachment branch. Renders inline in an
/// issue body **only for a public repository** — `raw.githubusercontent.com`
/// requires a token otherwise, and an embedded image that needs one renders as
/// a broken icon for every maintainer.
pub fn raw_url(owner: &str, name: &str, branch: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{name}/{branch}/{path}")
}

/// Create the attachment branch from the default branch's head, if it is not
/// there. A 422 from the ref-creation call is "it already exists", which is
/// the ordinary case after the first attachment and not a failure.
async fn ensure_branch(
    app: &ScopedApp,
    credential: &RepositoryCredential,
    owner: &str,
    name: &str,
    branch: &str,
) -> Result<(), HostFailure> {
    let repository_url = app.app.repository_url(owner, name);

    if json_call(
        client()?
            .get(format!("{repository_url}/git/ref/heads/{branch}"))
            .bearer_auth(credential.token()),
    )
    .await
    .is_ok()
    {
        return Ok(());
    }

    let repo = json_call(
        client()?
            .get(&repository_url)
            .bearer_auth(credential.token()),
    )
    .await?;
    let default_branch = repo
        .get("default_branch")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostFailure::status(502))?;

    let head = json_call(
        client()?
            .get(format!("{repository_url}/git/ref/heads/{default_branch}"))
            .bearer_auth(credential.token()),
    )
    .await?;
    let sha = head
        .get("object")
        .and_then(|o| o.get("sha"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostFailure::status(502))?;

    match json_call(
        client()?
            .post(format!("{repository_url}/git/refs"))
            .bearer_auth(credential.token())
            .json(&serde_json::json!({
                "ref": format!("refs/heads/{branch}"),
                "sha": sha,
            })),
    )
    .await
    {
        Ok(_) => Ok(()),
        // Somebody — or a concurrent attempt — created it between the read and
        // the write. Not a failure: the branch is what was wanted.
        Err(failure) if failure.status == Some(422) => Ok(()),
        Err(failure) => Err(failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scope_and_field_names_a_declared_setting() {
        for scope in [AppScope::Global, AppScope::Sync, AppScope::Feedback] {
            for field in [Field::ClientId, Field::Slug, Field::PrivateKey] {
                let key = setting_key(scope, field);
                assert!(
                    crate::settings::registry::declaration(key).is_some(),
                    "`{key}` is not declared in settings::registry"
                );
            }
        }
    }

    /// FR-027 and FR-021 in one assertion: a diagnostic may help an operator
    /// and must not help someone reading their logs.
    #[test]
    fn no_problem_carries_a_value_a_fragment_or_a_length() {
        let secret = "-----BEGIN RSA PRIVATE KEY-----abcdef-----END RSA PRIVATE KEY-----";
        let problems = [
            CredentialProblem::Missing {
                field: Field::PrivateKey,
                specific: setting_key(AppScope::Feedback, Field::PrivateKey),
                global: setting_key(AppScope::Global, Field::PrivateKey),
            },
            CredentialProblem::UndecodableBase64 {
                key: setting_key(AppScope::Feedback, Field::PrivateKey),
                detail: "Invalid symbol 33".to_string(),
            },
            CredentialProblem::UnreadableKey {
                key: setting_key(AppScope::Feedback, Field::PrivateKey),
                detail: "InvalidKeyFormat".to_string(),
            },
        ];
        for problem in &problems {
            let guidance = problem.guidance();
            assert!(!guidance.contains(secret), "{guidance}");
            assert!(!guidance.contains("abcdef"), "{guidance}");
            assert!(!guidance.contains("length"), "{guidance}");
            assert!(guidance.contains("github_app."), "{guidance}");
        }
    }

    /// A 4xx means the host declined and created nothing; a 5xx or a transport
    /// failure means it may have acted. The whole of "search before create,
    /// but only after an ambiguous failure" rests on this one function.
    #[test]
    fn only_a_lost_answer_is_ambiguous() {
        assert!(HostFailure::transport().ambiguous());
        assert!(HostFailure::status(500).ambiguous());
        assert!(HostFailure::status(502).ambiguous());
        assert!(!HostFailure::status(422).ambiguous());
        assert!(!HostFailure::status(403).ambiguous());
        assert!(!HostFailure::status(404).ambiguous());
    }

    /// The search query is one string with a space and a colon in it, and both
    /// have to survive being a URL. Asserted because the failure is silent: a
    /// malformed query returns no results, which reads exactly like "the issue
    /// is not there" — and that is the answer that creates a duplicate.
    #[test]
    fn a_search_query_is_escaped_rather_than_pasted_into_a_url() {
        let encoded = percent_encode("018f3c9a repo:owner/name");
        assert_eq!(encoded, "018f3c9a%20repo%3Aowner%2Fname");
        assert!(!encoded.contains(' '));
        assert_eq!(percent_encode("abc-DEF_1.2~3"), "abc-DEF_1.2~3");
    }

    #[test]
    fn the_global_scope_has_no_fallback_of_its_own() {
        let settings = Settings::default();
        let problems = registration_for(AppScope::Global, &settings)
            .err()
            .expect("nothing is configured");
        assert_eq!(problems.len(), 3);
        assert!(missing_keys(&problems).contains(&"github_app.global.client_id"));
    }

    #[test]
    fn an_unconfigured_instance_names_both_places_a_value_could_go() {
        let settings = Settings::default();
        let problems = registration_for(AppScope::Feedback, &settings)
            .err()
            .expect("nothing is configured");
        let guidance = join_problems(&problems);
        assert!(
            guidance.contains("github_app.feedback.client_id"),
            "{guidance}"
        );
        assert!(
            guidance.contains("github_app.global.client_id"),
            "{guidance}"
        );
    }
}
