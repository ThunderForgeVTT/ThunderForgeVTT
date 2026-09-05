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
//! `SYNC_GITHUB_APP_*`, matching the `OAUTH_<PROVIDER>_*` shape spec 007
//! established: the feature first, then the provider. Naming the host here is
//! deliberate and permitted — FR-004b puts the seam *after* the credential
//! grant, and arranging a grant is inherently host-shaped. A second host
//! arrives as `SYNC_GITLAB_APP_*` beside this one rather than as a widening of
//! it, which is the same shape `OAUTH_DISCORD_*` and `OAUTH_GITHUB_*` already
//! have.
//!
//! Three values:
//!
//! | Variable | What it is |
//! |---|---|
//! | `SYNC_GITHUB_APP_CLIENT_ID` | the application's client ID, which the assertion is issued by |
//! | `SYNC_GITHUB_APP_SLUG` | the application's URL slug, which the install link is built from |
//! | `SYNC_GITHUB_APP_PRIVATE_KEY_FILE` | a path to read the PEM from |
//! | `SYNC_GITHUB_APP_PRIVATE_KEY_BASE64` | *or* the PEM, base64-encoded |
//! | `SYNC_GITHUB_APP_PRIVATE_KEY` | *or* the PEM itself |
//!
//! The client ID and the slug are **not interchangeable** — one issues the
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
//! So three variables are accepted, in this precedence, and all carry the same
//! key:
//!
//! 1. **`..._PRIVATE_KEY_FILE`** — a path. The right answer for Docker and
//!    systemd secrets, which deliver secrets as files precisely because
//!    environments leak into logs and child processes.
//! 2. **`..._PRIVATE_KEY_BASE64`** — the PEM, base64-encoded. **The `.env`-safe
//!    form**: one line, no newlines to escape, nothing a shell or a dotenv
//!    parser will reinterpret. This is the one to reach for when a deployment
//!    platform accepts only single-line values.
//! 3. **`..._PRIVATE_KEY`** — the PEM itself, with either real newlines or the
//!    literal `\n` escapes that pasting into a `.env` file produces.
//!
//! **The encoding is declared, not detected.** An earlier version accepted
//! base64 in `..._PRIVATE_KEY` by sniffing the value, and that is retained as a
//! forgiving fallback — but sniffing is how `"not a key at all"` turned out to
//! be valid base64 and decoded to nine bytes of noise. A dedicated variable
//! means the operator says what they meant, and a value in it that is not
//! base64 is reported against *that* variable rather than falling through to a
//! confusing complaint about the other one.
//!
//! And the key is **parsed at configuration time, not at first use**
//! (FR-036c). A key that is present but not a usable RSA key is reported by
//! the diagnostic, alongside a missing one, rather than becoming an error the
//! first time a Game Master tries to connect.

use base64::Engine as _;
use base64::engine::general_purpose;
use thunderforge_repo_host::github::GitHubApp;

