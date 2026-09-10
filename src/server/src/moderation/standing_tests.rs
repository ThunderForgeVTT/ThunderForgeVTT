//! The ladder, rung by rung, and the two ways a strike stops counting.

use super::*;
use crate::models::NewContentModerationAction;
use crate::moderation::action_type;
use crate::schema::{account_notices, content_moderation_actions};
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

/// The shipped defaults, stated rather than read from the environment.
const LADDER: Ladder = Ladder {
    warn_at: 1,
    suspend_publishing_at: 2,
    threshold: 3,
    lookback_days: 365,
    window_days: 30,
    requires_human: true,
};

fn a_strike() -> Strike {
    Strike {
        case_id: Uuid::now_v7(),
        entity_type: "world_item".to_string(),
        entity_id: Uuid::now_v7(),
        world_id: Uuid::now_v7(),
        recorded_at: Utc::now(),
    }
}

/// T055: 0, 1, 2 and 3 strikes. Warned at one, unable to publish at two, and
/// still unable at three — the ladder has no rung where publishing comes back
/// by getting worse.
#[test]
fn the_ladder_at_each_rung() {
    for (count, warned, may_publish) in [
        (0, false, true),
        (1, true, true),
        (2, true, false),
        (3, true, false),
    ] {
        let standing = Standing::from_strikes(LADDER, (0..count).map(|_| a_strike()).collect());
        assert_eq!(standing.strike_count(), count);
        assert_eq!(standing.warned(), warned, "warned at {count} strikes");
        assert_eq!(
            standing.may_publish, may_publish,
            "may publish at {count} strikes"
        );
        assert!(
            !standing.disabled,
            "nothing is disabled until US7 can open a termination"
        );
    }
}

/// An operator who sets the suspension rung above the threshold has not
/// thereby let an account at the threshold publish.
#[test]
fn a_suspension_rung_above_the_threshold_does_not_reopen_publishing() {
    let ladder = Ladder {
        suspend_publishing_at: 5,
        ..LADDER
    };
    let standing = Standing::from_strikes(ladder, (0..3).map(|_| a_strike()).collect());
    assert!(!standing.may_publish);
}

/// One case against `account`, upheld: a notice received and the content
/// disabled — what a valid takedown writes.
fn an_upheld_case(conn: &mut PgConnection, account: Uuid, world: Uuid) -> Uuid {
    let case_id = Uuid::now_v7();
    diesel::insert_into(content_moderation_actions::table)
        .values(NewContentModerationAction {
            case_id,
            action_type: action_type::CONTENT_DISABLED.to_string(),
            entity_type: "world_item".to_string(),
            entity_id: Uuid::now_v7(),
            world_id: world,
            account_id: Some(account),
            claimant_name: "Jane Claimant".to_string(),
            claimant_contact: "jane@realdomain.org".to_string(),
            copyrighted_work_description: "An original work.".to_string(),
            infringing_material_location: "An item.".to_string(),
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Jane Claimant".to_string(),
            validity_result: Some("valid".to_string()),
            missing_elements: None,
            counter_notice_id: None,
            restoration_due_at: None,
            created_by: None,
        })
        .execute(conn)
        .expect("case");
    case_id
}

fn an_account_in_a_world(conn: &mut PgConnection) -> (Uuid, Uuid) {
    let account = insert_test_user(conn);
    let world = insert_test_world(conn, account);
    (account, world)
}

fn notices_for(conn: &mut PgConnection, account: Uuid) -> Vec<(String, Option<serde_json::Value>)> {
    account_notices::table
        .filter(account_notices::account_id.eq(account))
        .order(account_notices::created_at.asc())
        .select((account_notices::kind, account_notices::payload))
        .load(conn)
        .expect("notices")
}

