//! Saying, in public, that the platform has withdrawn — and nothing else.
//!
//! # What this does, and the much larger set of things it does not
//!
//! When a moderation takedown disables content that was mirrored to a
//! **publicly visible** repository, FR-040b requires the platform to lodge an
//! issue there recording three facts: it disabled the content at the source,
//! it has stopped exporting it, and it no longer associates itself with what
//! remains.
//!
//! FR-040c then says that is the **entire** extent of the action. Nothing here
//! deletes, edits, force-pushes, closes, or reads a repository's contents. The
//! only call this module makes is "open one issue", and the only reason that
//! call exists is that the alternative — saying nothing, on a repository
//! anyone can read, about material the platform put there — is worse.
//!
//! # Why the issue says so little
//!
//! It names no complainant: publishing a party to a dispute into a public
//! forum they did not choose is a harm the platform has no reason to cause.
//!
//! It asserts no infringement: the platform disabled content **on receipt of a
//! notice**, it did not adjudicate one, and it has no standing to. Claiming
//! infringement in public would be a claim it cannot support, and would pull
//! it *into* the dispute rather than out of it.
//!
//! It reproduces no content and pinpoints no entry: doing so in a public issue
//! would republish the very thing being withdrawn.
//!
//! The body is fixed prose in [`disassociation_body`] for exactly that reason.
//! A body assembled from a takedown's fields would eventually carry one of
//! those three, and no reviewer would notice the day it started to.
//!
//! # Never on a private repository
//!
//! FR-040c. A private repository already limits the exposure the issue exists
//! to address, and writing into someone's private workspace to announce a
//! withdrawal nobody else can see is an intrusion with no purpose. That
//! refusal is recorded as `skipped_private` rather than left absent, so that
//! "we deliberately did not" and "we forgot" never look the same a year later.
//!
//! # A failure here never touches the takedown
//!
//! FR-040d. The takedown is applied on the platform before this module is
//! reached and is not conditional on it; [`disassociate_after_takedown`]
//! therefore returns an [`Outcome`] and **cannot** return an error, because a
//! `?` in a caller is all it would take to make a revoked grant on someone
//! else's repository silently reverse a moderation decision. The obligation is
//! to attempt it and to say plainly when the attempt failed — which is what
//! the `lore_disassociation_notices` row is for.
//!
//! # The split
//!
//! The decision — should we lodge, and what does the body say — is pure, and
//! tested without a database or a network. The HTTP is a thin caller at the
//! bottom of this file. FR-040b and FR-040c are rules about the *decision*, so
//! that is the half that has to be testable; a rule only reachable through a
//! live host is a rule that is hoped for rather than checked.

use chrono::NaiveDate;
use diesel::prelude::*;
use uuid::Uuid;

use crate::models::{LoreDisassociationNotice, LoreRepositoryConnection};

/// The title of every disassociation issue, fixed by
/// `contracts/repository-file-format.md`.
///
/// It leads with what happened rather than with the platform's name, because
/// the person most likely to read it is the repository's owner scanning a list
/// of issue titles, and "content removed at source" is the part they need
/// before they open anything.
pub const DISASSOCIATION_ISSUE_TITLE: &str =
    "Content removed at source — ThunderForge no longer associates with this mirror";

/// Whether to lodge an issue on a repository, given what was observed of its
/// visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Observed public. FR-040b applies.
    Lodge,
    /// Observed private. FR-040c forbids the issue.
    SkipPrivate,
    /// Never observed. Not the same as either of the above, and treated as
    /// neither — see [`decide`].
    VisibilityUnknown,
}

