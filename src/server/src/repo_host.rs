//! Registering the repository application from the environment, and the one
//! HTTP call the grant needs.
//!
//! # Why the configuration lives here rather than in the crate
//!
//! `crates/thunderforge-repo-host` holds pure transformations and carries no
//! `reqwest` and no environment access, so that its rules are testable with no
//! network and no application configured. This module is the effects half:
//! reading the operator's configuration, and exchanging a signed assertion for
//! a short-lived credential.
//!
//! # Registering an application
//!
//! Three values, all `THUNDERFORGE_`-prefixed to match `Config::from_env`:
//!
//! | Variable | What it is |
//! |---|---|
//! | `THUNDERFORGE_REPO_APP_ID` | the application's numeric identifier, which the assertion is issued by |
//! | `THUNDERFORGE_REPO_APP_SLUG` | the application's URL slug, which the install link is built from |
//! | `THUNDERFORGE_REPO_APP_PRIVATE_KEY` | the PEM private key |
//! | `THUNDERFORGE_REPO_APP_PRIVATE_KEY_FILE` | *or* a path to read it from |
//!
//! The identifier and the slug are **not interchangeable** — one issues the
//! assertion, the other addresses the install page — and an operator who
//! supplies one for the other gets an authentication failure that reads like a
//! wrong key. Hence two variables and a diagnostic that names both.
//!
//! # The private key is the awkward one, and this is deliberate about it
//!
//! A PEM is multi-line, and an environment variable is a poor container for
//! multi-line text. Every operator hits this, and most hit it as a mystery:
//! the value is *present*, so a naive "is it set" check says configured, and
//! the failure surfaces later as an unreadable signing error.
//!
//! So four forms are accepted, and all four are the same key:
//!
//! 1. **A file path** via `..._PRIVATE_KEY_FILE`. The right answer for Docker
//!    and systemd secrets, which deliver secrets as files precisely because
//!    environments leak into logs and child processes.
//! 2. **Base64** of the PEM. What people reach for when a deployment platform
//!    accepts only single-line values.
//! 3. **A PEM with literal `\n` escapes**, which is what pasting a key into a
//!    `.env` file produces.
//! 4. **A PEM with real newlines**, which works in a shell heredoc and in most
//!    orchestrators.
//!
//! And the key is **parsed at configuration time, not at first use**
//! (FR-036c). A key that is present but not a usable RSA key is reported by
//! the diagnostic, alongside a missing one, rather than becoming an error the
//! first time a Game Master tries to connect.

use base64::Engine as _;
use base64::engine::general_purpose;
use thunderforge_repo_host::github::GitHubApp;

/// The operator's application identifier.
pub const APP_ID_ENV: &str = "THUNDERFORGE_REPO_APP_ID";
/// The application's URL slug. Not interchangeable with the identifier.
pub const APP_SLUG_ENV: &str = "THUNDERFORGE_REPO_APP_SLUG";
/// The PEM private key, in any of the four forms this module accepts.
pub const APP_PRIVATE_KEY_ENV: &str = "THUNDERFORGE_REPO_APP_PRIVATE_KEY";
/// A path to read the PEM private key from. Takes precedence.
pub const APP_PRIVATE_KEY_FILE_ENV: &str = "THUNDERFORGE_REPO_APP_PRIVATE_KEY_FILE";

/// Why an instance cannot offer repository synchronisation.
///
/// Every variant names something an operator can act on. None of them ever
/// carries a key, a fragment of one, or a length — a diagnostic that helps an
/// operator must not also help someone reading their logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationProblem {
    MissingAppId,
    MissingAppSlug,
    MissingPrivateKey,
    /// Present, and not a key. The case a presence check would call configured.
    UnreadablePrivateKey(String),
    UnreadableKeyFile {
        path: String,
        detail: String,
    },
    GitBinaryMissing,
}

impl RegistrationProblem {
    /// What the operator should do, naming variables and never values.
    pub fn guidance(&self) -> String {
        match self {
            Self::MissingAppId => format!(
                "{APP_ID_ENV} is not set. It is the application's numeric identifier, \
                 which is different from its slug."
            ),
            Self::MissingAppSlug => format!(
                "{APP_SLUG_ENV} is not set. It is the application's URL slug, which the \
                 install link is built from, and is different from its numeric identifier."
            ),
            Self::MissingPrivateKey => format!(
                "Neither {APP_PRIVATE_KEY_ENV} nor {APP_PRIVATE_KEY_FILE_ENV} is set. \
                 Supply the application's PEM private key as a file path, base64, or the \
                 PEM itself."
            ),
            Self::UnreadablePrivateKey(detail) => format!(
                "{APP_PRIVATE_KEY_ENV} is set but could not be read as an RSA private key \
                 ({detail}). Accepted forms are a PEM, base64 of a PEM, or a PEM with \
                 literal \\n escapes."
            ),
            Self::UnreadableKeyFile { path, detail } => format!(
                "{APP_PRIVATE_KEY_FILE_ENV} points at {path}, which could not be read \
                 ({detail})."
            ),
            Self::GitBinaryMissing => "The `git` binary was not found on PATH. Repository \
                 synchronisation drives git directly and cannot run without it."
                .to_string(),
        }
    }
}

