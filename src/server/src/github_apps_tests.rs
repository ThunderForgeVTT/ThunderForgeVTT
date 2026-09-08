//! Spec 040 US5: resolution, back-compatibility, and the two things that must
//! never happen — a half-configured application silently completed, and a
//! diagnostic carrying a credential.
//!
//! # Isolation
//!
//! Every case here reads the process environment **and** the
//! `instance_settings` table, and both are process-global. There is one lock
//! for them — `settings::test_env`'s — taken by `temp_env`, by
//! `settings::changes`'s tests and by `repo_host_tests`. A second mutex over
//! one global serialises nothing, which this session learned three times.

use super::*;
use crate::settings::resolver::{Settings, Source, encrypt, resolve_all};
use crate::settings::test_env::temp_env;
use crate::state::AppState;
use crate::test_support::test_app_state;
use diesel::prelude::*;

/// A real RSA key, so `GitHubApp::new` actually parses rather than being
/// stubbed. The same throwaway fixture `thunderforge-repo-host`'s own tests
/// use; it is committed and worthless, and the README beside it says so.
const KEY_PEM: &str = include_str!(
    "../../../crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem"
);

/// Every variable any of the three scopes reads. Cleared wholesale at the top
/// of each case: a leftover `FEEDBACK_GITHUB_APP_SLUG` from another test makes
/// this file's answers depend on its neighbours' order.
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

fn write_row(state: &AppState, key: &str, value: &str) {
    use crate::schema::instance_settings;
    let stored = if crate::settings::registry::declaration(key)
        .expect("a declared key")
        .secret
    {
        encrypt(state, value).expect("encrypts")
    } else {
        value.to_string()
    };
    let mut conn = state.db_pool.get().expect("conn");
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(instance_settings::table)
        .values((
            instance_settings::key.eq(key),
            instance_settings::value.eq(&stored),
            instance_settings::updated_at.eq(now),
            instance_settings::created_at.eq(now),
        ))
        .on_conflict(instance_settings::key)
        .do_update()
        .set(instance_settings::value.eq(&stored))
        .execute(&mut conn)
        .expect("row written");
}

fn clear_rows(state: &AppState) {
    use crate::schema::instance_settings;
    let mut conn = state.db_pool.get().expect("conn");
    for scope in AppScope::all() {
        for field in Field::all() {
            let key = setting_key(*scope, *field);
            diesel::delete(instance_settings::table.filter(instance_settings::key.eq(key)))
                .execute(&mut conn)
                .expect("row removed");
        }
    }
}

/// Clear all fifteen variables, apply the ones this case sets, write the rows
/// it wants, resolve, and hand the result to the body. Rows are cleared before
/// *and* after, because a case that fails mid-way must not poison the next.
///
/// # Why "cleared" means `Some("")` and not `None`
///
/// `test_support::test_app_state()` calls `dotenvy::dotenv()` on **every**
/// call, from hundreds of tests, none of which hold this file's lock — and a
/// developer's real `.env` has `SYNC_GITHUB_APP_*` in it. `dotenv()` does not
/// override a variable that is *set*, but it happily re-creates one that was
/// *removed*. So a case here that unset `SYNC_GITHUB_APP_CLIENT_ID` had it put
/// back mid-test by an unrelated test constructing an `AppState`, and a
/// fallback case then found the developer's own application configured. That
/// was observed as one random failure per run, in a different case each time,
/// and it is the third variant of this hazard this session.
///
/// Setting them to the empty string closes it: `registry::read_env` and
/// `repo_host::non_empty` both treat an exported-but-empty variable as unset
/// ("how a container platform says 'I did not set this'"), and `dotenv()`
/// leaves a set variable alone. The clearing is therefore stable against every
/// other test in the binary rather than against the ones that happen to be
/// polite.
fn with_apps(env: &[(&str, &str)], rows: &[(&str, &str)], body: impl FnOnce(&AppState, &Settings)) {
    let state = test_app_state();
    let mut vars: Vec<(&str, Option<&str>)> = ALL_VARS.iter().map(|v| (*v, Some(""))).collect();
    for (name, value) in env {
        vars.retain(|(existing, _)| existing != name);
        vars.push((name, Some(value)));
    }
    temp_env(&vars, || {
        clear_rows(&state);
        for (key, value) in rows {
            write_row(&state, key, value);
        }
        let settings = block_on(resolve_all(&state)).expect("resolves");
        body(&state, &settings);
        clear_rows(&state);
    });
}

