//! Spec 088 (US1): a GM chooses a world link's limit and expiry within
//! bounds, and a link that admits no one says why.

use super::tests::{insert_test_invite, insert_test_invite_with_revocation, load_invite};
use super::*;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};
use async_graphql::MaybeUndefined;

fn code_of(err: &Error) -> String {
    err.extensions
        .as_ref()
        .and_then(|ext| ext.get("code"))
        .map(|value| value.to_string().trim_matches('"').to_string())
        .unwrap_or_default()
}

fn at(text: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

// ===== T012: generateInviteCode's limits, expiry and defaults (FR-002) =====

#[test]
fn a_limit_outside_one_to_fifty_is_refused() {
    let now = Utc::now();
    for refused in [0, -1, 51, 1000] {
        assert_eq!(
            link_options(Some(refused), &MaybeUndefined::Undefined, now),
            Err(LINK_LIMIT_MESSAGE),
            "{refused} uses must be refused"
        );
    }
    for allowed in [1, 7, 50] {
        assert!(link_options(Some(allowed), &MaybeUndefined::Undefined, now).is_ok());
    }
}

#[test]
fn an_expiry_that_is_not_a_date_is_refused_not_ignored() {
    let now = Utc::now();
    for bad in ["tomorrow", "2026-13-40", ""] {
        assert_eq!(
            link_options(None, &MaybeUndefined::Value(bad.to_string()), now),
            Err(EXPIRY_NOT_A_DATE_MESSAGE),
            "{bad:?} must be refused"
        );
    }
}

#[test]
fn an_expiry_in_the_past_is_refused() {
    let now = at("2026-10-09T12:00:00Z");
    assert_eq!(
        link_options(
            None,
            &MaybeUndefined::Value("2026-10-09T11:59:59Z".to_string()),
            now
        ),
        Err(EXPIRY_PASSED_MESSAGE)
    );
}

#[test]
fn left_out_a_link_has_no_limit_and_lives_seven_days() {
    let now = at("2026-10-09T12:00:00Z");
    let (max_uses, expires_at) = link_options(None, &MaybeUndefined::Undefined, now).unwrap();
    assert_eq!(max_uses, None, "no limit by default");
    assert_eq!(
        expires_at,
        Some(at("2026-10-16T12:00:00Z").naive_utc()),
        "seven days by default"
    );
}

#[test]
fn a_null_expiry_is_a_link_that_never_expires() {
    let (_, expires_at) = link_options(Some(3), &MaybeUndefined::Null, Utc::now()).unwrap();
    assert_eq!(expires_at, None);
}

#[test]
fn a_chosen_expiry_is_kept_in_utc() {
    let now = at("2026-10-09T12:00:00Z");
    let (_, expires_at) = link_options(
        None,
        &MaybeUndefined::Value("2026-10-10T14:00:00+02:00".to_string()),
        now,
    )
    .unwrap();
    assert_eq!(expires_at, Some(at("2026-10-10T12:00:00Z").naive_utc()));
}

#[tokio::test]
async fn a_link_made_with_the_defaults_is_stored_without_a_limit() {
    let state = test_app_state();
    let (world_id, owner_id) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        (insert_test_world(&mut conn, owner), owner)
    };

    let payload = generate_invite_code_impl(
        &state,
        owner_id,
        GenerateInviteCodeInput {
            world_id,
            max_uses: None,
            expires_at: MaybeUndefined::Undefined,
        },
    )
    .await
    .expect("the defaults make a link");
    assert_eq!(payload.max_uses, None);
    assert_eq!(payload.remaining_uses, None);
    assert!(payload.expires_at.is_some(), "seven days, not forever");

    let row = {
        let mut conn = state.db_pool.get().unwrap();
        load_invite(&mut conn, payload.id)
    };
    assert_eq!(row.max_uses, None, "NULL is no limit");
}

#[tokio::test]
async fn a_refused_link_is_not_stored() {
    let state = test_app_state();
    let (world_id, owner_id) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        (insert_test_world(&mut conn, owner), owner)
    };

    let err = generate_invite_code_impl(
        &state,
        owner_id,
        GenerateInviteCodeInput {
            world_id,
            max_uses: Some(51),
            expires_at: MaybeUndefined::Undefined,
        },
    )
    .await
    .expect_err("51 uses is refused");
    assert_eq!(err.message, LINK_LIMIT_MESSAGE);

    let links: i64 = {
        let mut conn = state.db_pool.get().unwrap();
        world_invites::table
            .filter(world_invites::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap()
    };
    assert_eq!(links, 0);
}

// ===== T014: joinWorld says why (FR-008) =====