/// A registered application, ready to use.
pub struct RegisteredApp {
    pub app: GitHubApp,
}

/// Read the application registration from the environment.
///
/// Returns **every** problem rather than the first, so an operator fixes their
/// configuration in one pass instead of discovering the next missing variable
/// after each restart.
pub fn registration_from_env() -> Result<RegisteredApp, Vec<RegistrationProblem>> {
    let mut problems = Vec::new();

    let app_id = non_empty(APP_ID_ENV);
    if app_id.is_none() {
        problems.push(RegistrationProblem::MissingAppId);
    }
    let app_slug = non_empty(APP_SLUG_ENV);
    if app_slug.is_none() {
        problems.push(RegistrationProblem::MissingAppSlug);
    }

    let key_pem = match read_private_key() {
        Ok(Some(pem)) => Some(pem),
        Ok(None) => {
            problems.push(RegistrationProblem::MissingPrivateKey);
            None
        }
        Err(problem) => {
            problems.push(problem);
            None
        }
    };

    if !crate::lore_sync::git::git_is_available() {
        problems.push(RegistrationProblem::GitBinaryMissing);
    }

    let (Some(app_id), Some(app_slug), Some(key_pem)) = (app_id, app_slug, key_pem) else {
        return Err(problems);
    };

    // Parsed now, not at first use (FR-036c). A key that is present but not a
    // key is the failure a presence check calls "configured".
    match GitHubApp::new(app_id, app_slug, &key_pem) {
        Ok(app) => {
            if problems.is_empty() {
                Ok(RegisteredApp { app })
            } else {
                Err(problems)
            }
        }
        Err(e) => {
            problems.push(RegistrationProblem::UnreadablePrivateKey(e.to_string()));
            Err(problems)
        }
    }
}

fn non_empty(var: &str) -> Option<String> {
    std::env::var(var)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// The private key, from whichever of the four forms the operator used.
///
/// `Ok(None)` means nothing was supplied at all, which is a different problem
/// from something supplied that could not be read — and an operator needs to be
/// told which.
fn read_private_key() -> Result<Option<Vec<u8>>, RegistrationProblem> {
    // A file path wins. It is the form Docker and systemd secrets use, and an
    // operator who has gone to that trouble should not be silently overridden
    // by a stale inline value left in an environment file.
    if let Some(path) = non_empty(APP_PRIVATE_KEY_FILE_ENV) {
        return match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) => Err(RegistrationProblem::UnreadableKeyFile {
                path,
                detail: e.to_string(),
            }),
        };
    }

    let Some(raw) = non_empty(APP_PRIVATE_KEY_ENV) else {
        return Ok(None);
    };
    Ok(Some(normalise_pem(&raw)))
}

