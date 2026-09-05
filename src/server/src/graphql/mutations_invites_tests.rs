use diesel::{Connection, PgConnection};

/// Establishes a connection to the dev database configured via
/// DATABASE_URL (same source main.rs uses). Skips (rather than fails)
/// when no dev database is reachable, since this is a real-DB
/// integration test, not a unit test.
fn try_connect() -> Option<PgConnection> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").ok()?;
    diesel::Connection::establish(&url).ok()
}

/// Spec 005 US4 regression test (T020): before this fix,
/// `generate_invite_code`'s own inline `world_members` lookup had no
/// fallback, so a world's own owner — who has no `world_members` row
/// today (`create_world` doesn't insert one; see
/// `auth::world_membership::require_world_member`'s doc comment) —
/// could never generate an invite for their own world. The fix routes
/// `generate_invite_code` (and `world_invites`, the query
/// `CampaignSettingsPanel.tsx` calls on mount) through
/// `require_world_member` instead, which already falls back to
/// `worlds.created_by`. This test exercises that shared primitive
/// directly against a freshly created world with no `world_members`
/// row, which is exactly the state `generate_invite_code` now sees.
#[test]
fn owner_can_be_authorized_for_invites_immediately_after_world_creation() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping owner_can_be_authorized_for_invites_immediately_after_world_creation: no DATABASE_URL/dev DB reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let owner_id = crate::test_support::insert_test_user(conn);
        let world_id = crate::test_support::insert_test_world(conn, owner_id);

        // No insert_test_world_member call here — deliberately, since
        // this is exactly the state `create_world` leaves a fresh
        // world in today.
        let role = crate::auth::world_membership::require_world_member(conn, owner_id, world_id)
            .expect("owner must be authorized immediately, with no separate membership step");
        assert_eq!(role, "Owner");

        // A non-owner, non-member user must still be rejected — this
        // fix must not have loosened the check for anyone else.
        let intruder_id = crate::test_support::insert_test_user(conn);
        let intruder_result =
            crate::auth::world_membership::require_world_member(conn, intruder_id, world_id);
        assert!(
            intruder_result.is_err(),
            "a non-member/non-owner must still be rejected"
        );

        Ok(())
    });
}

// ===== Resolver-level tests for generate_invite_code_impl / join_world_impl =====
//
// These call the `_impl` free functions directly against
// `test_support::test_app_state()` (a real DB pool, no transaction
// wrapper — matching `mutations_actor_claims.rs`'s established
// convention), rather than the `require_world_member`/core-model unit
// tests above, which exercise the shared primitives in isolation but
// never actually call these two mutations end-to-end.

use super::*;
use crate::models::WorldMember;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

/// Inserts an invite row. The **8-character** code is deliberate: it is
/// exactly the shape codes had before spec 027, so every test built on
/// this helper doubles as coverage that pre-existing links still work
/// (FR-007 / SC-006).
fn insert_test_invite(
    conn: &mut PgConnection,
    world_id: Uuid,
    created_by: Uuid,
    max_uses: i32,
    used_count: i32,
    expires_at: Option<chrono::NaiveDateTime>,
) -> (Uuid, String) {
    insert_test_invite_with_revocation(
        conn, world_id, created_by, max_uses, used_count, expires_at, false,
    )
}

/// As above, but lets a test build an already-retired link.
fn insert_test_invite_with_revocation(
    conn: &mut PgConnection,
    world_id: Uuid,
    created_by: Uuid,
    max_uses: i32,
    used_count: i32,
    expires_at: Option<chrono::NaiveDateTime>,
    revoked: bool,
) -> (Uuid, String) {
    let id = Uuid::now_v7();
    let code = Uuid::new_v4()
        .to_string()
        .replace('-', "")
        .chars()
        .take(8)
        .collect::<String>()
        .to_uppercase();
    let now = Utc::now().naive_utc();
    diesel::insert_into(world_invites::table)
        .values(NewWorldInvite {
            id,
            world_id,
            invite_code: code.clone(),
            max_uses,
            used_count,
            expires_at,
            created_by,
            created_at: now,
            updated_at: now,
            revoked,
            rotated_from: None,
        })
        .execute(conn)
        .expect("failed to insert test invite");
    (id, code)
}

#[tokio::test]
async fn join_world_rejects_invalid_code() {
    let state = test_app_state();
    let joiner_id = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };

    let result = join_world_impl(
        &state,
        joiner_id,
        JoinWorldInput {
            invite_code: "NONEXIST".to_string(),
        },
    )
    .await;
    assert!(result.is_err(), "an unknown invite code must be rejected");
}

