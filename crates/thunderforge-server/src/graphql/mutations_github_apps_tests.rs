//! T098: **nothing here carries a key, a fragment of one, or its length.**
//!
//! FR-023 and SC-007. The assertion is against every string this surface can
//! produce for every scope, not against the fields somebody remembered — the
//! point of one rendering function is that the check can be exhaustive.
//!
//! # Isolation
//!
//! These cases read the process environment and the `instance_settings` table.
//! One lock for both, `settings::test_env`'s, the same one `github_apps`'s own
//! tests, `settings::changes`'s and `repo_host_tests`' take. The variables are
//! cleared to `""` rather than removed, because `test_app_state()` calls
//! `dotenvy::dotenv()` on every call from hundreds of unrelated tests and
//! re-creates a *removed* variable while leaving a *set* one alone —
//! `github_apps_tests.rs` records that failure in full.

use super::*;
use crate::settings::test_env::temp_env;
use crate::test_support::test_app_state;

const KEY_PEM: &str = include_str!(
    "../../../../crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem"
);

const ALL_VARS: &[&str] = &[
    "GLOBAL_GITHUB_APP_CLIENT_ID",
    "GLOBAL_GITHUB_APP_SLUG",
    "GLOBAL_GITHUB_APP_PRIVATE_KEY",
    "GLOBAL_GITHUB_APP_PRIVATE_KEY_FILE",
    "GLOBAL_GITHUB_APP_PRIVATE_KEY_BASE64",
    "SYNC_GITHUB_APP_CLIENT_ID",
    "SYNC_GITHUB_APP_SLUG",
    "SYNC_GITHUB_APP_PRIVATE_KEY",
    "SYNC_GITHUB_APP_PRIVATE_KEY_FILE",
    "SYNC_GITHUB_APP_PRIVATE_KEY_BASE64",
    "FEEDBACK_GITHUB_APP_CLIENT_ID",
    "FEEDBACK_GITHUB_APP_SLUG",
    "FEEDBACK_GITHUB_APP_PRIVATE_KEY",
    "FEEDBACK_GITHUB_APP_PRIVATE_KEY_FILE",
    "FEEDBACK_GITHUB_APP_PRIVATE_KEY_BASE64",
];

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(f)
}

/// Every one of the fifteen variables cleared, then these set.
fn with_env(overrides: &[(&str, &str)], body: impl FnOnce(&Settings)) {
    let state = test_app_state();
    let mut vars: Vec<(&str, Option<&str>)> = ALL_VARS.iter().map(|v| (*v, Some(""))).collect();
    for (name, value) in overrides {
        vars.retain(|(existing, _)| existing != name);
        vars.push((name, Some(value)));
    }
    temp_env(&vars, || {
        let settings = block_on(resolve_all(&state)).expect("resolves");
        body(&settings);
    });
}