/// What to do about a repository whose recorded visibility is `observed`.
///
/// Three answers rather than two, because `None` is a real state: FR-040a says
/// visibility is an *observation*, and a connection that has never completed a
/// pass has none. FR-037a's "a private repository must not be assumed" and
/// FR-040c's "never on a private repository" pull in opposite directions on an
/// unknown, and the only move that obeys both is to lodge nothing and record
/// that we could not carry the obligation out.
///
/// Recording an unknown as `skipped_private` would be the tempting shortcut
/// and is the one thing that must not happen: it would assert in the permanent
/// record that we knew the repository was private, when what we knew was
/// nothing. That row would read as a deliberate, correct decision forever.
pub fn decide(observed_public: Option<bool>) -> Decision {
    match observed_public {
        Some(true) => Decision::Lodge,
        Some(false) => Decision::SkipPrivate,
        None => Decision::VisibilityUnknown,
    }
}

/// The body of the disassociation issue.
///
/// Fixed prose from the contract, with the world's name and the date the
/// content was disabled carried in a header above it. The world's name is a
/// name its owner chose (the same licence FR-036j gives the binding record),
/// and a date is not content — between them they are what makes an issue
/// answerable a year later, when "which mirror, and when" is the only question
/// anyone still has.
///
/// Everything the body deliberately omits is listed in this module's header.
/// The one-line summary: it is the platform recording its own withdrawal, and
/// a body that said anything more would be the platform taking a position in a
/// dispute it is not party to.
pub fn disassociation_body(world_name: &str, disabled_on: NaiveDate) -> String {
    format!(
        "This repository contains lore mirrored from a ThunderForge world.\n\
         \n\
         **World:** {world_name}\n\
         **Disabled at source:** {}\n\
         \n\
         Content in that world has been disabled following a copyright notice. \
         The ThunderForge instance that wrote here has disabled it at the \
         source and has stopped exporting it. Nothing already committed to \
         this repository has been altered by us, and we will not alter it.\n\
         \n\
         We are recording that we no longer associate ourselves with the \
         material that remains in this repository. Its contents are published \
         by whoever owns this repository, and decisions about them — including \
         removal — rest with them.\n\
         \n\
         No claim about this repository's contents is made or implied here.\n",
        disabled_on.format("%Y-%m-%d"),
    )
}

/// What actually happened, in the three shapes the table's check constraint
/// allows.
///
/// The payloads live on the variants rather than beside them so that an
/// `issue_ref` cannot be attached to a failure, or a `failure_reason` to a
/// lodged notice, by a caller assembling the row by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Lodged, and here is where it landed so it can be pointed at.
    Lodged { issue_ref: String },
    /// Attempted and not lodged — a revoked grant, issues disabled on the
    /// repository, an unreachable host, or a connection we could not resolve
    /// to a repository at all. Also where [`Decision::VisibilityUnknown`]
    /// lands: an obligation we could not carry out is a failure, and the
    /// reason says which kind.
    Failed { reason: String },
    /// Deliberately not lodged, because the repository was observed private
    /// (FR-040c). Carries no reason because the outcome *is* the reason, and
    /// prose in `failure_reason` would make a correct decision read like a
    /// fault in every administrative listing that filters on that column.
    SkippedPrivate,
}

impl Outcome {
    /// The value written to `outcome`. These three strings are enforced by a
    /// check constraint in the migration; a fourth is a database error rather
    /// than a silently unrecognised row.
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Lodged { .. } => "lodged",
            Outcome::Failed { .. } => "failed",
            Outcome::SkippedPrivate => "skipped_private",
        }
    }

    fn issue_ref(&self) -> Option<String> {
        match self {
            Outcome::Lodged { issue_ref } => Some(issue_ref.clone()),
            _ => None,
        }
    }

    fn failure_reason(&self) -> Option<String> {
        match self {
            Outcome::Failed { reason } => Some(reason.clone()),
            _ => None,
        }
    }
}