#[tokio::test]
async fn join_world_rejects_expired_invite() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let past = Utc::now().naive_utc() - chrono::Duration::days(1);
    let (_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, Some(past));
    let joiner_id = insert_test_user(&mut conn);
    drop(conn);

    let result = join_world_impl(&state, joiner_id, JoinWorldInput { invite_code: code }).await;
    assert!(result.is_err(), "an expired invite must be rejected");
}

#[tokio::test]
async fn join_world_rejects_exhausted_invite() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    // max_uses == used_count: no uses remaining.
    let (_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
    let joiner_id = insert_test_user(&mut conn);
    drop(conn);

    let result = join_world_impl(&state, joiner_id, JoinWorldInput { invite_code: code }).await;
    assert!(result.is_err(), "an exhausted invite must be rejected");
}

#[tokio::test]
async fn join_world_rejects_existing_member() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let (_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
    let existing_member_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, existing_member_id, "Player");
    drop(conn);

    let result = join_world_impl(
        &state,
        existing_member_id,
        JoinWorldInput { invite_code: code },
    )
    .await;
    assert!(
        result.is_err(),
        "a user who is already a member must not be able to join again"
    );
}

#[tokio::test]
async fn join_world_success_creates_player_membership_and_increments_usage() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, None);
    let joiner_id = insert_test_user(&mut conn);
    drop(conn);

    let payload = join_world_impl(&state, joiner_id, JoinWorldInput { invite_code: code })
        .await
        .expect("a valid, unused invite must allow joining");
    assert_eq!(payload.world_id, world_id);
    assert_eq!(payload.user_id, joiner_id);
    assert_eq!(payload.role, "Player");

    let mut conn = state.db_pool.get().unwrap();
    let member: WorldMember = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(joiner_id))
        .select(WorldMember::as_select())
        .first(&mut conn)
        .expect("membership row must have been created");
    assert_eq!(member.role, "Player");

    let updated_invite: WorldInvite = world_invites::table
        .find(invite_id)
        .select(WorldInvite::as_select())
        .first(&mut conn)
        .expect("invite row must still exist");
    assert_eq!(
        updated_invite.used_count, 1,
        "used_count must be incremented on a successful join"
    );
}

#[tokio::test]
async fn generate_invite_code_rejects_non_member() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    let result = generate_invite_code_impl(
        &state,
        outsider_id,
        GenerateInviteCodeInput {
            world_id,
            max_uses: 5,
            expires_at: None,
        },
    )
    .await;
    assert!(
        result.is_err(),
        "a non-member/non-owner must not be able to generate an invite"
    );
}

#[tokio::test]
async fn generate_invite_code_success_path() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let payload = generate_invite_code_impl(
        &state,
        owner_id,
        GenerateInviteCodeInput {
            world_id,
            max_uses: 7,
            expires_at: None,
        },
    )
    .await
    .expect("the world's own owner must be able to generate an invite");
    assert_eq!(payload.world_id, world_id);
    assert_eq!(payload.max_uses, 7);
    assert_eq!(payload.used_count, 0);

    // Spec 027 (FR-006): this assertion previously expected 8 characters.
    // Raising it to 20 is a **deliberate behaviour change**, not a test
    // relaxed to fit an accident: an invite code grants membership in a
    // world, and ~32 bits did not meet ADR-049's unguessable-code
    // invariant while content share links already used ~80.
    assert_eq!(
        payload.invite_code.len(),
        20,
        "invite codes must match content-share-link strength"
    );

    // A freshly issued link is usable, with its whole cap intact.
    assert_eq!(payload.state, WorldAccessLinkState::Active);
    assert_eq!(payload.remaining_uses, Some(7));
    assert_eq!(payload.rotated_from, None);
}

// ===== Spec 027 US1: revoke and rotate =====

fn load_invite(conn: &mut PgConnection, id: Uuid) -> WorldInvite {
    world_invites::table
        .find(id)
        .select(WorldInvite::as_select())
        .first(conn)
        .expect("invite row must exist")
}