/// Every scope fully configured, with a real key — the state in which there is
/// the most to leak.
fn with_everything_configured(body: impl FnOnce(&Settings)) {
    with_env(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
            ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.syncsyncsyncsync"),
            ("SYNC_GITHUB_APP_SLUG", "sync-app"),
            ("SYNC_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
            ("FEEDBACK_GITHUB_APP_CLIENT_ID", "Iv1.feedbackfeedback"),
            ("FEEDBACK_GITHUB_APP_SLUG", "feedback-app"),
            ("FEEDBACK_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        body,
    );
}

/// Only the global application, which is the ordinary single-application
/// deployment.
fn with_only_a_global_application(body: impl FnOnce(&Settings)) {
    with_env(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        body,
    );
}

/// The key's distinctive middle, used as the needle. Taken from the fixture at
/// runtime rather than pasted here, so it cannot drift from what is actually
/// configured above.
fn key_fragments() -> Vec<String> {
    let body: String = KEY_PEM
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect();
    vec![
        KEY_PEM.to_string(),
        body.clone(),
        body.chars().take(24).collect(),
        body.chars().skip(40).take(16).collect(),
    ]
}

/// **T098.** Every string every scope's rendering can produce, checked against
/// the key, four fragments of it, and its length.
#[test]
fn no_field_of_this_surface_carries_a_key_a_fragment_or_a_length() {
    with_everything_configured(|settings| {
        let lengths = [
            KEY_PEM.len().to_string(),
            KEY_PEM.trim().len().to_string(),
            KEY_PEM.lines().count().to_string(),
        ];
        for scope in AppScope::all() {
            let rendered = describe(
                *scope,
                settings,
                Some((
                    "2026-09-07T00:00:00Z".to_string(),
                    "accepted by the host (200)".to_string(),
                )),
            );
            assert!(
                rendered.has_private_key,
                "`{}` should report a key it has",
                scope.as_str()
            );

            // The whole struct, every field, including the nested per-field
            // records and every guidance sentence.
            let whole = format!("{rendered:?}");
            for needle in key_fragments() {
                assert!(
                    !needle.is_empty() && !whole.contains(&needle),
                    "`{}` leaked key material",
                    scope.as_str()
                );
            }
            for length in &lengths {
                assert!(
                    !whole.contains(length.as_str()),
                    "`{}` leaked a length ({length}): {whole}",
                    scope.as_str()
                );
            }
            // And the one positive: a client ID is *not* a secret, GitHub
            // publishes it, and hiding it would make the screen useless.
            assert!(rendered.client_id.is_some());
        }
    });
}

/// The refusal an operator sees when they paste something that is not a key
/// names the setting and quotes nothing back. A message that echoes the value
/// is a message that echoes a nearly-correct key into a log.
#[test]
fn the_refusal_for_a_bad_key_names_the_setting_and_not_the_value() {
    let pretend_secret = "-----BEGIN RSA PRIVATE KEY-----swordfish-----END RSA PRIVATE KEY-----";
    let detail = crate::github_apps::parse_private_key_for_storage(pretend_secret)
        .expect_err("a presence check would have called this configured");
    assert!(!detail.contains("swordfish"), "{detail}");
    assert!(
        !detail.contains(&pretend_secret.len().to_string()),
        "{detail}"
    );
}

/// A live check reports a status code and a verdict. A response body would be
/// one 401 payload away from carrying a credential.
#[test]
fn a_check_outcome_carries_a_status_and_no_body() {
    for outcome in [
        "accepted by the host (200)",
        "rejected by the host (401) — the client ID and the private key do not belong to the \
         same application, or the key has been revoked",
        "not found by the host (404) — check the client ID; it is not the slug",
        "the host could not be reached",
        "not configured — nothing to check",
    ] {
        assert!(!outcome.contains("BEGIN"), "{outcome}");
        assert!(!outcome.to_lowercase().contains("length"), "{outcome}");
    }
}

/// A subsystem with no application of its own, beside a global one that
/// works, has **nothing to report**.
///
/// This is a bug that was here and is kept as a test. `guidance` was rendered
/// from the scope's own unmet requirements, and `CredentialProblem::Missing`
/// says "Neither `github_app.feedback.client_id` nor
/// `github_app.global.client_id` is set" — which is the right sentence when
/// nothing resolves and a plain falsehood on a screen that is showing the
/// global one set two cards above. Guidance is about the resolution.
#[test]
fn a_subsystem_using_the_global_application_reports_no_gap() {
    with_only_a_global_application(|settings| {
        for scope in [AppScope::Sync, AppScope::Feedback] {
            let rendered = describe(scope, settings, None);
            assert_eq!(rendered.resolves_to.as_deref(), Some("global"));
            assert!(!rendered.configured);
            assert!(!rendered.complete);
            assert_eq!(
                rendered.guidance,
                Vec::<String>::new(),
                "`{}` reported a gap it does not have",
                scope.as_str()
            );
            for line in &rendered.guidance {
                assert!(!line.contains("Neither"), "{line}");
            }
        }

        // And the global one says which subsystems it is acting for, which is
        // the sentence FR-020 asks be shown wherever it is set.
        let global = describe(AppScope::Global, settings, None);
        assert!(global.complete);
        assert_eq!(global.acts_for, vec!["sync", "feedback"]);
        assert_eq!(global.guidance, Vec::<String>::new());
    });
}

/// A subsystem that is **half** written does report, and the sentence says it
/// was stepped over rather than that nothing is set.
#[test]
fn a_half_written_subsystem_reports_being_stepped_over_and_not_being_unset() {
    with_env(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
            ("FEEDBACK_GITHUB_APP_CLIENT_ID", "Iv1.feedbackfeedback"),
        ],
        |settings| {
            let rendered = describe(AppScope::Feedback, settings, None);
            assert!(rendered.configured);
            assert!(!rendered.complete);
            assert_eq!(rendered.resolves_to.as_deref(), Some("global"));
            assert_eq!(rendered.guidance.len(), 1);
            let told = &rendered.guidance[0];
            assert!(told.contains("only partly configured"), "{told}");
            assert!(told.contains("github_app.feedback.slug"), "{told}");
            assert!(!told.contains("Neither"), "{told}");
        },
    );
}

/// **T103.** A surface that compiles but was never merged into the roots fails
/// for the first operator who opens the screen, not for the suite — and the
/// second half of this is the SDL guard FR-023 asks for: **no field of
/// `GithubApplication` returns a key.**
#[test]
fn the_surface_is_registered_and_no_field_of_it_returns_a_key() {
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish();
    let sdl = schema.sdl();

    for field in [
        "githubApplications",
        "setGithubApplication(scope: String!",
        "checkGithubApplication(scope: String!",
    ] {
        assert!(
            sdl.contains(field),
            "`{field}` must be reachable from the root"
        );
    }

    // The two type blocks this surface owns, read field by field. A field
    // whose name says it carries key material, or whose type would let it, is
    // the failure — `hasPrivateKey: Boolean!` is the only mention allowed.
    for type_name in ["GithubApplication", "GithubApplicationField"] {
        let block = sdl
            .split(&format!("type {type_name} "))
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .unwrap_or_else(|| panic!("`{type_name}` is not in the schema"));
        for line in block.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let lower = line.to_lowercase();
            if lower.starts_with("hasprivatekey:") {
                assert!(
                    line.contains("Boolean!"),
                    "hasPrivateKey stopped being a boolean: {line}"
                );
                continue;
            }
            assert!(
                !lower.contains("privatekey"),
                "`{type_name}.{line}` looks like it returns key material"
            );
            assert!(
                !lower.starts_with("secret") && !lower.contains("keylength"),
                "`{type_name}.{line}`"
            );
        }
    }
}