/// T055 / FR-035: a strike that ages past the lookback stops counting because
/// the calendar moved — and **nothing else happens**. No notice, no row
/// changed, nobody asked.
#[tokio::test]
async fn a_strike_ageing_past_the_lookback_restores_publishing_and_nothing_else() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, world) = an_account_in_a_world(&mut conn);
    an_upheld_case(&mut conn, account, world);
    let ageing = an_upheld_case(&mut conn, account, world);

    let suspended = standing_of_sync(&mut conn, account, LADDER).expect("standing");
    assert_eq!(suspended.strike_count(), 2);
    assert!(!suspended.may_publish);

    // A year and a bit ago. The only thing this test changes is when the
    // case happened, which is what the passage of time changes.
    diesel::update(content_moderation_actions::table)
        .filter(content_moderation_actions::case_id.eq(ageing))
        .set(content_moderation_actions::created_at.eq(Utc::now() - chrono::Duration::days(400)))
        .execute(&mut conn)
        .expect("age it");
    let rows_before: i64 = content_moderation_actions::table
        .filter(content_moderation_actions::account_id.eq(account))
        .count()
        .get_result(&mut conn)
        .expect("count");

    let restored = standing_of_sync(&mut conn, account, LADDER).expect("standing");
    assert_eq!(restored.strike_count(), 1);
    assert!(
        restored.may_publish,
        "publishing comes back on its own (FR-035)"
    );

    let rows_after: i64 = content_moderation_actions::table
        .filter(content_moderation_actions::account_id.eq(account))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(rows_before, rows_after, "reading a standing writes nothing");
    assert!(
        notices_for(&mut conn, account).is_empty(),
        "and tells nobody anything — an ageing strike is not an event",
    );
}

/// FR-020 / FR-027: restoration follows the existing process. A case resolved
/// in the person's favour stops counting, with no edit to any count.
#[tokio::test]
async fn a_restored_case_stops_counting() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, world) = an_account_in_a_world(&mut conn);
    an_upheld_case(&mut conn, account, world);
    let restored = an_upheld_case(&mut conn, account, world);
    assert!(
        !standing_of_sync(&mut conn, account, LADDER)
            .expect("standing")
            .may_publish
    );

    diesel::insert_into(content_moderation_actions::table)
        .values(NewContentModerationAction {
            case_id: restored,
            action_type: action_type::CONTENT_RESTORED.to_string(),
            entity_type: "world_item".to_string(),
            entity_id: Uuid::now_v7(),
            world_id: world,
            account_id: Some(account),
            claimant_name: String::new(),
            claimant_contact: String::new(),
            copyrighted_work_description: String::new(),
            infringing_material_location: String::new(),
            good_faith_statement: false,
            accuracy_statement: false,
            signature: String::new(),
            validity_result: None,
            missing_elements: None,
            counter_notice_id: None,
            restoration_due_at: None,
            created_by: None,
        })
        .execute(&mut conn)
        .expect("restore");

    let after = standing_of_sync(&mut conn, account, LADDER).expect("standing");
    assert_eq!(after.strike_count(), 1);
    assert!(after.may_publish);
}

/// FR-028: each strike is told — what it was, that it counts, how many remain,
/// when it stops counting — and the one that reaches the suspension rung says
/// so as well.
#[tokio::test]
async fn each_strike_is_told_and_the_suspending_one_says_so() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, world) = an_account_in_a_world(&mut conn);

    let first = an_upheld_case(&mut conn, account, world);
    tell_of_strike_sync(&mut conn, account, first, LADDER).expect("tell");
    let told = notices_for(&mut conn, account);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].0, notices::kind::STRIKE_RECORDED);
    let payload = told[0].1.clone().expect("payload");
    assert_eq!(payload["strikeCount"], 1);
    assert_eq!(payload["suspendPublishingAt"], 2);
    assert_eq!(payload["threshold"], 3);
    assert!(
        payload["agesOutAt"].is_string(),
        "when it stops counting is part of being told (FR-028)",
    );

    let second = an_upheld_case(&mut conn, account, world);
    tell_of_strike_sync(&mut conn, account, second, LADDER).expect("tell");
    let kinds: Vec<String> = notices_for(&mut conn, account)
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            notices::kind::STRIKE_RECORDED,
            notices::kind::STRIKE_RECORDED,
            notices::kind::PUBLISHING_SUSPENDED,
        ],
    );
}