/// FR-003 / SC-001: the retired code fails on its very next use, and the
/// replacement works. This is the whole point of the feature.
#[tokio::test]
async fn rotating_kills_the_old_code_immediately_and_issues_a_working_one() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let (invite_id, old_code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
    drop(conn);

    // Control: the code works before rotation.
    let first_joiner = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };
    join_world_impl(
        &state,
        first_joiner,
        JoinWorldInput {
            invite_code: old_code.clone(),
        },
    )
    .await
    .expect("the code must work before rotation — otherwise this proves nothing");

    let replacement = rotate_invite_code_impl(&state, owner_id, false, invite_id)
        .await
        .expect("a DM must be able to rotate their world's link");

    assert_ne!(
        replacement.invite_code, old_code,
        "a new code must be issued"
    );
    assert_eq!(replacement.state, WorldAccessLinkState::Active);

    // The retired code fails on its next use, with no grace window.
    let second_joiner = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };
    let refused = join_world_impl(
        &state,
        second_joiner,
        JoinWorldInput {
            invite_code: old_code,
        },
    )
    .await;
    assert!(
        refused.is_err(),
        "the retired code must fail on its very next use (SC-001)"
    );

    // The replacement works.
    let third_joiner = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };
    join_world_impl(
        &state,
        third_joiner,
        JoinWorldInput {
            invite_code: replacement.invite_code,
        },
    )
    .await
    .expect("the replacement code must work");
}

/// FR-014: the replacement is a clean instance of the same link — same
/// cap, same expiry, count back at zero.
#[tokio::test]
async fn rotation_inherits_cap_and_expiry_but_resets_the_count() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let expiry = Utc::now().naive_utc() + chrono::Duration::days(3);
    let (invite_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 10, 3, Some(expiry));
    drop(conn);

    let replacement = rotate_invite_code_impl(&state, owner_id, false, invite_id)
        .await
        .expect("rotation should succeed");

    assert_eq!(replacement.max_uses, 10, "cap must be inherited");
    assert_eq!(replacement.used_count, 0, "count must reset (FR-014)");
    assert_eq!(replacement.remaining_uses, Some(10));

    // The GM chose a ~3-day lifetime; the replacement carries that same
    // lifetime measured from now (see `rotated_expiry`). The source was
    // created moments ago in this test, so the new expiry lands within a
    // few seconds of the original — asserted as a window rather than an
    // equality, since Postgres stores microseconds while chrono carries
    // nanoseconds.
    let new_expiry = chrono::NaiveDateTime::parse_from_str(
        replacement
            .expires_at
            .as_ref()
            .expect("an expiring link must rotate into an expiring link"),
        "%Y-%m-%d %H:%M:%S%.f",
    )
    .expect("expiry must round-trip as a timestamp");
    let drift = (new_expiry - expiry).num_seconds().abs();
    assert!(
        drift <= 5,
        "the chosen lifetime must be preserved; drifted {drift}s"
    );
    assert!(
        new_expiry > Utc::now().naive_utc(),
        "a rotated link must not be born expired"
    );

    assert_eq!(
        replacement.rotated_from,
        Some(invite_id),
        "the replacement must record what it replaced"
    );

    let mut conn = state.db_pool.get().unwrap();
    assert!(
        load_invite(&mut conn, invite_id).revoked,
        "the source link must be retired by the same action"
    );
}

/// FR-005: rotation governs future joins only. Anyone already admitted
/// stays — it is not a retroactive removal.
#[tokio::test]
async fn rotation_leaves_existing_members_untouched() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
    let joiner = insert_test_user(&mut conn);
    drop(conn);

    join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
        .await
        .expect("join should succeed");

    rotate_invite_code_impl(&state, owner_id, false, invite_id)
        .await
        .expect("rotation should succeed");

    let mut conn = state.db_pool.get().unwrap();
    let still_a_member = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(joiner))
        .select(WorldMember::as_select())
        .first::<WorldMember>(&mut conn)
        .optional()
        .unwrap();
    assert!(
        still_a_member.is_some(),
        "rotation must never retroactively remove someone who already joined"
    );
}

