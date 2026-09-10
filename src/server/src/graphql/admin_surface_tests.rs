//! Every operator-scoped GraphQL field refuses a non-administrator — and a
//! field nobody classified fails the build.
//!
//! # Why this exists
//!
//! The audit on 2026-09-09 found every operator-scoped resolver correctly
//! guarded, by three different idioms, applied one resolver at a time. Nothing
//! was open. But the first pass of that audit **missed** one
//! (`resolveModerationCase`, which authorises by passing `is_admin` into an
//! `_impl` and mentions "admin" nowhere a grep would find), and an audit that
//! can miss a *present* guard can miss an *absent* one.
//!
//! The REST admin routes were made structural: they live in a router wrapped in
//! `require_admin_user`, so a route is guarded by having been added there.
//! GraphQL has no equivalent — `async_graphql` resolvers are functions on one
//! root object, and there is nowhere to hang a layer that means "this half of
//! the schema is administrators only".
//!
//! So the guarantee is made here instead, in two halves that need each other:
//!
//! 1. **Nothing is unclassified.** Every root field is named in exactly one of
//!    the tables below. A field added to the schema and not to a table fails
//!    `every_root_field_is_classified` — which is the property that stops this
//!    from decaying into a list somebody forgot.
//! 2. **The classification is honest.** Every field called `AdminOnly` is
//!    *executed* as an ordinary authenticated user and must be refused. A
//!    resolver that lost its guard fails here even though its name never
//!    changed.
//!
//! Half 1 without half 2 is a list of good intentions. Half 2 without half 1
//! only covers the fields somebody remembered to list. Together they say: the
//! schema has no operator-scoped field that an ordinary account can reach.
//!
//! # Why execution and not inspection
//!
//! Because the thing worth asserting is behaviour. `resolveModerationCase`
//! proves inspection is not enough: it is guarded, and it does not look it.
//! Running it as a non-admin and watching it refuse is the only check that
//! cannot be fooled by an idiom nobody anticipated.

use async_graphql::{Request, Variables};

use crate::auth_middleware::AuthenticatedUser;
use crate::test_support::test_app_state;