/// A case that does not count is not a strike, and telling of it writes
/// nothing — an accusation is not a strike (FR-027).
#[tokio::test]
async fn a_case_that_does_not_count_tells_nobody_anything() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, _) = an_account_in_a_world(&mut conn);

    tell_of_strike_sync(&mut conn, account, Uuid::now_v7(), LADDER).expect("tell");
    assert!(notices_for(&mut conn, account).is_empty());
}

// ---------------------------------------------------------------------------
// US7: the window, the appeal, and the two remedies (T064–T066)
// ---------------------------------------------------------------------------

/// A ladder whose windows end without a person, so the sweep carries them out.
/// Every sweep below is scoped to one account: the database is shared, and an
/// unscoped sweep under this ladder would disable every account at the
/// threshold in it.
const TIMER_LADDER: Ladder = Ladder {
    requires_human: false,
    ..LADDER
};

/// Three strikes, and a window opened `days_ago` — so a window of thirty days
/// opened forty days ago is ten days past due.
fn a_window(conn: &mut PgConnection, days_ago: i64) -> (Uuid, AccountTermination) {
    let (account, world) = an_account_in_a_world(conn);
    for _ in 0..3 {
        an_upheld_case(conn, account, world);
    }
    let opened = open_termination_sync(
        conn,
        account,
        TIMER_LADDER,
        Utc::now() - chrono::Duration::days(days_ago),
    )
    .expect("open")
    .expect("three strikes open a window");
    (account, opened)
}

fn account_exists(conn: &mut PgConnection, account: Uuid) -> bool {
    use crate::schema::users;
    diesel::select(diesel::dsl::exists(
        users::table.filter(users::id.eq(account)),
    ))
    .get_result(conn)
    .expect("exists")
}

/// T064 / FR-034: an appeal open when the window ends **pauses** the deletion.
/// Deletion is never the outcome of the instance being slow to decide.
#[tokio::test]
async fn an_open_appeal_blocks_deletion_past_the_due_date() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, opened) = a_window(&mut conn, 40);
    assert!(opened.disables_account);
    assert!(
        opened.deletion_due_at < Utc::now(),
        "the fixture is past due"
    );

    file_appeal_sync(&mut conn, account, "The work is mine.", Utc::now()).expect("appeal");
    let report =
        sweep_scoped_sync(&mut conn, TIMER_LADDER, Utc::now(), Some(account)).expect("sweep");

    assert_eq!(report.deleted, 0);
    assert!(
        account_exists(&mut conn, account),
        "nothing is deleted while an appeal is open"
    );
}

/// T064: a rejected appeal resumes from the date already fixed. Filing on day
/// twenty-nine bought no second window — the very next sweep carries it out.
#[tokio::test]
async fn a_rejected_appeal_resumes_from_the_date_already_passed() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, opened) = a_window(&mut conn, 40);
    let admin = insert_test_user(&mut conn);

    file_appeal_sync(&mut conn, account, "Please reconsider.", Utc::now()).expect("appeal");
    let rejected = resolve_appeal_sync(
        &mut conn,
        account,
        admin,
        &AppealDecision {
            upheld: false,
            note: Some("The notices were valid.".to_string()),
            overturned_case: None,
        },
        TIMER_LADDER,
        Utc::now(),
    )
    .expect("reject");
    assert_eq!(
        rejected.deletion_due_at, opened.deletion_due_at,
        "a rejection does not move the date"
    );

    let report =
        sweep_scoped_sync(&mut conn, TIMER_LADDER, Utc::now(), Some(account)).expect("sweep");
    assert_eq!(report.deleted, 1);
    assert!(
        !account_exists(&mut conn, account),
        "real deletion (FR-036)"
    );

    let closed: Option<String> = account_terminations::table
        .filter(account_terminations::id.eq(opened.id))
        .select(account_terminations::closed_reason)
        .first(&mut conn)
        .expect("the row outlives the account");
    assert_eq!(closed.as_deref(), Some(closed_reason::DELETED));
}