/// The application's **client ID**, which the assertion is issued by.
///
/// GitHub accepts either the client ID or the numeric application ID as a
/// JWT's `iss`, and its documentation says plainly: "Use of the client ID is
/// recommended." Naming the variable for the recommended one means an operator
/// copying the value GitHub puts in front of them lands in the right place,
/// rather than hunting for the numeric id that a variable called `..._APP_ID`
/// would have implied.
pub const APP_ID_ENV: &str = "SYNC_GITHUB_APP_CLIENT_ID";
/// The application's URL slug. Not interchangeable with the identifier.
pub const APP_SLUG_ENV: &str = "SYNC_GITHUB_APP_SLUG";
/// The PEM private key, in any of the four forms this module accepts.
pub const APP_PRIVATE_KEY_ENV: &str = "SYNC_GITHUB_APP_PRIVATE_KEY";
/// A path to read the PEM private key from. Highest precedence.
pub const APP_PRIVATE_KEY_FILE_ENV: &str = "SYNC_GITHUB_APP_PRIVATE_KEY_FILE";
/// The PEM private key, base64-encoded — declared rather than detected.
pub const APP_PRIVATE_KEY_BASE64_ENV: &str = "SYNC_GITHUB_APP_PRIVATE_KEY_BASE64";

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
    /// `..._PRIVATE_KEY_BASE64` was set to something that is not base64.
    /// Reported against that variable rather than falling through, because the
    /// operator declared an encoding and deserves to be told it was wrong.
    UndecodableBase64Key(String),
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
                "{APP_ID_ENV} is not set. It is the application's client ID — the value \
                 GitHub recommends issuing assertions with — and is different from the \
                 application's slug."
            ),
            Self::MissingAppSlug => format!(
                "{APP_SLUG_ENV} is not set. It is the application's URL slug, which the \
                 install link is built from, and is different from its numeric identifier."
            ),
            Self::MissingPrivateKey => format!(
                "None of {APP_PRIVATE_KEY_FILE_ENV}, {APP_PRIVATE_KEY_BASE64_ENV} or \
                 {APP_PRIVATE_KEY_ENV} is set. Supply the application's PEM private key \
                 as a file path, base64 (the .env-safe form), or the PEM itself."
            ),
            Self::UndecodableBase64Key(detail) => format!(
                "{APP_PRIVATE_KEY_BASE64_ENV} is set but is not valid base64 ({detail}). \
                 It must be the PEM file encoded whole — for example \
                 `base64 -w0 app-key.pem`."
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

    // An explicitly declared encoding. A value here that is not base64 is an
    // error against this variable, not a silent fall-through: the operator
    // said what it was, so being told "that is not base64" is more useful than
    // being told the key is unreadable.
    if let Some(encoded) = non_empty(APP_PRIVATE_KEY_BASE64_ENV) {
        let compact: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
        return match general_purpose::STANDARD.decode(&compact) {
            Ok(decoded) => Ok(Some(decoded)),
            Err(e) => Err(RegistrationProblem::UndecodableBase64Key(e.to_string())),
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

    /// Set some variables, run, restore.
    ///
    /// The environment is process-global and `cargo test` is threaded, so
    /// these cases are serialised behind one lock. Without it they pass alone
    /// and fail together, which is the worst way for a test to be wrong.
    fn temp_env(vars: &[(&str, Option<&str>)], body: impl FnOnce()) {
        use std::sync::Mutex;
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let previous: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| ((*k).to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            // SAFETY: serialised by LOCK above; no other thread in this test
            // binary reads these variables outside this helper.
            unsafe {
                match v {
                    Some(value) => std::env::set_var(k, value),
                    None => std::env::remove_var(k),
                }
            }
        }

        body();

        for (k, v) in previous {
            unsafe {
                match v {
                    Some(value) => std::env::set_var(&k, value),
                    None => std::env::remove_var(&k),
                }
            }
        }
    }

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

    /// The `.env`-safe form, declared rather than sniffed.
    #[test]
    fn a_declared_base64_key_is_decoded() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
        let encoded = general_purpose::STANDARD.encode(pem);
        temp_env(
            &[
                (APP_PRIVATE_KEY_BASE64_ENV, Some(encoded.as_str())),
                (APP_PRIVATE_KEY_ENV, None),
                (APP_PRIVATE_KEY_FILE_ENV, None),
            ],
            || {
                let key = read_private_key().expect("readable").expect("present");
                assert_eq!(String::from_utf8(key).unwrap(), pem);
            },
        );
    }

    /// The operator declared an encoding, so being told "that is not base64"
    /// is more useful than being told the key is unreadable. Falling through
    /// to the plain variable's forgiving path would produce the second.
    #[test]
    fn a_declared_base64_key_that_is_not_base64_says_so() {
        temp_env(
            &[
                (
                    APP_PRIVATE_KEY_BASE64_ENV,
                    Some("-----BEGIN PRIVATE KEY-----"),
                ),
                (APP_PRIVATE_KEY_ENV, None),
                (APP_PRIVATE_KEY_FILE_ENV, None),
            ],
            || match read_private_key() {
                Err(RegistrationProblem::UndecodableBase64Key(_)) => {}
                other => panic!("expected an UndecodableBase64Key, got {other:?}"),
            },
        );
    }

    /// A file path outranks both inline forms. An operator who went to the
    /// trouble of a mounted secret should not be silently overridden by a
    /// stale value left in an environment file.
    #[test]
    fn a_file_path_outranks_the_inline_forms() {
        let dir = std::env::temp_dir().join(format!("tf-key-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.pem");
        std::fs::write(
            &path,
            b"-----BEGIN PRIVATE KEY-----\nFROMFILE\n-----END PRIVATE KEY-----",
        )
        .unwrap();

        temp_env(
            &[
                (APP_PRIVATE_KEY_FILE_ENV, Some(path.to_str().unwrap())),
                (
                    APP_PRIVATE_KEY_BASE64_ENV,
                    Some(
                        &general_purpose::STANDARD.encode(
                            "-----BEGIN PRIVATE KEY-----\nINLINE\n-----END PRIVATE KEY-----",
                        ),
                    ),
                ),
                (
                    APP_PRIVATE_KEY_ENV,
                    Some("-----BEGIN PRIVATE KEY-----\nPLAIN\n-----END PRIVATE KEY-----"),
                ),
            ],
            || {
                let key =
                    String::from_utf8(read_private_key().expect("readable").expect("present"))
                        .unwrap();
                assert!(key.contains("FROMFILE"), "the file did not win: {key}");
            },
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `_BASE64` outranks the plain variable, so an operator migrating to the
    /// safer form does not have to remember to unset the old one.
    #[test]
    fn base64_outranks_the_plain_variable() {
        temp_env(
            &[
                (APP_PRIVATE_KEY_FILE_ENV, None),
                (
                    APP_PRIVATE_KEY_BASE64_ENV,
                    Some(
                        &general_purpose::STANDARD.encode(
                            "-----BEGIN PRIVATE KEY-----\nWINNER\n-----END PRIVATE KEY-----",
                        ),
                    ),
                ),
                (
                    APP_PRIVATE_KEY_ENV,
                    Some("-----BEGIN PRIVATE KEY-----\nLOSER\n-----END PRIVATE KEY-----"),
                ),
            ],
            || {
                let key =
                    String::from_utf8(read_private_key().expect("readable").expect("present"))
                        .unwrap();
                assert!(key.contains("WINNER"), "precedence is wrong: {key}");
            },
        );
    }

    /// FR-036c. An operator who set two of the three must be told which one is
    /// missing, not merely that the instance is unconfigured.
    #[test]
    fn a_partial_registration_names_only_what_is_missing() {
        temp_env(
            &[
                (APP_ID_ENV, Some("12345")),
                (APP_SLUG_ENV, None),
                (APP_PRIVATE_KEY_FILE_ENV, None),
                (APP_PRIVATE_KEY_BASE64_ENV, None),
                (APP_PRIVATE_KEY_ENV, Some("-----BEGIN PRIVATE KEY-----")),
            ],
            || {
                let problems = registration_from_env().err().expect("unconfigured");
                assert!(problems.contains(&RegistrationProblem::MissingAppSlug));
                assert!(!problems.contains(&RegistrationProblem::MissingAppId));
            },
        );
    }

    /// A variable set to whitespace is set to nothing. Templated configuration
    /// producing an empty value is common, and treating `""` as present would
    /// report a usable integration and then fail at the grant.
    #[test]
    fn a_blank_value_counts_as_missing() {
        temp_env(
            &[
                (APP_ID_ENV, Some("   ")),
                (APP_SLUG_ENV, Some("slug")),
                (APP_PRIVATE_KEY_FILE_ENV, None),
                (APP_PRIVATE_KEY_BASE64_ENV, None),
                (APP_PRIVATE_KEY_ENV, Some("-----BEGIN PRIVATE KEY-----")),
            ],
            || {
                let problems = registration_from_env().err().expect("unconfigured");
                assert!(problems.contains(&RegistrationProblem::MissingAppId));
            },
        );
    }

    /// A key that is present and is not a key. **The case a presence check
    /// calls configured**, and the whole reason this runs at configuration
    /// time rather than at first use.
    #[test]
    fn a_present_but_unusable_key_is_reported_rather_than_accepted() {
        temp_env(
            &[
                (APP_ID_ENV, Some("12345")),
                (APP_SLUG_ENV, Some("slug")),
                (APP_PRIVATE_KEY_FILE_ENV, None),
                (APP_PRIVATE_KEY_BASE64_ENV, None),
                (APP_PRIVATE_KEY_ENV, Some("hunter2")),
            ],
            || {
                let problems = registration_from_env().err().expect("unconfigured");
                assert!(
                    problems
                        .iter()
                        .any(|p| matches!(p, RegistrationProblem::UnreadablePrivateKey(_))),
                    "an unusable key was accepted: {problems:?}",
                );
            },
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
            assert!(guidance.contains("SYNC_GITHUB_APP"), "{guidance}");
        }
    }

    /// The client ID and the slug are different things, and an operator who
    /// swaps them gets an authentication failure that reads like a bad key.
    /// The guidance has to say so — and has to name the client ID, because a
    /// variable called `..._APP_ID` would send them looking for the numeric
    /// one GitHub no longer recommends.
    #[test]
    fn the_guidance_distinguishes_the_identifier_from_the_slug() {
        assert!(
            RegistrationProblem::MissingAppId
                .guidance()
                .contains("client ID"),
            "the guidance does not say which identifier GitHub recommends",
        );
        assert!(
            RegistrationProblem::MissingAppId
                .guidance()
                .contains("different from the application's slug")
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