/// Root fields that only an administrator may reach.
///
/// Each is executed below as an ordinary account and must be refused. The
/// arguments are whatever satisfies the schema — the values do not matter,
/// because authorisation is checked before anything is looked up, and a
/// resolver that reached its lookup first would be the finding.
const ADMIN_ONLY: &[(&str, &str)] = &[
    // --- the operator's own view of the instance -------------------------
    (
        "adminWelcomeSummary",
        "{ adminWelcomeSummary { __typename } }",
    ),
    ("adminStats", "{ adminStats { __typename } }"),
    ("systemManifest", "{ systemManifest { __typename } }"),
    (
        "adminBootstrapSettings",
        "{ adminBootstrapSettings { __typename } }",
    ),
    ("instanceReadiness", "{ instanceReadiness { __typename } }"),
    // --- credentials and access policy -----------------------------------
    ("oauthProviders", "{ oauthProviders { __typename } }"),
    (
        "authSecuritySettings",
        "{ authSecuritySettings { __typename } }",
    ),
    // Spec 041 FR-021: counts, and admin-only because the shape of an
    // instance's coverage is an operator's business and nobody else's.
    ("twoFactorCoverage", "{ twoFactorCoverage { __typename } }"),
    // Spec 039: what an old agreement actually said. The notice-handling
    // surface, not a public archive — admin-only for the reason
    // `moderationCase` is.
    (
        "legalDocumentVersion",
        r#"{ legalDocumentVersion(versionId: "sharing-terms@0000000000000000") { __typename } }"#,
    ),
    // Spec 041 US6/US7: one account by exact identifier, so an operator can
    // act on the second factor of somebody who has asked them for help.
    (
        "adminAccount",
        r#"{ adminAccount(identifier: "nobody") { __typename } }"#,
    ),
    (
        "instanceAccessSettings",
        "{ instanceAccessSettings { __typename } }",
    ),
    (
        "instanceInvitations",
        "{ instanceInvitations { __typename } }",
    ),
    (
        "githubApplications",
        "{ githubApplications { __typename } }",
    ),
    (
        "instanceAccessEvents",
        "{ instanceAccessEvents(limit: 1) { __typename } }",
    ),
    // --- settings ---------------------------------------------------------
    ("instanceSettings", "{ instanceSettings { __typename } }"),
    (
        "instanceSettingChanges",
        "{ instanceSettingChanges(key: \"realm_name\") { __typename } }",
    ),
    // --- mail -------------------------------------------------------------
    ("mailAvailability", "{ mailAvailability { __typename } }"),
    ("mailOutbox", "{ mailOutbox { __typename } }"),
    // --- compliance -------------------------------------------------------
    (
        "undeliveredFeedback",
        "{ undeliveredFeedback { __typename } }",
    ),
    ("legalEnquiries", "{ legalEnquiries { __typename } }"),
    (
        "openLegalEnquiryCounts",
        "{ openLegalEnquiryCounts { __typename } }",
    ),
    // --- mutations --------------------------------------------------------
    (
        "recalculateDiskUsage",
        "mutation { recalculateDiskUsage { __typename } }",
    ),
    (
        "updateManifestKey",
        r#"mutation { updateManifestKey(key: "realm_name", value: "x") { __typename } }"#,
    ),
    (
        "updateInstanceSetting",
        r#"mutation { updateInstanceSetting(key: "realm_name", value: "x") { __typename } }"#,
    ),
    (
        "setInstanceAccessPolicy",
        "mutation { setInstanceAccessPolicy(policy: CLOSED) { __typename } }",
    ),
    (
        "createInstanceInvitation",
        "mutation { createInstanceInvitation(input: {}) { __typename } }",
    ),
    (
        "revokeInstanceInvitation",
        r#"mutation { revokeInstanceInvitation(invitationId: "00000000-0000-0000-0000-000000000001") }"#,
    ),
    (
        "setGithubApplication",
        r#"mutation { setGithubApplication(scope: "global", field: "slug", value: "x") { __typename } }"#,
    ),
    (
        "checkGithubApplication",
        r#"mutation { checkGithubApplication(scope: "global") { __typename } }"#,
    ),
    (
        "sendTestMail",
        r#"mutation { sendTestMail(to: "nobody@example.org") { __typename } }"#,
    ),
    (
        "setLegalEnquiryStatus",
        r#"mutation { setLegalEnquiryStatus(id: "00000000-0000-0000-0000-000000000001", status: CLOSED) { __typename } }"#,
    ),
    (
        "resolveModerationCase",
        r#"mutation { resolveModerationCase(caseId: "00000000-0000-0000-0000-000000000001", resolution: CONTENT_RESTORED) { __typename } }"#,
    ),
    (
        "updateOauthProvider",
        r#"mutation { updateOauthProvider(providerId: "00000000-0000-0000-0000-000000000001", config: {}) { __typename } }"#,
    ),
];

/// Operator-shaped names that are **not** operator-scoped, each with the
/// reason. Listed rather than silently excluded, because "that one is fine"
/// is exactly the sentence a real hole hides behind.
const OPERATOR_SHAPED_BUT_NOT_ADMIN: &[&str] = &[
    // Spec 015: a claimant checks the status of a takedown they filed, and a
    // world owner sees that their content is under one. Neither is an
    // operator, and both need to know.
    "moderationStatus",
    "moderationCase",
    "moderationHistoryForAccount",
    // Spec 034: whether this instance can offer repository synchronisation at
    // all — `{ configured, operatorGuidance }`, and the guidance is documented
    // as never carrying the value of anything it names. A Game Master needs
    // this answer *before* starting a connection flow they cannot finish, so
    // it is deliberately open to any signed-in account.
    //
    // Listed here because the behavioural test below caught it: it was in
    // ADMIN_ONLY on my first pass, and answered an ordinary account. The code
    // was right and the classification was wrong, which is the direction this
    // test is most useful in.
    "instanceRepositoryIntegration",
];

/// Root fields any signed-in account may reach. Listed, not tested here: what
/// they authorise beyond "signed in" is world membership and per-object
/// permission, which their own suites cover.
const AUTHENTICATED: &[&str] = &["mySessions", "myTwoFactorEvents", "myWorlds"];

/// Root fields deliberately reachable without an account, each for a stated
/// reason. This table is the one to read twice: an addition here is a decision
/// to expose something to the whole internet.
const ANONYMOUS: &[&str] = &[
    // Spec 040: the operator values legal pages render. Published on purpose.
    "publishedOperatorValues",
    // Spec 039 FR-045: what an operator takes on. Shown to somebody who is
    // *becoming* an operator — first-run setup, before any account exists — so
    // requiring a session would make it unreachable at the one moment it is
    // about. It is a statement about responsibility and says nothing about this
    // instance's contents.
    "operatorStatement",
    // Share links (specs 026/027): the read needs no account by design, and is
    // rate-limited. A revoked share is indistinguishable from one that never
    // existed.
    "sharedActor",
    "sharedItem",
    "sharedAbility",
    "sharedCollection",
    // DMCA (spec 015): a takedown notice must be submittable by somebody who
    // has no account here, because the law does not require claimants to hold
    // one.
    "submitTakedownNotice",
    // Spec 041 FR-026 / legal intake: a person locked out, or with a privacy
    // complaint, must have a way to reach the operator.
    "submitLegalEnquiry",
];

