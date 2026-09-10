//! What a disabled account can still reach — exactly the allowlist, and no
//! more (spec 039 T067, FR-031).
//!
//! # Two halves, for the reason `admin_surface_tests` has two
//!
//! 1. **The allowlist is closed.** The crate source is walked for every call to
//!    `authenticated_user_even_if_disabled`, and the set of call sites must be
//!    exactly the six in `contracts/standing-and-termination.md`. Adding a
//!    seventh fails here — widening what a disabled person can do is a
//!    decision, and this is where it has to be made on purpose (the sixth,
//!    `submitCounterNotice`, was: 2026-09-10).
//! 2. **The allowlist is honest.** Each allowed surface answers a disabled
//!    account, and an ordinary field refuses one with the sentence that sends
//!    them to their remedies.

use async_graphql::Request;

use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::helpers::ACCOUNT_DISABLED;
use crate::test_support::{insert_test_user, test_app_state};

/// `(file under src/, surface)` — the six a disabled account may reach.
const ALLOWLIST: &[(&str, &str)] = &[
    ("graphql/queries/user.rs", "exportMyData"),
    ("graphql/queries/standing.rs", "myStanding"),
    ("graphql/queries/standing.rs", "myNotices"),
    ("graphql/queries/legal.rs", "myAttestations"),
    ("graphql/mutations_standing.rs", "fileAppeal"),
    // The statutory route back — decided 2026-09-10.
    ("graphql/mutations_moderation.rs", "submitCounterNotice"),
];

fn call_sites() -> Vec<String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read src") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, root, out);
                continue;
            }
            let name = path.to_string_lossy();
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .expect("under src")
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "graphql/helpers.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read");
            // Whole-name calls only: `require_authenticated_user_even_if_disabled`,
            // the REST layer, contains this name and is not a GraphQL surface.
            let needle = "authenticated_user_even_if_disabled(";
            for (at, _) in source.match_indices(needle) {
                let preceded_by_identifier = source[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                if !preceded_by_identifier {
                    out.push(relative.clone());
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    walk(&root, &root, &mut found);
    found.sort();
    found
}

/// Half 1: exactly five call sites, in exactly these files.
#[test]
fn only_the_allowlisted_surfaces_admit_a_disabled_account() {
    let mut expected: Vec<String> = ALLOWLIST.iter().map(|(file, _)| file.to_string()).collect();
    expected.sort();
    assert_eq!(
        call_sites(),
        expected,
        "`authenticated_user_even_if_disabled` is called somewhere the allowlist in \
         `contracts/standing-and-termination.md` does not name. A disabled account may \
         download, see its standing, notices and agreements, and appeal — nothing else. \
         If a new surface belongs on that list, change the contract and this table \
         together.",
    );
}

fn disabled(user_id: uuid::Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: uuid::Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: true,
    }
}

/// Half 2: the allowed surfaces answer, and an ordinary one refuses — with the
/// sentence that says where the remedies are.
#[tokio::test]
async fn a_disabled_account_reaches_its_remedies_and_nothing_else() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let account = insert_test_user(&mut conn);
    drop(conn);

    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish();

    for (surface, query) in [
        ("exportMyData", "{ exportMyData { user { id } } }"),
        ("myStanding", "{ myStanding { disabled } }"),
        ("myNotices", "{ myNotices { id } }"),
        ("myAttestations", "{ myAttestations { id } }"),
    ] {
        let response = schema
            .execute(Request::new(query).data(disabled(account)))
            .await;
        assert!(
            response.errors.is_empty(),
            "`{surface}` must answer a disabled account: {:?}",
            response.errors,
        );
    }

    // `fileAppeal` reaches its own logic — this account has no window, so it is
    // refused for that, not for being disabled.
    let appeal = schema
        .execute(
            Request::new(r#"mutation { fileAppeal(statement: "Mine.") { appealState } }"#)
                .data(disabled(account)),
        )
        .await;
    assert!(
        appeal.errors.iter().all(|e| e.message != ACCOUNT_DISABLED),
        "fileAppeal must not be refused for the disablement it exists to contest",
    );

    // Likewise the counter-notice: refused here because the case does not
    // exist, never because the account is disabled.
    let counter_notice = schema
        .execute(
            Request::new(
                r#"mutation { submitCounterNotice(input: {
                    caseId: "00000000-0000-0000-0000-000000000000",
                    removedMaterialDescription: "Mine.",
                    goodFaithMistakeStatement: true,
                    consentToJurisdiction: true,
                    contactInformation: "me@realdomain.org",
                    signature: "Me"
                }) { currentStatus } }"#,
            )
            .data(disabled(account)),
        )
        .await;
    assert!(
        counter_notice
            .errors
            .iter()
            .all(|e| e.message != ACCOUNT_DISABLED),
        "a disabled account keeps the statutory route back",
    );

    let refused = schema
        .execute(Request::new("{ myWorlds { id } }").data(disabled(account)))
        .await;
    assert_eq!(
        refused.errors.first().map(|e| e.message.as_str()),
        Some(ACCOUNT_DISABLED),
        "everything else refuses, and says where the remedies are",
    );
}