/// T064 / FR-033: an upheld appeal restores the account, cancels the deletion,
/// and removes the strike it overturned.
#[tokio::test]
async fn an_upheld_appeal_restores_and_removes_the_strike() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, _) = a_window(&mut conn, 5);
    let admin = insert_test_user(&mut conn);
    assert!(is_disabled_sync(&mut conn, account).expect("disabled"));

    file_appeal_sync(&mut conn, account, "The third was never mine.", Utc::now()).expect("appeal");
    resolve_appeal_sync(
        &mut conn,
        account,
        admin,
        &AppealDecision {
            upheld: true,
            note: None,
            overturned_case: None,
        },
        TIMER_LADDER,
        Utc::now(),
    )
    .expect("uphold");

    let after = standing_of_sync(&mut conn, account, TIMER_LADDER).expect("standing");
    assert!(!after.disabled, "restored");
    assert!(after.termination.is_none(), "the window is closed");
    assert_eq!(
        after.strike_count(),
        2,
        "the overturned strike no longer counts"
    );
    assert!(
        notices_for(&mut conn, account)
            .iter()
            .any(|(kind, payload)| kind == notices::kind::APPEAL_RESOLVED
                && payload.as_ref().is_some_and(|p| p["upheld"] == true)),
        "and the person is told",
    );
}

/// FR-035: a strike that ages past the lookback while an account is disabled
/// is recounted, and the account is restored without anybody asking.
#[tokio::test]
async fn a_strike_ageing_out_while_disabled_restores_the_account() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, opened) = a_window(&mut conn, 5);

    let oldest: Uuid = content_moderation_actions::table
        .filter(content_moderation_actions::account_id.eq(account))
        .order(content_moderation_actions::created_at.asc())
        .select(content_moderation_actions::case_id)
        .first(&mut conn)
        .expect("a case");
    diesel::update(content_moderation_actions::table)
        .filter(content_moderation_actions::case_id.eq(oldest))
        .set(content_moderation_actions::created_at.eq(Utc::now() - chrono::Duration::days(400)))
        .execute(&mut conn)
        .expect("age it");

    let report =
        sweep_scoped_sync(&mut conn, TIMER_LADDER, Utc::now(), Some(account)).expect("sweep");
    assert_eq!(report.closed, 1);
    assert!(!is_disabled_sync(&mut conn, account).expect("disabled"));

    let reason: Option<String> = account_terminations::table
        .filter(account_terminations::id.eq(opened.id))
        .select(account_terminations::closed_reason)
        .first(&mut conn)
        .expect("row");
    assert_eq!(reason.as_deref(), Some(closed_reason::STRIKES_AGED_OUT));
    assert!(
        notices_for(&mut conn, account)
            .iter()
            .any(|(kind, _)| kind == notices::kind::ACCOUNT_RESTORED),
        "told they are back",
    );
}

/// The window opens at the moment of the third strike (FR-030), not at the
/// next sweep — and the notice says when it ends.
#[tokio::test]
async fn the_third_strike_opens_the_window_there_and_then() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, world) = an_account_in_a_world(&mut conn);
    for _ in 0..3 {
        let case = an_upheld_case(&mut conn, account, world);
        tell_of_strike_sync(&mut conn, account, case, LADDER).expect("tell");
    }

    let standing = standing_of_sync(&mut conn, account, LADDER).expect("standing");
    assert!(standing.disabled);
    let window = standing.termination.expect("open");
    assert!(
        window.requires_human,
        "the shipped default waits for a person"
    );
    let disabled_notice = notices_for(&mut conn, account)
        .into_iter()
        .find(|(kind, _)| kind == notices::kind::ACCOUNT_DISABLED)
        .expect("told at the start of the window (FR-036)");
    assert!(disabled_notice.1.expect("payload")["deletionDueAt"].is_string());
}

