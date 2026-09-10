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