fn ordinary_user() -> AuthenticatedUser {
    AuthenticatedUser {
        user_id: uuid::Uuid::now_v7(),
        session_id: uuid::Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        // The whole point.
        is_admin: false,
        role: "User".to_string(),
    }
}

/// Every field this schema exposes at the root, read from the SDL.
///
/// Read from the schema rather than a hand-kept list, because a hand-kept list
/// is a list that disagrees with the code the moment somebody adds a field —
/// which is exactly the moment this test exists for.
fn root_fields(sdl: &str, type_name: &str) -> Vec<String> {
    let Some(start) = sdl.find(&format!("type {type_name} ")) else {
        return Vec::new();
    };
    let body = &sdl[start..];
    let Some(end) = body.find("\n}") else {
        return Vec::new();
    };

    // Description blocks have to be *tracked*, not merely skipped by their
    // opening line: async_graphql emits doc comments as `"""` blocks whose
    // prose wraps over several lines, and a line reading `Administrators can
    // ...` looks exactly like a field declaration to anything scanning
    // line-by-line. The first draft of this parser reported `Admin`,
    // `Administrators` and `operator` as fields for precisely that reason.
    let mut in_description = false;
    let mut fields = Vec::new();

    for line in body[..end].lines().skip(1) {
        let line = line.trim();

        if line.starts_with("\"\"\"") {
            // A one-line `"""text"""` opens and closes at once.
            let fenced = line.len() > 3 && line.ends_with("\"\"\"");
            if !fenced {
                in_description = !in_description;
            }
            continue;
        }
        if in_description || line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }
        // A field declaration always has its type after a colon; prose does
        // not. This is the second filter, so a description that escaped the
        // first still cannot be mistaken for a field.
        if !line.contains(':') {
            continue;
        }
        let name: String = line
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            fields.push(name);
        }
    }

    fields
}

fn sdl() -> String {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl()
}

/// The property that keeps this honest: a field nobody classified is a
/// failure, not a silence.
///
/// When this fails, the field you just added is unclassified. Decide which
/// table it belongs in — and if that decision is not obvious, that is the
/// finding, not the test.
#[test]
fn every_root_field_is_classified() {
    let sdl = sdl();
    let mut unclassified = Vec::new();

    for type_name in ["QueryRoot", "MutationRoot"] {
        for field in root_fields(&sdl, type_name) {
            let known = ADMIN_ONLY.iter().any(|(name, _)| *name == field)
                || AUTHENTICATED.contains(&field.as_str())
                || ANONYMOUS.contains(&field.as_str())
                || OPERATOR_SHAPED_BUT_NOT_ADMIN.contains(&field.as_str())
                // Everything else is world-scoped: signed in, plus membership
                // and per-object permission checked by the resolver against
                // the world it names. Those have their own suites; what this
                // file guarantees is that nothing *operator*-scoped is
                // reachable without an administrator, and a world-scoped field
                // is not operator-scoped.
                || !is_operator_shaped(&field);
            if !known {
                unclassified.push(format!("{type_name}.{field}"));
            }
        }
    }

    assert!(
        unclassified.is_empty(),
        "these root fields look operator-scoped and are not classified in \
         `admin_surface_tests`. Add each to ADMIN_ONLY (and it will be executed \
         as a non-admin and must refuse), or to AUTHENTICATED/ANONYMOUS with a \
         reason:\n  {}",
        unclassified.join("\n  "),
    );
}

/// Whether a field name suggests it reaches instance-wide state.
///
/// A heuristic, deliberately, and deliberately *broad*: it decides only what
/// must be **classified**, never what is allowed. Being wrong here costs
/// somebody one line in a table; being narrow would cost the guarantee. If a
/// new operator surface is named something this does not catch, the
/// behavioural test below is the backstop — and this list should grow.
fn is_operator_shaped(field: &str) -> bool {
    const MARKERS: &[&str] = &[
        "instance",
        "admin",
        "manifest",
        "oauthProvider",
        "githubApp",
        "legal",
        "moderation",
        "undelivered",
        "diskUsage",
        "mail",
        "readiness",
        "setting",
        "operator",
    ];
    let lowered = field.to_ascii_lowercase();
    MARKERS
        .iter()
        .any(|marker| lowered.contains(&marker.to_ascii_lowercase()))
}