/// US1-4: a dead link can always be revived by rotation. But rotating an
/// already-revoked link is refused — it would yield two replacements for
/// one original.
#[tokio::test]
async fn expired_and_exhausted_links_rotate_but_revoked_ones_do_not() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    // A realistically expired link: created two days ago with a one-day
    // lifetime, so it lapsed a day ago. `insert_test_invite` stamps
    // `created_at` as now, which would describe a link that expired before
    // it existed — impossible through the API, and it would exercise
    // `rotated_expiry`'s defensive branch instead of the real path.
    let past = Utc::now().naive_utc() - chrono::Duration::days(1);
    let (expired_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
    diesel::update(world_invites::table.find(expired_id))
        .set(world_invites::created_at.eq(Utc::now().naive_utc() - chrono::Duration::days(2)))
        .execute(&mut conn)
        .expect("backdate the expired link's creation");
    let (exhausted_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
    let (revoked_id, _) =
        insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
    drop(conn);

    let from_expired = rotate_invite_code_impl(&state, owner_id, false, expired_id)
        .await
        .expect("rotating an expired link must yield a usable one");
    assert_eq!(from_expired.state, WorldAccessLinkState::Active);

    let from_exhausted = rotate_invite_code_impl(&state, owner_id, false, exhausted_id)
        .await
        .expect("rotating an exhausted link must yield a usable one");
    assert_eq!(from_exhausted.state, WorldAccessLinkState::Active);
    assert_eq!(from_exhausted.used_count, 0);

    assert!(
        rotate_invite_code_impl(&state, owner_id, false, revoked_id)
            .await
            .is_err(),
        "an already-revoked link must not rotate again"
    );
}

/// FR-002 / FR-008: revoke is idempotent, and neither operation is open to
/// a non-DM.
#[tokio::test]
async fn revoke_is_idempotent_and_both_operations_are_dm_only() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let (invite_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    // A plain member and a non-member are both refused, for both verbs.
    for actor in [player_id, outsider_id] {
        assert!(
            revoke_invite_code_impl(&state, actor, false, invite_id)
                .await
                .is_err(),
            "only a DM may revoke"
        );
        assert!(
            rotate_invite_code_impl(&state, actor, false, invite_id)
                .await
                .is_err(),
            "only a DM may rotate"
        );
    }

    let first = revoke_invite_code_impl(&state, owner_id, false, invite_id)
        .await
        .expect("the DM must be able to revoke");
    assert_eq!(first.state, WorldAccessLinkState::Revoked);

    let second = revoke_invite_code_impl(&state, owner_id, false, invite_id)
        .await
        .expect("revoking twice must succeed rather than error");
    assert_eq!(second.state, WorldAccessLinkState::Revoked);
}

/// FR-012 — **fails before spec 027's atomic consume**.
///
/// The previous implementation read the invite, validated it in memory,
/// then wrote back a computed count. Two joins racing for the last use
/// both read `used_count = N`, both computed `N + 1`, and both wrote it —
/// admitting two members against one remaining use.
#[tokio::test]
async fn concurrent_joins_on_the_last_use_admit_exactly_one() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    // Cap 5 with 4 spent: exactly one use remains.
    let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 5, 4, None);

    // Enough contenders that a lost update is near-certain if the race is
    // still present, rather than relying on catching a two-way tie.
    let racers: Vec<Uuid> = (0..8).map(|_| insert_test_user(&mut conn)).collect();
    drop(conn);

    let attempts = racers.into_iter().map(|user_id| {
        let state = state.clone();
        let code = code.clone();
        tokio::spawn(async move {
            join_world_impl(&state, user_id, JoinWorldInput { invite_code: code })
                .await
                .is_ok()
        })
    });

    let mut succeeded = 0;
    for attempt in attempts {
        if attempt.await.expect("join task must not panic") {
            succeeded += 1;
        }
    }

    assert_eq!(
        succeeded, 1,
        "exactly one racer may claim the last use (FR-012)"
    );

    let mut conn = state.db_pool.get().unwrap();
    let invite = load_invite(&mut conn, invite_id);
    assert_eq!(
        invite.used_count, 5,
        "used_count must land exactly on the cap, never past it"
    );

    let members: i64 = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(members, 1, "only one membership may be created");
}

// ===== Spec 027 US4: unusable links fail identically =====

/// FR-011 / SC-005: unknown, expired, exhausted and revoked are
/// indistinguishable. Possessing a dead code must reveal nothing about
/// whether it was ever real, or which world it belonged to.
#[tokio::test]
async fn every_unusable_code_fails_with_the_same_message() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    let past = Utc::now().naive_utc() - chrono::Duration::days(1);
    let (_, expired) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
    let (_, exhausted) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
    let (_, revoked) =
        insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
    let never_issued = "ZZZZZZZZZZZZZZZZZZZZ".to_string();
    drop(conn);

    let mut messages = Vec::new();
    for (label, code) in [
        ("expired", expired),
        ("exhausted", exhausted),
        ("revoked", revoked),
        ("never issued", never_issued),
    ] {
        let joiner = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        let err = join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
            .await
            .expect_err(&format!("a {label} code must be refused"));
        messages.push((label, err.message));
    }

    for (label, message) in &messages {
        assert_eq!(
            message, LINK_UNAVAILABLE_MESSAGE,
            "a {label} code must return the uniform message, not its own"
        );
    }

    // Belt and braces: prove they are all literally equal to each other,
    // so a future change that gives one case its own wording fails here.
    let first = &messages[0].1;
    assert!(
        messages.iter().all(|(_, m)| m == first),
        "all unusable-code failures must be indistinguishable: {messages:?}"
    );
}