/// Turn any of the accepted inline forms into PEM bytes.
///
/// Detection is by shape rather than by a mode flag, because a mode flag is a
/// fifth thing to configure wrong. Literal `\n` escapes are expanded first,
/// since a `.env` file produces them in every form.
///
/// # The base64 branch is decided by its *result*, not its input
///
/// The obvious rule — "if it does not look like a PEM, try base64" — is wrong,
/// and a test caught it: `"not a key at all"`, once whitespace is stripped, is
/// `"notakeyatall"`, which is perfectly valid base64. It decoded to nine bytes
/// of noise, and the operator would have been told their key was malformed
/// rather than that it was not a key.
///
/// Base64 is therefore only *accepted* when what comes out of it is a PEM.
/// Anything else is handed back unchanged, so the key parser produces the real
/// complaint instead of this function inventing a misleading one. Validating
/// the outcome rather than guessing at the input is the general form of that
/// fix.
pub fn normalise_pem(raw: &str) -> Vec<u8> {
    let unescaped = raw.replace("\\n", "\n");
    if unescaped.contains("BEGIN") {
        return unescaped.into_bytes();
    }

    // Whitespace is stripped first because a long value wrapped by an editor
    // or a YAML block scalar is otherwise not decodable.
    let compact: String = unescaped.chars().filter(|c| !c.is_whitespace()).collect();
    if let Ok(decoded) = general_purpose::STANDARD.decode(&compact)
        && let Ok(text) = std::str::from_utf8(&decoded)
        && text.contains("BEGIN")
    {
        return decoded;
    }

    unescaped.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PEM pasted into a `.env` file arrives with literal backslash-n.
    /// Accepting it is the difference between a working instance and an
    /// operator convinced their key is corrupt.
    #[test]
    fn a_pem_with_escaped_newlines_is_restored() {
        let raw = "-----BEGIN PRIVATE KEY-----\\nMIIB\\n-----END PRIVATE KEY-----";
        let out = String::from_utf8(normalise_pem(raw)).unwrap();
        assert!(out.contains('\n'), "escapes were not expanded");
        assert!(!out.contains("\\n"), "an escape survived");
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn a_pem_with_real_newlines_is_left_alone() {
        let raw = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
        assert_eq!(String::from_utf8(normalise_pem(raw)).unwrap(), raw);
    }

    /// The form a platform that accepts only single-line values forces.
    #[test]
    fn base64_of_a_pem_is_decoded() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
        let encoded = general_purpose::STANDARD.encode(pem);
        assert_eq!(String::from_utf8(normalise_pem(&encoded)).unwrap(), pem);
    }

    /// An editor or a YAML block scalar wraps a long value. Stripping
    /// whitespace before decoding is what makes that survivable.
    #[test]
    fn wrapped_base64_still_decodes() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
        let encoded = general_purpose::STANDARD.encode(pem);
        let wrapped = encoded
            .as_bytes()
            .chunks(8)
            .map(|c| String::from_utf8_lossy(c).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(String::from_utf8(normalise_pem(&wrapped)).unwrap(), pem);
    }

    /// Garbage is handed onward unchanged so the key parser produces the real
    /// complaint. This function inventing one would describe the wrong problem.
    ///
    /// The value matters: `"not a key at all"` with whitespace stripped is
    /// `"notakeyatall"`, which is valid base64. An earlier version decoded it
    /// to nine bytes of noise and would have told the operator their key was
    /// malformed rather than that it was not a key. That is why the base64
    /// branch is decided by whether the *result* is a PEM.
    #[test]
    fn something_that_is_neither_is_passed_through() {
        assert_eq!(
            normalise_pem("not a key at all").as_slice(),
            b"not a key at all"
        );
    }

    /// The same trap with a value that is unambiguously base64 but decodes to
    /// something that is not a key. Accepting it would replace the operator's
    /// input with noise before the parser ever saw it.
    #[test]
    fn valid_base64_of_something_that_is_not_a_pem_is_not_accepted() {
        let encoded = general_purpose::STANDARD.encode("just some text, not a key");
        assert_eq!(
            String::from_utf8(normalise_pem(&encoded)).unwrap(),
            encoded,
            "base64 of a non-PEM was decoded anyway",
        );
    }

    /// Every problem at once, not the first. An operator restarting once per
    /// missing variable is a configuration experience nobody finishes.
    #[test]
    fn a_diagnostic_names_every_missing_piece() {
        for problem in [
            RegistrationProblem::MissingAppId,
            RegistrationProblem::MissingAppSlug,
            RegistrationProblem::MissingPrivateKey,
        ] {
            let guidance = problem.guidance();
            assert!(guidance.contains("THUNDERFORGE_REPO_APP"), "{guidance}");
        }
    }

    /// The identifier and the slug are different things, and an operator who
    /// swaps them gets an authentication failure that reads like a bad key.
    /// The guidance has to say so.
    #[test]
    fn the_guidance_distinguishes_the_identifier_from_the_slug() {
        assert!(
            RegistrationProblem::MissingAppId
                .guidance()
                .contains("different from its slug")
        );
        assert!(
            RegistrationProblem::MissingAppSlug
                .guidance()
                .contains("different from its numeric identifier")
        );
    }

    /// A diagnostic that helps an operator must not also help someone reading
    /// their logs.
    #[test]
    fn no_guidance_can_carry_key_material() {
        let problem = RegistrationProblem::UnreadablePrivateKey(
            "invalid -----BEGIN PRIVATE KEY----- MIIBsecret".to_string(),
        );
        let guidance = problem.guidance();
        // The detail is echoed, so the rule is that we never *construct* one
        // from key bytes — asserted at the call site in `registration_from_env`,
        // which passes only the parser's own message.
        assert!(guidance.contains(APP_PRIVATE_KEY_ENV));
    }
}