/// T066 / FR-039: the last administrator's window is written, needs a person,
/// and does **not** disable them. With another administrator it is an
/// ordinary window.
#[test]
fn the_last_administrator_is_never_disabled_by_the_counting() {
    assert_eq!(
        termination_terms(TIMER_LADDER, true, 0),
        (false, true),
        "the only administrator: written, a person decides, not disabled",
    );
    assert_eq!(
        termination_terms(TIMER_LADDER, true, 1),
        (true, false),
        "an administrator with a colleague is treated like anybody else",
    );
    assert_eq!(termination_terms(TIMER_LADDER, false, 0), (true, false));
    assert_eq!(
        termination_terms(LADDER, false, 3),
        (true, true),
        "the snapshot follows the setting when FR-039 does not apply",
    );
}

/// An administrator cannot carry out the last administrator's window.
#[tokio::test]
async fn no_administrator_can_delete_the_last_one() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let (account, world) = an_account_in_a_world(&mut conn);
    for _ in 0..3 {
        an_upheld_case(&mut conn, account, world);
    }
    // Written directly with the terms FR-039 gives the last administrator,
    // because making this database hold exactly one would change global state.
    diesel::insert_into(account_terminations::table)
        .values(&NewAccountTermination {
            id: Uuid::now_v7(),
            account_id: account,
            opened_at: Utc::now() - chrono::Duration::days(40),
            deletion_due_at: Utc::now() - chrono::Duration::days(10),
            strike_count_at_open: 3,
            requires_human: true,
            disables_account: false,
        })
        .execute(&mut conn)
        .expect("window");

    let refused = execute_by_administrator_sync(&mut conn, account, Utc::now());
    assert!(refused.is_err());
    assert!(account_exists(&mut conn, account));
    assert!(!is_disabled_sync(&mut conn, account).expect("disabled"));
}

/// T065 / FR-032: downloading writes nothing to the window, and appealing
/// writes nothing to the download — in both orders. Exercising one remedy
/// forfeits nothing of the other.
#[tokio::test]
async fn downloading_and_appealing_leave_each_other_alone_in_either_order() {
    let state = test_app_state();

    let exported = |value: crate::users::UserDataExport| {
        let mut json = serde_json::to_value(value).expect("json");
        json["manifest"]["exported_at"] = serde_json::Value::Null;
        json
    };
    let window_of = |conn: &mut PgConnection, account: Uuid| -> AccountTermination {
        account_terminations::table
            .filter(account_terminations::account_id.eq(account))
            .select(AccountTermination::as_select())
            .first(conn)
            .expect("window")
    };

    for appeal_first in [false, true] {
        let mut conn = state.db_pool.get().expect("conn");
        let (account, _) = a_window(&mut conn, 5);
        drop(conn);

        let download = || crate::users::export_user_data_payload(&state, account);
        let appeal = || {
            let mut conn = state.db_pool.get().expect("conn");
            file_appeal_sync(&mut conn, account, "Mine.", Utc::now()).expect("appeal");
        };

        let before_download = exported(download().await.expect("export"));
        if appeal_first {
            appeal();
        }
        let mut conn = state.db_pool.get().expect("conn");
        let window_before = window_of(&mut conn, account);
        drop(conn);

        let after_download = exported(download().await.expect("export"));
        let mut conn = state.db_pool.get().expect("conn");
        assert_eq!(
            window_of(&mut conn, account),
            window_before,
            "downloading moved nothing in the window (appeal first: {appeal_first})",
        );
        drop(conn);

        if !appeal_first {
            appeal();
        }
        assert_eq!(
            exported(download().await.expect("export")),
            after_download,
            "appealing changed nothing in the download (appeal first: {appeal_first})",
        );
        assert_eq!(before_download, after_download);
    }
}