#[tokio::test]
async fn each_dead_link_gives_its_own_code_and_message() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    let past = Utc::now().naive_utc() - chrono::Duration::days(1);
    let (expired_id, expired) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
    let (used_up_id, used_up) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
    let (revoked_id, revoked) =
        insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, Some(past), true);
    drop(conn);

    for (code, expected_code, expected_message) in [
        (revoked, "LINK_REVOKED", LINK_REVOKED_MESSAGE),
        (expired, "LINK_EXPIRED", LINK_EXPIRED_MESSAGE),
        (used_up, "LINK_USED_UP", LINK_USED_UP_MESSAGE),
        (
            "ZZZZZZZZZZZZZZZZZZZZZZZZZZ".to_string(),
            "LINK_UNKNOWN",
            LINK_UNKNOWN_MESSAGE,
        ),
    ] {
        let joiner = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        let err = join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
            .await
            .expect_err("a dead link admits no one");
        assert_eq!(code_of(&err), expected_code);
        assert_eq!(err.message, expected_message);
    }

    // A failed join uses nothing.
    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(load_invite(&mut conn, expired_id).used_count, 0);
    assert_eq!(load_invite(&mut conn, used_up_id).used_count, 3);
    assert_eq!(load_invite(&mut conn, revoked_id).used_count, 0);
}

#[tokio::test]
async fn a_link_with_no_limit_keeps_admitting_and_keeps_counting() {
    let state = test_app_state();
    let (invite_id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        // `0` here is the helper's spelling of no limit: NULL in the row.
        insert_test_invite(&mut conn, world, owner, 0, 0, None)
    };

    for _ in 0..3 {
        let joiner = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        join_world_impl(
            &state,
            joiner,
            JoinWorldInput {
                invite_code: code.clone(),
            },
        )
        .await
        .expect("a link with no limit admits everyone who comes");
    }
    let mut conn = state.db_pool.get().unwrap();
    let row = load_invite(&mut conn, invite_id);
    assert_eq!(row.max_uses, None);
    assert_eq!(row.used_count, 3, "joins are still counted");
}

#[tokio::test]
async fn a_member_rejoining_is_told_so_and_uses_nothing() {
    let state = test_app_state();
    let (invite_id, code, owner) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let (id, code) = insert_test_invite(&mut conn, world, owner, 2, 0, None);
        (id, code, owner)
    };

    // The owner counts as a member.
    let err = join_world_impl(
        &state,
        owner,
        JoinWorldInput {
            invite_code: code.clone(),
        },
    )
    .await
    .expect_err("the owner is already in the world");
    assert_eq!(code_of(&err), "ALREADY_MEMBER");

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(load_invite(&mut conn, invite_id).used_count, 0);
}

#[tokio::test]
async fn a_code_typed_in_lower_case_with_look_alikes_still_joins() {
    let state = test_app_state();
    let (world_id, owner_id, joiner) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        (world, owner, insert_test_user(&mut conn))
    };
    let link = generate_invite_code_impl(
        &state,
        owner_id,
        GenerateInviteCodeInput {
            world_id,
            max_uses: None,
            expires_at: MaybeUndefined::Undefined,
        },
    )
    .await
    .unwrap();

    // Read aloud: lower case, with `0` heard as `o` and `1` as `l`.
    let typed = link
        .invite_code
        .to_lowercase()
        .replace('0', "o")
        .replace('1', "l");
    let joined = join_world_impl(&state, joiner, JoinWorldInput { invite_code: typed })
        .await
        .expect("the retyped code finds its link");
    assert_eq!(joined.world_id, world_id);
}

// ===== T018 (FR-001, FR-004): what the world's event stream says =====

#[tokio::test]
async fn link_events_name_no_code_and_a_revoke_is_announced() {
    use crate::schema::world_events;

    let state = test_app_state();
    let (world_id, owner_id) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        (insert_test_world(&mut conn, owner), owner)
    };
    let link = generate_invite_code_impl(
        &state,
        owner_id,
        GenerateInviteCodeInput {
            world_id,
            max_uses: None,
            expires_at: MaybeUndefined::Undefined,
        },
    )
    .await
    .unwrap();
    revoke_invite_code_impl(&state, owner_id, false, link.id)
        .await
        .unwrap();

    let payloads: Vec<Option<serde_json::Value>> = {
        let mut conn = state.db_pool.get().unwrap();
        world_events::table
            .filter(world_events::world_id.eq(world_id))
            .filter(world_events::event_code.eq(2))
            .order(world_events::id.asc())
            .select(world_events::token_event)
            .load(&mut conn)
            .unwrap()
    };
    assert_eq!(payloads.len(), 2, "made, then revoked");
    for payload in &payloads {
        let text = payload.as_ref().unwrap().to_string();
        assert!(
            !text.contains(&link.invite_code),
            "a player reads this stream; it must not carry the code: {text}"
        );
    }
    assert_eq!(payloads[1].as_ref().unwrap()["revoked"], true);
}
