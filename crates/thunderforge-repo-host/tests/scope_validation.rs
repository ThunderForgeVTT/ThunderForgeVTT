//! FR-036a in tests: a grant covers exactly one repository, or it is refused.
//!
//! This is the rule with the largest gap between "obviously right" and
//! "actually enforced". A grant that covers an account's whole set of
//! repositories looks, in JSON, almost exactly like a grant that covers none —
//! the difference is a `repository_selection` field that a careless reader
//! would skip. So the account-wide case is tested first and separately, and
//! the property test below asserts the general shape: any count other than one
//! is a refusal.
//!
//! Also here: FR-036e's second permission. A test that asserts the presence of
//! the issue-write ask, and its reason, is what stops a future "narrow the
//! permissions" cleanup from quietly removing the thing that makes FR-040b's
//! public disassociation possible.

use proptest::prelude::*;
use thunderforge_repo_host::github::{GitHubApp, REQUESTED_PERMISSIONS};
use thunderforge_repo_host::{RepoHost, RepoHostError};

const TEST_KEY: &[u8] = include_bytes!("fixtures/throwaway-test-app-key.pem");

/// A GitHub App backed by the throwaway fixture key.
///
/// Nothing is registered anywhere: the key was generated for this test suite
/// and corresponds to no real application. That is the point — the whole crate
/// is testable with no application configured and no network reachable.
fn app() -> GitHubApp {
    GitHubApp::new("123456", "thunderforge-test", TEST_KEY).expect("the fixture key is valid RSA")
}