/// Write one attempt into `lore_disassociation_notices` (FR-040d).
///
/// Every attempt, including the ones that did nothing. The table exists to
/// answer "did we, for this takedown, and if not why" a year later, and a
/// table that only holds successes cannot answer the half of that question
/// anyone would ask.
pub fn record_attempt(
    conn: &mut PgConnection,
    connection_id: Uuid,
    moderation_action_id: Uuid,
    outcome: &Outcome,
) -> Result<Uuid, String> {
    use crate::schema::lore_disassociation_notices as n;

    let id = Uuid::now_v7();
    diesel::insert_into(n::table)
        .values((
            n::id.eq(id),
            n::connection_id.eq(connection_id),
            n::moderation_action_id.eq(moderation_action_id),
            n::attempted_at.eq(chrono::Utc::now().naive_utc()),
            n::outcome.eq(outcome.label()),
            n::issue_ref.eq(outcome.issue_ref()),
            n::failure_reason.eq(outcome.failure_reason()),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to record a disassociation attempt: {e}"))?;
    Ok(id)
}

/// Every attempt that failed, newest first — the administrator's view.
///
/// FR-040d requires a failure to *reach* an administrator, and a row nobody
/// can query has not reached anyone. This is the query that surface is built
/// on, and it is here rather than in the caller so that "which notices need a
/// human" has one definition.
pub fn failed_notices(conn: &mut PgConnection) -> Result<Vec<LoreDisassociationNotice>, String> {
    use crate::schema::lore_disassociation_notices as n;

    n::table
        .filter(n::outcome.eq("failed"))
        .order(n::attempted_at.desc())
        .select(LoreDisassociationNotice::as_select())
        .load(conn)
        .map_err(|e| format!("Failed to load disassociation notices: {e}"))
}

/// Split a stored `owner/name` reference.
///
/// Refused rather than guessed when it does not have that shape. A malformed
/// reference sent to the host comes back as a 404, which reads exactly like a
/// revoked grant — and the difference matters, because one of those is fixed
/// by the operator and the other by the repository's owner.
pub fn split_repository_ref(repository_ref: &str) -> Option<(&str, &str)> {
    let (owner, name) = repository_ref.split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some((owner, name))
}

// ============================================================================
// The effects half: one POST, and nothing else.
// ============================================================================
/// Open the issue, through `repo_host`.
///
/// The HTTP deliberately does not live here. FR-004c confines host knowledge
/// to the grant boundary, and T061 checks it by grepping this directory for a
/// host's name — a request built here would pass that grep by avoiding a
/// vendor header while still being the thing the rule exists to prevent. What
/// matters to a second host is where the HTTP lives, not which strings it
/// contains.
async fn open_issue(connection: &LoreRepositoryConnection, body: &str) -> Result<String, String> {
    crate::repo_host::open_issue_for_connection(connection, DISASSOCIATION_ISSUE_TITLE, body).await
}

/// Attempt the disassociation for one connection, and record what happened.
///
/// **Returns no error, by design.** FR-040d: a failure here must not block or
/// reverse the takedown, and the surest way to guarantee that is to give the
/// caller nothing it can propagate. Everything that can go wrong — a private
/// repository, an unknown visibility, a malformed reference, a revoked grant,
/// a host that is down — comes back as an [`Outcome`] and goes into the table.
///
/// The takedown itself is applied by the moderation path before this is
/// reached, and is not conditional on the value returned here.
pub async fn disassociate_after_takedown(
    conn: &mut PgConnection,
    connection: &LoreRepositoryConnection,
    moderation_action_id: Uuid,
    world_name: &str,
    disabled_on: NaiveDate,
) -> Outcome {
    let outcome = match decide(connection.repository_is_public) {
        Decision::SkipPrivate => Outcome::SkippedPrivate,
        Decision::VisibilityUnknown => Outcome::Failed {
            reason: "The repository's visibility has never been observed, so it could not be \
                     established that lodging an issue would not write into a private workspace."
                .to_string(),
        },
        // The reference is still validated here rather than left to fail at
        // the boundary, because a malformed one is a *configuration* problem
        // this module can name precisely — "not an owner/name reference" —
        // where the same failure arriving from `repo_host` would read as the
        // host being unreachable. The split's result is discarded; splitting
        // for real is the grant boundary's job (FR-004c).
        Decision::Lodge if split_repository_ref(&connection.repository_ref).is_none() => {
            Outcome::Failed {
                reason: format!(
                    "\"{}\" is not an owner/name repository reference.",
                    connection.repository_ref
                ),
            }
        }
        Decision::Lodge => {
            let body = disassociation_body(world_name, disabled_on);
            match open_issue(connection, &body).await {
                Ok(issue_ref) => Outcome::Lodged { issue_ref },
                Err(reason) => Outcome::Failed { reason },
            }
        }
    };

    // A recording failure is the one thing this cannot itself record. It is
    // still not allowed to reach the takedown, so it is swallowed here and the
    // outcome is returned regardless — the caller's own logging is the last
    // line, and losing a row is strictly better than reversing a moderation
    // decision because the database blinked.
    let _ = record_attempt(conn, connection.id, moderation_action_id, &outcome);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 4).expect("a valid date")
    }

    // ------------------------------------------------------------------
    // The decision and the body. No database, no network — these are the
    // rules FR-040b and FR-040c actually state.
    // ------------------------------------------------------------------

    #[test]
    fn a_public_repository_is_lodged_against_and_a_private_one_is_not() {
        assert_eq!(decide(Some(true)), Decision::Lodge);
        assert_eq!(decide(Some(false)), Decision::SkipPrivate);
    }

    /// FR-040c plus FR-037a. An unobserved repository is not a private one and
    /// is not a public one, and collapsing it into either would be a guess
    /// recorded as a fact.
    #[test]
    fn an_unobserved_repository_is_neither() {
        assert_eq!(decide(None), Decision::VisibilityUnknown);
    }

    /// FR-040b. Everything in this test is a thing the body must not say, and
    /// each one is a different way of taking a position the platform has no
    /// standing to take.
    #[test]
    fn the_body_names_no_complainant_asserts_no_infringement_and_carries_no_content() {
        let body = disassociation_body("Westeros", a_date());

        // The world's own name and the date are the only substitutions.
        assert!(body.contains("Westeros"));
        assert!(body.contains("2026-09-04"));

        // No complainant. The body is fixed prose precisely so that no
        // takedown field can reach it, and this asserts the fixture's
        // complainant is nowhere in the output.
        assert!(!body.contains("Rights Holder"));
        assert!(!body.contains("complainant"));

        // No assertion of infringement. "a copyright notice" is a description
        // of what the platform received; "infringes" would be a finding.
        assert!(!body.to_lowercase().contains("infring"));
        assert!(!body.to_lowercase().contains("unlawful"));
        assert!(body.contains("No claim about this repository's contents is made or implied"));

        // No content, and no entry pinpointed — nothing that could republish
        // the material being withdrawn.
        assert!(
            !body.contains("]("),
            "a link that could carry content leaked"
        );
    }

    /// The three facts FR-040b requires the issue to state. Asserted in the
    /// words a person reads, because a body that merely mentioned a takedown
    /// would satisfy no part of the obligation.
    #[test]
    fn the_body_states_the_three_things_it_exists_to_state() {
        let body = disassociation_body("Westeros", a_date());
        assert!(body.contains("disabled it at the source"));
        assert!(body.contains("stopped exporting it"));
        assert!(body.contains("no longer associate ourselves"));
        // FR-040c, said out loud: we altered nothing and will not.
        assert!(body.contains("has been altered by us, and we will not alter it"));
    }

    #[test]
    fn the_title_is_the_one_the_contract_fixes() {
        assert_eq!(
            DISASSOCIATION_ISSUE_TITLE,
            "Content removed at source — ThunderForge no longer associates with this mirror"
        );
    }

    #[test]
    fn a_repository_reference_is_split_or_refused_rather_than_guessed() {
        assert_eq!(split_repository_ref("owner/name"), Some(("owner", "name")));
        assert_eq!(split_repository_ref("name"), None);
        assert_eq!(split_repository_ref("owner/"), None);
        assert_eq!(split_repository_ref("/name"), None);
        assert_eq!(split_repository_ref("owner/name/extra"), None);
    }

    /// The payloads are bound to the variants so a lodged notice cannot carry
    /// a failure reason, or a failure an issue reference.
    #[test]
    fn an_outcome_carries_only_the_fields_that_belong_to_it() {
        let lodged = Outcome::Lodged {
            issue_ref: "https://example.invalid/issues/1".to_string(),
        };
        assert_eq!(lodged.label(), "lodged");
        assert!(lodged.failure_reason().is_none());

        let failed = Outcome::Failed {
            reason: "issues are disabled".to_string(),
        };
        assert_eq!(failed.label(), "failed");
        assert!(failed.issue_ref().is_none());

        assert_eq!(Outcome::SkippedPrivate.label(), "skipped_private");
        assert!(Outcome::SkippedPrivate.issue_ref().is_none());
        assert!(Outcome::SkippedPrivate.failure_reason().is_none());
    }

    // ------------------------------------------------------------------
    // The record. These need a database, and nothing more.
    // ------------------------------------------------------------------

    mod recorded {
        use super::*;
        use crate::schema::{lore_disassociation_notices, lore_repository_connections};
        use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

        fn connection(
            conn: &mut PgConnection,
            observed_public: Option<bool>,
            repository_ref: &str,
        ) -> LoreRepositoryConnection {
            let owner = insert_test_user(conn);
            let world_id = insert_test_world(conn, owner);
            let now = chrono::Utc::now().naive_utc();
            let row = LoreRepositoryConnection {
                id: Uuid::now_v7(),
                world_id,
                host_kind: "test".to_string(),
                installation_ref: "test-installation".to_string(),
                repository_ref: repository_ref.to_string(),
                branch: "main".to_string(),
                // FR-033's unique (repository_ref, directory) is real, and
                // these rows outlive the test run — a fixed directory would
                // make the second run of this suite fail on a constraint that
                // has nothing to do with what is being tested.
                directory: format!("lore-{}", Uuid::now_v7().simple()),
                incoming_enabled: false,
                notice_acknowledged_at: Some(now),
                state: "working".to_string(),
                state_reason: None,
                repository_is_public: observed_public,
                visibility_checked_at: observed_public.map(|_| now),
                deactivated_at: None,
                deactivated_reason: None,
                last_synced_at: Some(now),
                last_written_commit: None,
                created_by: owner,
                updated_by: owner,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(lore_repository_connections::table)
                .values(row.clone())
                .execute(conn)
                .expect("insert connection");
            row
        }

        fn notices_for(
            conn: &mut PgConnection,
            connection_id: Uuid,
        ) -> Vec<LoreDisassociationNotice> {
            lore_disassociation_notices::table
                .filter(lore_disassociation_notices::connection_id.eq(connection_id))
                .select(LoreDisassociationNotice::as_select())
                .load(conn)
                .expect("load notices")
        }

        /// FR-040c. The refusal is a *row*, not an absence: an empty table
        /// cannot tell an auditor whether the platform decided not to act or
        /// never got as far as deciding.
        #[tokio::test]
        async fn a_private_repository_records_skipped_private_and_lodges_nothing() {
            let state = test_app_state();
            let conn = &mut state.db_pool.get().expect("a connection");

            let connection = connection(conn, Some(false), "owner/private-repo");
            let action = Uuid::now_v7();

            let outcome =
                disassociate_after_takedown(conn, &connection, action, "Westeros", a_date()).await;

            assert_eq!(outcome, Outcome::SkippedPrivate);
            let notices = notices_for(conn, connection.id);
            assert_eq!(notices.len(), 1);
            assert_eq!(notices[0].outcome, "skipped_private");
            assert_eq!(notices[0].moderation_action_id, action);
            assert!(notices[0].issue_ref.is_none());
            // Not a failure, so it must not appear as one in the column an
            // administrative surface filters on.
            assert!(notices[0].failure_reason.is_none());
        }

        /// FR-040d. The takedown is applied before this runs and is untouched
        /// by it — this asserts the only two things a failure is allowed to
        /// do: come back as a value, and land in the table with a reason.
        #[tokio::test]
        async fn a_failure_to_lodge_is_recorded_and_returns_rather_than_erroring() {
            let state = test_app_state();
            let conn = &mut state.db_pool.get().expect("a connection");

            // Public, so FR-040b applies and the attempt is made — and the
            // reference is one no host can be asked about, so the attempt
            // fails without a network call and without depending on whether
            // this machine has a repository host configured.
            let connection = connection(conn, Some(true), "not-an-owner-name-pair");
            let action = Uuid::now_v7();

            let outcome =
                disassociate_after_takedown(conn, &connection, action, "Westeros", a_date()).await;

            assert!(
                matches!(outcome, Outcome::Failed { .. }),
                "expected a failure, got {outcome:?}"
            );
            let notices = notices_for(conn, connection.id);
            assert_eq!(notices.len(), 1);
            assert_eq!(notices[0].outcome, "failed");
            assert!(
                notices[0]
                    .failure_reason
                    .as_deref()
                    .is_some_and(|r| !r.is_empty()),
                "a failure with no reason cannot reach an administrator"
            );

            // And it is visible in the administrator's view.
            let failures = failed_notices(conn).expect("load failures");
            assert!(failures.iter().any(|n| n.id == notices[0].id));
        }

        /// An unobserved visibility is a failure, not a skip. The distinction
        /// is the whole reason the outcome vocabulary has three words.
        #[tokio::test]
        async fn an_unobserved_repository_records_failed_not_skipped_private() {
            let state = test_app_state();
            let conn = &mut state.db_pool.get().expect("a connection");

            let connection = connection(conn, None, "owner/never-observed");
            let outcome = disassociate_after_takedown(
                conn,
                &connection,
                Uuid::now_v7(),
                "Westeros",
                a_date(),
            )
            .await;

            assert!(matches!(outcome, Outcome::Failed { .. }));
            let notices = notices_for(conn, connection.id);
            assert_eq!(notices.len(), 1);
            assert_eq!(notices[0].outcome, "failed");
        }

        /// A lodged notice keeps where it landed, so a human can be pointed at
        /// the issue rather than told one exists somewhere.
        #[test]
        fn a_lodged_notice_keeps_the_issue_it_landed_on() {
            let state = test_app_state();
            let conn = &mut state.db_pool.get().expect("a connection");

            let connection = connection(conn, Some(true), "owner/public-repo");
            let issue = "https://example.invalid/owner/public-repo/issues/7";
            record_attempt(
                conn,
                connection.id,
                Uuid::now_v7(),
                &Outcome::Lodged {
                    issue_ref: issue.to_string(),
                },
            )
            .expect("record");

            let notices = notices_for(conn, connection.id);
            assert_eq!(notices.len(), 1);
            assert_eq!(notices[0].outcome, "lodged");
            assert_eq!(notices[0].issue_ref.as_deref(), Some(issue));
            assert!(notices[0].failure_reason.is_none());
        }

        /// The check constraint is the last line: only the three known
        /// outcomes reach the table, and `label` is the only thing that writes
        /// that column.
        #[test]
        fn only_the_three_known_outcomes_are_written() {
            for label in [
                Outcome::Lodged {
                    issue_ref: "x".to_string(),
                }
                .label(),
                Outcome::Failed {
                    reason: "x".to_string(),
                }
                .label(),
                Outcome::SkippedPrivate.label(),
            ] {
                assert!(matches!(label, "lodged" | "failed" | "skipped_private"));
            }
        }
    }
}
