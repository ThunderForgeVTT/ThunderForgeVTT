//! The calls a subsystem makes against the repository host, as the application
//! it resolved to.
//!
//! # What used to be here, and where it went
//!
//! This file was written by spec 037 and carried two halves: *which* GitHub
//! application a subsystem acts as, and *what* it then does with it. Spec 040
//! US5 owns the first half — one credential vocabulary for the whole product,
//! not one per subsystem — so `AppScope`, `Field`, `setting_key`,
//! `CredentialProblem`, `ScopedApp` and `registration_for` now live in
//! [`crate::github_apps`] and are re-exported below. Every caller keeps the
//! path it had.
//!
//! **The one behavioural change is deliberate and is the point of the move.**
//! The version here resolved *field by field*: a feedback application with a
//! slug and no key borrowed the global application's key. Spec 040 FR-021, its
//! US5 acceptance scenario 4 and research.md § R10 all require the opposite —
//! an application resolves whole or is stepped over whole — because a client
//! ID from one registration with a private key from another is not an
//! application, it is an authentication failure that reads like a bad key.
//! Spec 037's own FR-029 assumed the merging reading; its pointer now names
//! spec 040's contract, and the two agree.
//!
//! What remains here is the effects half, which is genuinely `repo_host`'s
//! job: research § R2 is explicit that **one module speaks to the host**, and
//! this is a child of that module rather than a second one.
//!
//! # Why a separate file at all
//!
//! `repo_host.rs` was already 723 lines and `scripts/check-file-length.sh`
//! caps a source file at 1,000 — a limit that exists so a file full of logic
//! stays testable.

use base64::Engine as _;
use base64::engine::general_purpose;
use thunderforge_repo_host::github::GitHubApp;
use thunderforge_repo_host::{RepoHost, RepositoryCredential};

// The credential vocabulary, owned by `github_apps` and re-exported so that
// `use crate::repo_host::scoped::{AppScope, registration_for}` keeps working.
// One vocabulary, two paths to it — not two vocabularies.
pub use crate::github_apps::{
    AppScope, CredentialProblem, Field, FieldSource, ScopedApp, acts_for, join_problems,
    missing_keys, registration_for, setting_key, stepped_over_guidance,
};

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

    // The resolution cases that used to live here — the declared-key walk, the
    // no-value-no-fragment-no-length assertion, the global scope's lack of a
    // fallback and the both-places guidance — moved with the code they test,
    // to `github_apps_tests.rs`. Duplicating them here would be a second
    // statement of one contract, which is the thing spec 040 US5 removes.

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
}