/// Two registrations are the same registration when they issue byte-identical
/// assertions and address the same host. RS256 with PKCS#1 v1.5 padding is
/// deterministic, so this is an equality test and not an approximation —
/// `GitHubApp` deliberately exposes no accessor for its key, which is the
/// point.
fn same_registration(left: &GitHubApp, right: &GitHubApp) {
    const AT: u64 = 1_700_000_000;
    assert_eq!(
        left.app_assertion(AT).expect("signs"),
        right.app_assertion(AT).expect("signs"),
        "the two registrations issue different assertions"
    );
    assert_eq!(left.installations_url(), right.installations_url());
}

// ============================================================================
// T095 — the test that protects somebody's running deployment (FR-024)
// ============================================================================

/// **A deployment that sets only `SYNC_GITHUB_APP_*` sees no change at all.**
///
/// This is written first and it is the one that matters most. Somebody is
/// running this today with five variables and nothing else, and a nicer
/// configuration model is not worth their lore synchronisation.
///
/// It asserts against `registration_from_env()` — the function that was
/// serving them before this feature existed — rather than against a value
/// typed into this file, because a constant here would be a restatement of
/// what the new code does and would agree with it however it changed.
///
/// `git` on PATH is a precondition: `registration_from_env` reports a missing
/// binary as a registration problem (it is one, for lore synchronisation), and
/// that is a fact about the machine rather than about the configuration. It is
/// asserted rather than skipped, because a test that quietly does nothing on a
/// machine without git is a test that quietly does nothing in CI.
#[test]
fn a_sync_only_deployment_resolves_exactly_what_it_resolved_before() {
    assert!(
        crate::lore_sync::git::git_is_available(),
        "this case compares against `registration_from_env()`, which reports a missing \
         `git` binary as a registration problem. Install git to run it."
    );

    with_apps(
        &[
            ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.deadbeefdeadbeef"),
            ("SYNC_GITHUB_APP_SLUG", "thunderforge-sync"),
            ("SYNC_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        &[],
        |_state, settings| {
            let before = crate::repo_host::registration_from_env().unwrap_or_else(|problems| {
                panic!("the configuration in use today was refused: {problems:?}")
            });
            let after = registration_for(AppScope::Sync, settings).unwrap_or_else(|problems| {
                panic!("a SYNC-only deployment stopped resolving: {problems:?}")
            });

            same_registration(&before.app, &after.app);
            assert_eq!(
                after.scope,
                AppScope::Sync,
                "sync stopped using its own application"
            );
            assert!(!after.fell_back());
            assert!(after.stepped_over.is_empty());
        },
    );
}

/// The `.env`-safe form and the mounted-secret form travel the same road they
/// did before, through the *declaration's aliases* rather than through a
/// second reader. If this passes and the case above passes, no deployment's
/// key form has been quietly dropped.
#[test]
fn a_sync_only_deployment_keeps_all_three_key_forms() {
    let dir = std::env::temp_dir().join(format!("tf-app-key-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("app.pem");
    std::fs::write(&path, KEY_PEM).expect("fixture written");
    let encoded = base64::engine::general_purpose::STANDARD.encode(KEY_PEM);

    for (var, value) in [
        ("SYNC_GITHUB_APP_PRIVATE_KEY", KEY_PEM.to_string()),
        ("SYNC_GITHUB_APP_PRIVATE_KEY_BASE64", encoded),
        (
            "SYNC_GITHUB_APP_PRIVATE_KEY_FILE",
            path.to_string_lossy().to_string(),
        ),
    ] {
        with_apps(
            &[
                ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.deadbeefdeadbeef"),
                ("SYNC_GITHUB_APP_SLUG", "thunderforge-sync"),
                (var, &value),
            ],
            &[],
            |_state, settings| {
                let app = registration_for(AppScope::Sync, settings)
                    .unwrap_or_else(|p| panic!("`{var}` stopped working: {p:?}"));
                assert_eq!(app.scope, AppScope::Sync);
            },
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

/// **The three key forms keep their precedence: `_FILE` beats `_BASE64` beats
/// the plain variable.**
///
/// This one found a real back-compatibility break and is the reason T095 is
/// more than a single case. `registration_from_env` reads the three in that
/// order explicitly, and `repo_host_tests` pins it: "an operator who went to
/// the trouble of a mounted secret should not be silently overridden by a
/// stale value left in an environment file."
///
/// The declaration route did not, at first. `SettingDeclaration::env_name_in_use`
/// checks `env_var` and then `env_aliases` **in order**, and the declaration
/// named the *plain* variable as primary with `_FILE` as an alias — so a
/// deployment setting both resolved to the inline value here and to the file
/// through `registration_from_env`. Two answers for one configuration, in a
/// function whose entire job is to give the same answer as the old one.
/// Reordering the declaration fixed it, which is what the field means: the
/// aliases are documented as being "in order".
#[test]
fn the_three_key_forms_keep_their_precedence() {
    let dir = std::env::temp_dir().join(format!("tf-precedence-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("app.pem");
    std::fs::write(&path, KEY_PEM).expect("fixture written");

    // A PEM that parses and is *not* the fixture, so "which one won" is
    // observable rather than inferred. Any second key would do; a second
    // fixture is not worth committing, so the loser is deliberately unusable
    // and the assertion is that resolution succeeded at all.
    let loser = "-----BEGIN PRIVATE KEY-----\nLOSER\n-----END PRIVATE KEY-----";
    let encoded_loser = base64::engine::general_purpose::STANDARD.encode(loser);

    // The file beats both inline forms.
    with_apps(
        &[
            ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.deadbeefdeadbeef"),
            ("SYNC_GITHUB_APP_SLUG", "thunderforge-sync"),
            ("SYNC_GITHUB_APP_PRIVATE_KEY_FILE", &path.to_string_lossy()),
            ("SYNC_GITHUB_APP_PRIVATE_KEY_BASE64", &encoded_loser),
            ("SYNC_GITHUB_APP_PRIVATE_KEY", loser),
        ],
        &[],
        |_state, settings| {
            let before = crate::repo_host::registration_from_env()
                .unwrap_or_else(|p| panic!("the old reader refused this: {p:?}"));
            let after = registration_for(AppScope::Sync, settings)
                .unwrap_or_else(|p| panic!("the file form lost to a stale inline value: {p:?}"));
            same_registration(&before.app, &after.app);
        },
    );

    // And base64 beats the plain variable, so an operator migrating to the
    // safer form does not have to remember to unset the old one.
    with_apps(
        &[
            ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.deadbeefdeadbeef"),
            ("SYNC_GITHUB_APP_SLUG", "thunderforge-sync"),
            (
                "SYNC_GITHUB_APP_PRIVATE_KEY_BASE64",
                &base64::engine::general_purpose::STANDARD.encode(KEY_PEM),
            ),
            ("SYNC_GITHUB_APP_PRIVATE_KEY", loser),
        ],
        &[],
        |_state, settings| {
            let before = crate::repo_host::registration_from_env()
                .unwrap_or_else(|p| panic!("the old reader refused this: {p:?}"));
            let after = registration_for(AppScope::Sync, settings)
                .unwrap_or_else(|p| panic!("base64 lost to the plain variable: {p:?}"));
            same_registration(&before.app, &after.app);
        },
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The same precedence, for every scope, asserted against the declarations
/// rather than against behaviour — because the behaviour above only covers
/// `sync`, and `global` and `feedback` have no `registration_from_env` to
/// compare with.
#[test]
fn every_scopes_private_key_declaration_orders_the_forms_the_same_way() {
    for scope in AppScope::all() {
        let d = crate::settings::registry::declaration(setting_key(*scope, Field::PrivateKey))
            .expect("declared");
        let order: Vec<&str> = std::iter::once(d.env_var.expect("has a variable"))
            .chain(d.env_aliases.iter().copied())
            .collect();
        assert_eq!(
            order.len(),
            3,
            "`{}` does not offer all three key forms",
            d.key
        );
        assert!(order[0].ends_with("_PRIVATE_KEY_FILE"), "{order:?}");
        assert!(order[1].ends_with("_PRIVATE_KEY_BASE64"), "{order:?}");
        assert!(order[2].ends_with("_PRIVATE_KEY"), "{order:?}");
    }
}

// ============================================================================
// T096 — scope outside, source inside (contract, research.md § R10)
// ============================================================================

/// **A global application set in the environment does not beat a subsystem
/// application configured in the admin screens.**
///
/// This is the case the spec left open and R10 closed. FR-010 ("the
/// environment wins") and FR-019 ("the subsystem wins") point opposite ways
/// here, and somebody who configured a subsystem application meant it.
#[test]
fn a_global_environment_application_does_not_beat_a_deliberate_subsystem_one() {
    with_apps(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        &[
            ("github_app.feedback.client_id", "Iv1.feedbackfeedback"),
            ("github_app.feedback.slug", "feedback-app"),
            ("github_app.feedback.private_key", KEY_PEM),
        ],
        |_state, settings| {
            let app = registration_for(AppScope::Feedback, settings).expect("resolves");
            assert_eq!(
                app.scope,
                AppScope::Feedback,
                "a broad GLOBAL_* silently overrode the application an operator configured"
            );
            let slug = app
                .sources
                .iter()
                .find(|s| s.field == Field::Slug)
                .expect("the slug has a source");
            assert_eq!(slug.key, "github_app.feedback.slug");
            assert_eq!(
                slug.fixed_by, None,
                "it came from the instance, not a variable"
            );
        },
    );
}

/// Within one scope the ordinary rule applies: the environment beats the
/// instance's store. Steps 1 and 2 of the four.
#[test]
fn within_one_scope_the_environment_still_wins() {
    with_apps(
        &[
            ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.fromtheenvironmt"),
            ("SYNC_GITHUB_APP_SLUG", "from-the-environment"),
            ("SYNC_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        &[
            ("github_app.sync.client_id", "Iv1.fromtheinstance"),
            ("github_app.sync.slug", "from-the-instance"),
            ("github_app.sync.private_key", KEY_PEM),
        ],
        |_state, settings| {
            let app = registration_for(AppScope::Sync, settings).expect("resolves");
            assert_eq!(app.scope, AppScope::Sync);
            assert_eq!(
                settings.value("github_app.sync.slug"),
                Some("from-the-environment")
            );
            assert_eq!(
                settings.get("github_app.sync.slug").map(|r| r.source),
                Some(Source::Environment)
            );
        },
    );
}

/// Steps 3 and 4: with no subsystem application anywhere, every subsystem uses
/// the global one, and `actsFor` says so.
#[test]
fn one_global_application_serves_every_subsystem_and_says_which() {
    with_apps(
        &[],
        &[
            ("github_app.global.client_id", "Iv1.globalglobalglob"),
            ("github_app.global.slug", "global-app"),
            ("github_app.global.private_key", KEY_PEM),
        ],
        |_state, settings| {
            for subsystem in AppScope::subsystems() {
                let app = registration_for(*subsystem, settings)
                    .unwrap_or_else(|p| panic!("{} did not fall back: {p:?}", subsystem.as_str()));
                assert_eq!(app.scope, AppScope::Global);
                assert_eq!(app.requested, *subsystem);
                assert!(app.fell_back());
                assert!(
                    app.stepped_over.is_empty(),
                    "an unconfigured subsystem is the ordinary case, not a problem report"
                );
            }

            let serves = acts_for(AppScope::Global, settings);
            assert_eq!(serves, vec![AppScope::Sync, AppScope::Feedback]);
            assert!(acts_for(AppScope::Sync, settings).is_empty());
        },
    );
}

/// A subsystem application in the environment wins over a global one in the
/// environment — step 1 over step 3, the ordinary reading of FR-019.
#[test]
fn a_subsystem_application_wins_for_its_own_subsystem_and_the_global_serves_the_rest() {
    with_apps(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
            ("SYNC_GITHUB_APP_CLIENT_ID", "Iv1.syncsyncsyncsync"),
            ("SYNC_GITHUB_APP_SLUG", "sync-app"),
            ("SYNC_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        &[],
        |_state, settings| {
            assert_eq!(
                registration_for(AppScope::Sync, settings)
                    .expect("resolves")
                    .scope,
                AppScope::Sync
            );
            assert_eq!(
                registration_for(AppScope::Feedback, settings)
                    .expect("resolves")
                    .scope,
                AppScope::Global
            );
            assert_eq!(
                acts_for(AppScope::Global, settings),
                vec![AppScope::Feedback]
            );
            assert_eq!(acts_for(AppScope::Sync, settings), vec![AppScope::Sync]);
        },
    );
}

// ============================================================================
// T097 — an application resolves whole (FR-021, US5 scenario 4)
// ============================================================================

/// **A half-written subsystem application is stepped over whole; it never
/// borrows the global application's private key.**
///
/// A client ID from one registration with a key from another is not an
/// application. It is an authentication failure that reads like a bad key, and
/// an operator debugging it has no way to see that the two halves came from
/// different places.
#[test]
fn a_partly_configured_subsystem_falls_through_to_the_whole_global_application() {
    with_apps(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        &[
            // A client ID and nothing else — the shape somebody produces by
            // getting halfway through the form and being interrupted.
            ("github_app.feedback.client_id", "Iv1.feedbackfeedback"),
        ],
        |_state, settings| {
            let app = registration_for(AppScope::Feedback, settings).expect("resolves");

            assert_eq!(app.scope, AppScope::Global, "it did not fall through");
            // Every field came from the global scope. Not one of them is the
            // half-written subsystem's.
            for source in &app.sources {
                assert_eq!(
                    source.scope,
                    AppScope::Global,
                    "`{}` was borrowed across scopes",
                    source.key
                );
            }
            assert!(
                !app.sources.iter().any(|s| s.key.contains(".feedback.")),
                "a field of the half-written application was used: {:?}",
                app.sources
            );

            // And the operator is told, naming what is missing (FR-021).
            let missing = missing_keys(&app.stepped_over);
            assert!(missing.contains(&"github_app.feedback.slug"), "{missing:?}");
            assert!(
                missing.contains(&"github_app.feedback.private_key"),
                "{missing:?}"
            );
            assert!(!missing.contains(&"github_app.feedback.client_id"));

            let told = stepped_over_guidance(AppScope::Feedback, &app.stepped_over);
            assert!(told.contains("github_app.feedback.slug"), "{told}");
            assert!(told.contains("only partly configured"), "{told}");
            assert!(!told.contains("Iv1."), "{told}");
        },
    );
}

/// The same shape with no global application at all: the subsystem is reported
/// incomplete naming only what is missing, and the client ID it *does* have is
/// not reported as a problem.
#[test]
fn a_partly_configured_subsystem_with_no_global_names_only_the_gaps() {
    with_apps(
        &[],
        &[("github_app.sync.client_id", "Iv1.syncsyncsyncsync")],
        |_state, settings| {
            let problems = registration_for(AppScope::Sync, settings)
                .err()
                .expect("incomplete");
            let missing = missing_keys(&problems);
            assert!(missing.contains(&"github_app.sync.slug"), "{missing:?}");
            assert!(
                missing.contains(&"github_app.sync.private_key"),
                "{missing:?}"
            );
            assert!(
                !missing.contains(&"github_app.sync.client_id"),
                "{missing:?}"
            );
        },
    );
}

/// A key that is present and is not a key is a refusal, not a fallback. The
/// application is *configured* and *broken*, and silently using the global one
/// would hide a credential the operator believes is in use.
#[test]
fn a_subsystem_key_that_is_not_a_key_is_reported_rather_than_stepped_over_silently() {
    with_apps(
        &[
            ("GLOBAL_GITHUB_APP_CLIENT_ID", "Iv1.globalglobalglob"),
            ("GLOBAL_GITHUB_APP_SLUG", "global-app"),
            ("GLOBAL_GITHUB_APP_PRIVATE_KEY", KEY_PEM),
        ],
        &[
            ("github_app.sync.client_id", "Iv1.syncsyncsyncsync"),
            ("github_app.sync.slug", "sync-app"),
            ("github_app.sync.private_key", "not a key at all"),
        ],
        |_state, settings| {
            let app = registration_for(AppScope::Sync, settings).expect("the global still serves");
            assert_eq!(app.scope, AppScope::Global);
            assert!(
                app.stepped_over
                    .iter()
                    .any(|p| matches!(p, CredentialProblem::UnreadableKey { .. })),
                "the broken key was not reported: {:?}",
                app.stepped_over
            );
            let told = stepped_over_guidance(AppScope::Sync, &app.stepped_over);
            assert!(told.contains("github_app.sync.private_key"), "{told}");
            assert!(!told.contains("not a key at all"), "{told}");
        },
    );
}

// ============================================================================
// The vocabulary, and the promise about what a diagnostic may say
// ============================================================================

/// Every scope-and-field pair names a setting the registry declares. A
/// `format!`-built key would pass compilation and resolve to nothing.
#[test]
fn every_scope_and_field_names_a_declared_setting() {
    for scope in AppScope::all() {
        for field in Field::all() {
            let key = setting_key(*scope, *field);
            assert!(
                crate::settings::registry::declaration(key).is_some(),
                "`{key}` is not declared in settings::registry"
            );
        }
    }
}

/// FR-023 and SC-007 in one assertion: a diagnostic may help an operator and
/// must not help someone reading their logs. **A length is information about a
/// secret and is excluded deliberately.**
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
        assert!(!guidance.contains(&secret.len().to_string()), "{guidance}");
        assert!(!guidance.contains("length"), "{guidance}");
        assert!(guidance.contains("github_app."), "{guidance}");
    }
    let joined = join_problems(&problems);
    assert!(!joined.contains("abcdef"), "{joined}");
    assert!(!stepped_over_guidance(AppScope::Feedback, &problems).contains("abcdef"));
}

/// The global scope has no fallback of its own — it *is* the fallback — and
/// says so in its own terms rather than pointing at itself twice.
#[test]
fn the_global_scope_has_no_fallback_of_its_own() {
    let settings = Settings::default();
    let problems = registration_for(AppScope::Global, &settings)
        .err()
        .expect("nothing is configured");
    assert_eq!(problems.len(), 3);
    assert!(missing_keys(&problems).contains(&"github_app.global.client_id"));
    let guidance = join_problems(&problems);
    assert!(
        !guidance.contains("Neither"),
        "the global points at itself: {guidance}"
    );
}

/// An instance with nothing set names both places a value could go — the
/// posture spec 037's delivery already took, kept.
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

/// The strings a GraphQL argument arrives as, round-tripped. A scope or a
/// field this cannot name is a mutation nobody can call.
#[test]
fn every_scope_and_field_round_trips_through_its_wire_name() {
    for scope in AppScope::all() {
        assert_eq!(AppScope::parse(scope.as_str()), Some(*scope));
    }
    for field in Field::all() {
        assert_eq!(Field::parse(field.as_str()), Some(*field));
    }
    assert_eq!(AppScope::parse("Global"), None);
    assert_eq!(Field::parse("private_key_file"), None);
}

/// FR-022 at the moment a value is typed. The parser is the real one, so a
/// value this accepts is a value resolution accepts.
#[test]
fn a_value_that_is_not_a_key_is_refused_on_save() {
    assert!(parse_private_key_for_storage(KEY_PEM).is_ok());
    assert!(parse_private_key_for_storage(&KEY_PEM.replace('\n', "\\n")).is_ok());
    assert!(
        parse_private_key_for_storage(&base64::engine::general_purpose::STANDARD.encode(KEY_PEM))
            .is_ok(),
        "the .env-safe form must survive being pasted into the admin screen too"
    );

    let refused = parse_private_key_for_storage("not a key at all")
        .expect_err("a presence check would have called this configured");
    assert!(!refused.contains("not a key at all"), "{refused}");
}