/// US4-2: an existing member gets their own message — and critically, this
/// consumes **no use**, so a repeat click never burns the GM's cap.
#[tokio::test]
async fn an_existing_member_gets_a_distinct_message_and_consumes_no_use() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
    let joiner = insert_test_user(&mut conn);
    drop(conn);

    join_world_impl(
        &state,
        joiner,
        JoinWorldInput {
            invite_code: code.clone(),
        },
    )
    .await
    .expect("first join should succeed");

    let count_after_first = {
        let mut conn = state.db_pool.get().unwrap();
        load_invite(&mut conn, invite_id).used_count
    };
    assert_eq!(count_after_first, 1);

    let err = join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
        .await
        .expect_err("a second join by the same user must be refused");
    assert_eq!(
        err.message, ALREADY_A_MEMBER_MESSAGE,
        "an existing member deserves a message that says what happened"
    );
    assert_ne!(
        err.message, LINK_UNAVAILABLE_MESSAGE,
        "the link is fine — do not report it as dead"
    );

    let count_after_second = {
        let mut conn = state.db_pool.get().unwrap();
        load_invite(&mut conn, invite_id).used_count
    };
    assert_eq!(
        count_after_second, 1,
        "a repeat click must not burn a use of the GM's cap"
    );
}

// ===== Spec 027 (T012, FR-010): link-state derivation =====
//
// Pure functions over a row's fields, so these need no database.

fn in_the_past() -> Option<chrono::NaiveDateTime> {
    Some(Utc::now().naive_utc() - chrono::Duration::hours(1))
}

fn in_the_future() -> Option<chrono::NaiveDateTime> {
    Some(Utc::now().naive_utc() + chrono::Duration::hours(1))
}

#[test]
fn a_fresh_capped_link_is_active() {
    assert_eq!(
        derive_link_state(false, None, 10, 0),
        WorldAccessLinkState::Active
    );
    assert_eq!(
        derive_link_state(false, in_the_future(), 10, 3),
        WorldAccessLinkState::Active
    );
}

#[test]
fn a_past_expiry_reads_expired() {
    assert_eq!(
        derive_link_state(false, in_the_past(), 10, 0),
        WorldAccessLinkState::Expired
    );
}

#[test]
fn a_spent_cap_reads_exhausted() {
    assert_eq!(
        derive_link_state(false, None, 5, 5),
        WorldAccessLinkState::Exhausted
    );
    // Over-consumption still reads exhausted rather than active.
    assert_eq!(
        derive_link_state(false, None, 5, 7),
        WorldAccessLinkState::Exhausted
    );
}

#[test]
fn revocation_reads_revoked() {
    assert_eq!(
        derive_link_state(true, None, 10, 0),
        WorldAccessLinkState::Revoked
    );
}

/// The precedence case from data-model.md §2: a link can be revoked *and*
/// expired *and* exhausted at once. The GM should see the most decisive
/// reason, which is revocation — it is the one a human deliberately did.
#[test]
fn revoked_outranks_expired_and_exhausted() {
    assert_eq!(
        derive_link_state(true, in_the_past(), 5, 5),
        WorldAccessLinkState::Revoked,
        "revocation must outrank every other reason"
    );
    assert_eq!(
        derive_link_state(false, in_the_past(), 5, 5),
        WorldAccessLinkState::Expired,
        "expiry must outrank exhaustion"
    );
}

/// `max_uses == 0` means unlimited, so it can never be exhausted and has
/// no remaining count to report. Unreachable via the API today, but the
/// model still branches on it — see `WorldInvite::is_valid`.
#[test]
fn an_uncapped_link_never_exhausts_and_reports_no_remainder() {
    assert_eq!(
        derive_link_state(false, None, 0, 9_999),
        WorldAccessLinkState::Active
    );
    assert_eq!(remaining_uses(0, 9_999), None);
}

#[test]
fn remaining_uses_counts_down_and_saturates_at_zero() {
    assert_eq!(remaining_uses(10, 0), Some(10));
    assert_eq!(remaining_uses(10, 4), Some(6));
    assert_eq!(remaining_uses(10, 10), Some(0));
    // Never negative, even if a row somehow over-consumed.
    assert_eq!(remaining_uses(10, 12), Some(0));
}