fn installation(selection: &str, repos: &[(&str, bool)]) -> String {
    let repos: Vec<String> = repos
        .iter()
        .map(|(full_name, private)| format!(r#"{{"full_name":"{full_name}","private":{private}}}"#))
        .collect();
    format!(
        r#"{{"id":42,"repository_selection":"{selection}","repositories":[{}]}}"#,
        repos.join(",")
    )
}

#[test]
fn one_repository_is_accepted_and_described_neutrally() {
    let (grant, repo) = app()
        .validate_grant(&installation("selected", &[("gm/our-world-lore", false)]))
        .expect("a single-repository grant is exactly what this feature asks for");

    assert_eq!(repo.owner, "gm");
    assert_eq!(repo.name, "our-world-lore");
    assert_eq!(repo.full_name(), "gm/our-world-lore");
    // FR-040a: visibility is recorded as observed at grant time.
    assert!(repo.public);
    // The grant is opaque: it can be persisted and read back, and that is all.
    assert_eq!(grant.to_string(), "42");
}

#[test]
fn a_private_repository_is_recorded_as_not_public() {
    let (_, repo) = app()
        .validate_grant(&installation("selected", &[("gm/private-lore", true)]))
        .expect("a private single repository is still a valid grant");
    assert!(!repo.public);
}

#[test]
fn an_account_wide_grant_is_refused_by_its_own_name() {
    // The dangerous case: `"all"` can legitimately arrive with an empty
    // repository list, and reading that as "zero repositories" would give the
    // worst outcome the mildest error message.
    assert_eq!(
        app().validate_grant(&installation("all", &[])).unwrap_err(),
        RepoHostError::GrantCoversAllRepositories
    );
    assert_eq!(
        app()
            .validate_grant(&installation("all", &[("gm/one", false)]))
            .unwrap_err(),
        RepoHostError::GrantCoversAllRepositories
    );
}

#[test]
fn a_broader_grant_is_refused_rather_than_narrowed() {
    // Narrowing after the fact would leave us holding access we promised not
    // to use, which is still access we hold.
    assert_eq!(
        app()
            .validate_grant(&installation(
                "selected",
                &[("gm/one", false), ("gm/two", false)]
            ))
            .unwrap_err(),
        RepoHostError::GrantNotSingleRepository { count: 2 }
    );
}

#[test]
fn an_empty_grant_is_refused_too() {
    assert_eq!(
        app()
            .validate_grant(&installation("selected", &[]))
            .unwrap_err(),
        RepoHostError::GrantNotSingleRepository { count: 0 }
    );
}

#[test]
fn a_malformed_payload_is_an_error_and_not_a_panic() {
    assert!(matches!(
        app().validate_grant("<html>502</html>"),
        Err(RepoHostError::MalformedResponse(_))
    ));
    assert!(matches!(
        app().validate_grant(r#"{"id":1,"repositories":[{"full_name":"no-slash"}]}"#),
        Err(RepoHostError::MalformedResponse(_))
    ));
}

#[test]
fn the_grant_asks_for_contents_write_and_issue_write() {
    // FR-036 (contents) and FR-036e (issues). Asserted here so that a future
    // "narrow the permissions" change has to confront FR-040b rather than
    // discover it after a takedown cannot be performed.
    let ids: Vec<&str> = REQUESTED_PERMISSIONS.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec!["contents:write", "issues:write"]);
}

#[test]
fn every_permission_carries_a_reason_the_user_can_read() {
    // FR-036e requires the user be shown *why* the second permission exists.
    // Carrying the reason in the same value as the ask is what makes a consent
    // screen unable to render one without the other; this asserts the values
    // are actually populated.
    for permission in REQUESTED_PERMISSIONS {
        assert!(!permission.summary.trim().is_empty(), "{}", permission.id);
        assert!(permission.reason.len() > 40, "{}", permission.id);
    }
    let issues = REQUESTED_PERMISSIONS
        .iter()
        .find(|p| p.id == "issues:write")
        .expect("FR-036e's permission must be present");
    assert!(issues.reason.contains("disabled"));
    assert!(issues.reason.contains("never delete"));
}

#[test]
fn the_hand_off_carries_the_permissions_it_must_display() {
    let handoff = app().grant_handoff("abc-123_XYZ").expect("a safe state");
    assert_eq!(
        handoff.url,
        "https://github.com/apps/thunderforge-test/installations/new?state=abc-123_XYZ"
    );
    assert_eq!(handoff.permissions.len(), 2);
}

#[test]
fn an_enterprise_deployment_can_move_both_hosts() {
    let app = app().with_bases(
        "https://git.example.org/",
        "https://git.example.org/api/v3/",
    );
    let handoff = app.grant_handoff("state").expect("a safe state");
    assert!(handoff.url.starts_with("https://git.example.org/apps/"));
}

#[test]
fn an_unusable_registration_is_reported_at_construction() {
    // FR-036c: an instance should know its registration is broken before a
    // Game Master presses "connect", not at that moment.
    assert!(matches!(
        GitHubApp::new("123", "slug", b"-----BEGIN PRIVATE KEY-----\nnope\n"),
        Err(RepoHostError::InvalidPrivateKey(_))
    ));
    assert_eq!(
        GitHubApp::new("  ", "slug", TEST_KEY).unwrap_err(),
        RepoHostError::MissingAppId
    );
    assert_eq!(
        GitHubApp::new("123", "", TEST_KEY).unwrap_err(),
        RepoHostError::NotConfigured
    );
}

proptest! {
    /// Any repository count other than exactly one is refused, whatever the
    /// selection mode says.
    #[test]
    fn only_a_count_of_one_is_ever_accepted(count in 0usize..8) {
        let repos: Vec<(String, bool)> = (0..count)
            .map(|i| (format!("gm/repo-{i}"), false))
            .collect();
        let borrowed: Vec<(&str, bool)> =
            repos.iter().map(|(n, p)| (n.as_str(), *p)).collect();
        let result = app().validate_grant(&installation("selected", &borrowed));
        prop_assert_eq!(result.is_ok(), count == 1);
    }

    /// A state that is not URL-safe is refused rather than escaped: this
    /// platform generates its own state, so an unsafe one is our bug and
    /// should not be allowed to travel.
    #[test]
    fn an_unsafe_hand_off_state_is_refused(state in "[&?# /=%\"'<>]{1,8}") {
        prop_assert_eq!(
            app().grant_handoff(&state).unwrap_err(),
            RepoHostError::UnsafeHandoffState
        );
    }

    /// A safe state always survives into the URL verbatim.
    #[test]
    fn a_safe_hand_off_state_is_carried_verbatim(state in "[A-Za-z0-9._~-]{1,40}") {
        let handoff = app().grant_handoff(&state).expect("safe by construction");
        let expected = format!("?state={state}");
        prop_assert!(handoff.url.ends_with(&expected));
    }

    /// Grant validation is total over arbitrary bodies.
    #[test]
    fn grant_validation_never_panics(body in ".{0,120}") {
        let _ = app().validate_grant(&body);
    }
}