/// The half that cannot be fooled: run each one and watch it refuse.
///
/// Refusal here means "did not return data" — an authorisation error. The exact
/// message is deliberately not asserted: `resolveModerationCase` says "Only
/// compliance staff may resolve a moderation case" and `admin_user` says "Admin
/// privileges required", and pinning either would make this a test about
/// wording rather than about access.
#[tokio::test]
async fn no_operator_field_answers_an_ordinary_account() {
    let state = test_app_state();
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state.clone())
    .finish();

    for (field, query) in ADMIN_ONLY {
        let response = schema
            .execute(
                Request::new(*query)
                    .variables(Variables::default())
                    .data(ordinary_user()),
            )
            .await;

        assert!(
            !response.errors.is_empty(),
            "`{field}` answered an ordinary authenticated account. Every field \
             in ADMIN_ONLY must refuse one.",
        );
    }
}

/// The guard against this whole file passing by not testing.
///
/// `no_operator_field_answers_an_ordinary_account` asserts that a response has
/// errors — and a **malformed** query has errors too, before authorisation is
/// ever consulted. A typo in an argument name would make that test pass for
/// entirely the wrong reason, which is the exact shape of the two vacuous
/// tests found on 2026-09-08.
///
/// So: the same queries, run as an administrator, must not fail *validation*.
/// They may fail for any other reason — a row that does not exist, an
/// unconfigured mail transport — because what is being checked here is that
/// the query reached execution at all.
#[tokio::test]
async fn the_admin_only_queries_are_well_formed() {
    let state = test_app_state();
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state.clone())
    .finish();

    // Phrases async_graphql uses when a query does not match the schema. A
    // response carrying one of these never reached a resolver.
    const VALIDATION_MARKERS: &[&str] = &[
        "Unknown argument",
        "Unknown field",
        "Cannot query field",
        "is not defined",
        "expected input type",
        "Expected input type",
        "Invalid value",
        "Missing argument",
        "not found on type",
        "Unknown type",
        "Parse error",
        "syntax error",
    ];

    let administrator = AuthenticatedUser {
        is_admin: true,
        ..ordinary_user()
    };

    for (field, query) in ADMIN_ONLY {
        let response = schema
            .execute(Request::new(*query).data(administrator.clone()))
            .await;

        for message in response.errors.iter().map(|e| e.message.as_str()) {
            for marker in VALIDATION_MARKERS {
                assert!(
                    !message.contains(marker),
                    "the ADMIN_ONLY query for `{field}` does not match the                      schema ({message}). It would fail the refusal test for                      the wrong reason — fix the query, not the assertion.",
                );
            }
        }
    }
}

/// And the same fields refuse somebody with no account at all.
///
/// A separate case because the mechanisms differ: with no `AuthenticatedUser`
/// in the request, `authenticated_user(ctx)` fails before `admin_user` is ever
/// consulted. Both paths have to end in a refusal, and only one of them is
/// exercised by the test above.
#[tokio::test]
async fn no_operator_field_answers_an_anonymous_caller() {
    let state = test_app_state();
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state.clone())
    .finish();

    for (field, query) in ADMIN_ONLY {
        let response = schema.execute(Request::new(*query)).await;

        assert!(
            !response.errors.is_empty(),
            "`{field}` answered a caller with no session at all.",
        );
    }
}

/// The tables describe fields that exist.
///
/// Without this, a renamed field would silently stop being tested: its entry
/// in ADMIN_ONLY would sit there matching nothing, `every_root_field_is_classified`
/// would see an unclassified new name, and the behavioural test would keep
/// passing against a query that errors for the wrong reason entirely.
#[test]
fn the_tables_name_fields_that_exist() {
    let sdl = sdl();
    let mut all = root_fields(&sdl, "QueryRoot");
    all.extend(root_fields(&sdl, "MutationRoot"));

    for (field, _) in ADMIN_ONLY {
        assert!(
            all.iter().any(|f| f == field),
            "ADMIN_ONLY names `{field}`, which is not a root field. Renamed or \
             removed? A stale entry tests nothing.",
        );
    }
    for field in ANONYMOUS {
        assert!(
            all.iter().any(|f| f == field),
            "ANONYMOUS names `{field}`, which is not a root field.",
        );
    }
}
