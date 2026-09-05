use super::*;

fn command(kind: &str, id: Uuid, x: Option<f64>) -> serde_json::Value {
    serde_json::json!({
        "type": kind,
        "token": { "id": id, "x": x, "y": 2.0 },
    })
}

#[test]
fn a_token_move_is_understood() {
    let id = Uuid::now_v7();
    let edit = parse_token_edit(&command("upsert_token", id, Some(1.0)))
        .expect("a token move is exactly what this replays");

    assert_eq!(edit.token_id, id);
    assert_eq!(edit.x, Some(1.0));
    assert_eq!(edit.y, Some(2.0));
}

/// FR-035a. Anything that is not a token move/manipulate is refused, and
/// refusal has to be a *rejection* rather than a skip — a skipped input
/// would produce no outcome, which is the silent loss FR-041 prohibits.
#[test]
fn a_command_that_is_not_a_token_edit_is_refused() {
    let id = Uuid::now_v7();
    assert!(parse_token_edit(&command("remove_token", id, Some(1.0))).is_none());
    assert!(parse_token_edit(&serde_json::json!({"type": "create_wall"})).is_none());
    assert!(parse_token_edit(&serde_json::json!("not even an object")).is_none());
}

/// A command naming a token and changing nothing is not an edit. Applying
/// it would take the conflict mark for an item nobody actually moved,
/// which would then supersede someone's real change.
#[test]
fn an_edit_that_changes_nothing_is_refused() {
    let id = Uuid::now_v7();
    assert!(
        parse_token_edit(&serde_json::json!({
            "type": "upsert_token",
            "token": { "id": id },
        }))
        .is_none()
    );
}

/// Rotation and scale are permitted alongside position (FR-035a), so a
/// manipulate replays as readily as a move.
#[test]
fn rotation_and_scale_are_permitted() {
    let id = Uuid::now_v7();
    let edit = parse_token_edit(&serde_json::json!({
        "type": "upsert_token",
        "token": { "id": id, "rotation": 90.0, "scale": 2.0 },
    }))
    .expect("rotation and scale are offline-editable");

    assert_eq!(edit.rotation, Some(90.0));
    assert_eq!(edit.scale, Some(2.0));
    assert_eq!(edit.x, None);
}

/// Owner and GM both run the table. A player is anything else, including
/// a role string this server does not recognise — the safe reading of an
/// unknown role is the least privileged one.
#[test]
fn owner_and_gm_are_both_game_masters() {
    assert_eq!(role_from_membership("Owner"), Role::GameMaster);
    assert_eq!(role_from_membership("GM"), Role::GameMaster);
    assert_eq!(role_from_membership("Player"), Role::Player);
    assert_eq!(role_from_membership("something-new"), Role::Player);
}

fn insert_token(conn: &mut PgConnection, scene_id: Uuid, owner: Option<Uuid>) -> Uuid {
    use crate::schema::tokens;
    let token_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(token_id),
            tokens::scene_id.eq(scene_id),
            tokens::x.eq(0.0_f64),
            tokens::y.eq(0.0_f64),
            tokens::owner_user_id.eq(owner),
            tokens::created_at.eq(now),
            tokens::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test token");
    token_id
}

fn move_command(token_id: Uuid, x: f64) -> QueuedChangeInput {
    QueuedChangeInput {
        local_id: Uuid::now_v7().to_string(),
        command: async_graphql::Json(serde_json::json!({
            "type": "upsert_token",
            "token": { "id": token_id, "x": x, "y": 0.0 },
        })),
        attributed_to_user_id: None,
        reported_outcome: None,
    }
}

/// A roll the server itself resolved and recorded (ADR-044) — the only
/// thing discrepancy detection is ever allowed to compare against.
fn insert_roll_record(conn: &mut PgConnection, world_id: Uuid, by: Uuid, value: f64) -> Uuid {
    use crate::schema::world_roll_records;
    diesel::insert_into(world_roll_records::table)
        .values(&crate::models::NewRollRecord {
            world_id,
            triggered_by: by,
            formula: "1d20".to_string(),
            bindings: None,
            detail: serde_json::json!({}),
            result_kind: "total".to_string(),
            result_value: value,
        })
        .returning(world_roll_records::id)
        .get_result::<Uuid>(conn)
        .expect("failed to insert test roll record")
}

fn dice_report(record_id: Uuid, value: f64) -> ReportedOutcomeInput {
    ReportedOutcomeInput {
        kind: REPORTED_OUTCOME_KIND_DICE.to_string(),
        version: REPORTED_OUTCOME_VERSION,
        record_id: Some(record_id),
        value: Some(value),
    }
}

fn token_x(conn: &mut PgConnection, token_id: Uuid) -> f64 {
    use crate::schema::tokens;
    tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select(tokens::x)
        .first::<f64>(conn)
        .expect("token should exist")
}

/// T078/FR-041: every submitted change gets exactly one outcome, whatever
/// happened to it. Silent loss is the prohibited failure, and the way it
/// would appear is an input with no matching outcome.
#[test]
fn every_change_gets_exactly_one_outcome_including_the_bad_ones() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, owner);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
    let token = insert_token(&mut conn, scene, Some(owner));
    let gone = Uuid::now_v7();

    let changes = vec![
        move_command(token, 5.0),
        move_command(gone, 7.0),
        QueuedChangeInput {
            local_id: Uuid::now_v7().to_string(),
            command: async_graphql::Json(serde_json::json!({"type": "delete_token"})),
            attributed_to_user_id: None,
            reported_outcome: None,
        },
    ];
    let submitted: Vec<String> = changes.iter().map(|c| c.local_id.clone()).collect();

    let outcomes: Vec<GraphQLReconcileOutcome> = changes
        .into_iter()
        .map(|change| {
            apply_one(
                &mut conn,
                world,
                owner,
                Role::GameMaster,
                take_reconnect_seq(world),
                change,
            )
        })
        .collect();

    assert_eq!(outcomes.len(), submitted.len());
    let answered: Vec<String> = outcomes.iter().map(|o| o.local_id.clone()).collect();
    assert_eq!(
        answered, submitted,
        "outcomes must match inputs one for one"
    );

    assert!(outcomes[0].applied, "a valid move by the owner applies");
    assert_eq!(
        outcomes[1].reason,
        Some(GraphQLRejectionReason::GoneAway),
        "a token deleted server-side is discarded with a reason, not resurrected"
    );
    assert_eq!(
        outcomes[2].reason,
        Some(GraphQLRejectionReason::Invalid),
        "a command outside FR-035a is rejected, never skipped"
    );
}

/// FR-042: a player may replay a move of their own token, and not of
/// someone else's — asked at reconnect time, not at edit time.
#[test]
fn a_player_may_only_replay_moves_of_their_own_token() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, owner);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
    let theirs = insert_token(&mut conn, scene, Some(player));
    let someone_elses = insert_token(&mut conn, scene, Some(owner));

    let mine = apply_one(
        &mut conn,
        world,
        player,
        Role::Player,
        take_reconnect_seq(world),
        move_command(theirs, 3.0),
    );
    let not_mine = apply_one(
        &mut conn,
        world,
        player,
        Role::Player,
        take_reconnect_seq(world),
        move_command(someone_elses, 3.0),
    );

    assert!(mine.applied);
    assert_eq!(
        not_mine.reason,
        Some(GraphQLRejectionReason::PermissionDenied)
    );
    assert_eq!(
        token_x(&mut conn, someone_elses),
        0.0,
        "a refused replay must not have written anything"
    );
}

/// FR-040, the sharp edge: a player reconnects first and their change
/// applies; the GM reconnects later with a conflicting edit and takes
/// precedence. The GM's change is what stands.
#[test]
fn a_game_master_reconnecting_later_still_wins() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(player));

    let player_first = apply_one(
        &mut conn,
        world,
        player,
        Role::Player,
        take_reconnect_seq(world),
        move_command(token, 10.0),
    );
    assert!(
        player_first.applied,
        "the player got back first and applied"
    );
    assert_eq!(token_x(&mut conn, token), 10.0);

    let gm_later = apply_one(
        &mut conn,
        world,
        gm,
        Role::GameMaster,
        take_reconnect_seq(world),
        move_command(token, 20.0),
    );

    assert!(gm_later.applied, "a GM beats a player regardless of order");
    assert_eq!(token_x(&mut conn, token), 20.0);
}

/// And the other direction, which is the case the UI has to explain: the
/// GM reconnected first, so the player's queued edit is superseded rather
/// than failing. `supersededByRole` is what lets the client say who won
/// instead of reporting an anonymous error.
#[test]
fn a_player_losing_to_a_game_master_is_told_who_won() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(player));

    apply_one(
        &mut conn,
        world,
        gm,
        Role::GameMaster,
        take_reconnect_seq(world),
        move_command(token, 42.0),
    );

    let player_late = apply_one(
        &mut conn,
        world,
        player,
        Role::Player,
        take_reconnect_seq(world),
        move_command(token, 99.0),
    );

    assert!(!player_late.applied);
    assert_eq!(
        player_late.reason,
        Some(GraphQLRejectionReason::Superseded),
        "losing to a GM is a working rule, not an error"
    );
    assert_eq!(
        player_late.superseded_by_role.as_deref(),
        Some("GameMaster"),
        "the UI has to be able to say who won"
    );
    assert_eq!(token_x(&mut conn, token), 42.0, "the GM's value stands");
}

/// The sequence is per world and monotonic. Two worlds reconnecting do
/// not share an ordering, and one world's sequence never repeats — a
/// repeat would make two different reconnections tie, and
/// `conflict::resolve` breaks a tie by favouring the *held* side, so the
/// later client would lose without a rule ever having said so.
#[test]
fn reconnect_sequences_are_per_world_and_increasing() {
    let world_a = Uuid::now_v7();
    let world_b = Uuid::now_v7();

    let first = take_reconnect_seq(world_a);
    let second = take_reconnect_seq(world_a);
    let other = take_reconnect_seq(world_b);

    assert!(second > first, "a world's sequence must advance");
    assert_eq!(other, 0, "a different world starts its own ordering");
}

/// FR-061, the whole of the positive case: the GM's client relays a change
/// a player originated while the server was unreachable, and the only
/// question asked is whether the submitter runs this table. If this ever
/// starts demanding an attestation, peer-adjudicated play has stopped
/// working for the party the design trusts.
#[test]
fn a_game_master_may_submit_a_change_a_player_originated() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(player));

    let mut change = move_command(token, 12.0);
    change.attributed_to_user_id = Some(player);

    let outcome = apply_one(
        &mut conn,
        world,
        gm,
        Role::GameMaster,
        take_reconnect_seq(world),
        change,
    );

    assert!(outcome.applied, "the GM is the trusted party by design");
    assert_eq!(token_x(&mut conn, token), 12.0);
}

/// SC-021/FR-061a, and the reason the previous test is not simply a hole:
/// the *identical* submission from someone who does not run the table is
/// refused. This is the one thing attribution is checked for.
#[test]
fn a_player_may_not_submit_a_change_attributed_to_someone_else() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let other = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    // Owned by the submitter, so ownership cannot be what refuses this —
    // only the attribution can.
    let token = insert_token(&mut conn, scene, Some(player));

    let mut change = move_command(token, 12.0);
    change.attributed_to_user_id = Some(other);

    let outcome = apply_one(
        &mut conn,
        world,
        player,
        Role::Player,
        take_reconnect_seq(world),
        change,
    );

    assert!(!outcome.applied);
    assert_eq!(
        outcome.reason,
        Some(GraphQLRejectionReason::PermissionDenied)
    );
    assert_eq!(
        token_x(&mut conn, token),
        0.0,
        "a refused attribution must not have written anything"
    );
}

/// FR-064/FR-065/FR-066 together, and the ordering matters: the change is
/// **applied**, and the difference is disclosed alongside it with both
/// numbers. A test that only checked the flag would pass on an
/// implementation that rejected the change, which is the outcome the ADR
/// specifically rules out.
#[test]
fn a_reported_value_the_server_determined_differently_is_applied_and_disclosed() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(player));
    let record = insert_roll_record(&mut conn, world, player, 7.0);

    let mut change = move_command(token, 3.0);
    change.attributed_to_user_id = Some(player);
    change.reported_outcome = Some(dice_report(record, 20.0));

    let outcome = apply_one(
        &mut conn,
        world,
        gm,
        Role::GameMaster,
        take_reconnect_seq(world),
        change,
    );

    assert!(
        outcome.applied,
        "a discrepancy never rejects, interrupts, or alters the outcome"
    );
    assert_eq!(token_x(&mut conn, token), 3.0);

    let found = outcome
        .discrepancy
        .expect("a genuine determined-value mismatch is what disclosure exists for");
    assert_eq!(
        found.user_id, player,
        "whose result it was, not who relayed it"
    );
    assert_eq!(found.record_id, record);
    assert_eq!(found.reported_value, 20.0);
    assert_eq!(
        found.determined_value, 7.0,
        "both numbers, so the GM can look"
    );
}

/// FR-061b. A GM acting on a player's behalf is table authority being
/// exercised, not an attack: it produces no flag and notifies nobody. The
/// failure this catches is treating attribution *itself* as suspicious,
/// which would put every relayed change under a cloud.
#[test]
fn a_game_master_acting_on_a_players_behalf_produces_no_flag() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(player));
    let record = insert_roll_record(&mut conn, world, player, 15.0);

    let mut change = move_command(token, 4.0);
    change.attributed_to_user_id = Some(player);
    change.reported_outcome = Some(dice_report(record, 15.0));

    let outcome = apply_one(
        &mut conn,
        world,
        gm,
        Role::GameMaster,
        take_reconnect_seq(world),
        change,
    );

    assert!(outcome.applied);
    assert!(
        outcome.discrepancy.is_none(),
        "agreeing values are the silent case, exactly like no determination"
    );
}

/// FR-068. Ordinary token movement — the overwhelming majority of what
/// this mutation carries — has no independent basis, so there is nothing
/// to detect and nothing to say. Absence of evidence is not a flag.
#[test]
fn an_ordinary_token_move_reports_no_discrepancy() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(gm));

    let outcome = apply_one(
        &mut conn,
        world,
        gm,
        Role::GameMaster,
        take_reconnect_seq(world),
        move_command(token, 6.0),
    );

    assert!(outcome.applied);
    assert!(outcome.discrepancy.is_none());
}

/// FR-067. Disclosure is the GM's and nobody else's. A player replaying
/// their own queued work is told what became of it and is never handed a
/// flag about themselves — and, since a world event reaches every member,
/// the discrepancy deliberately never travels that way either.
#[test]
fn a_player_submitter_is_shown_no_discrepancy() {
    let state = crate::test_support::test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = crate::test_support::insert_test_user(&mut conn);
    let player = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, gm);
    let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
    let token = insert_token(&mut conn, scene, Some(player));
    let record = insert_roll_record(&mut conn, world, player, 2.0);

    let mut change = move_command(token, 8.0);
    change.reported_outcome = Some(dice_report(record, 19.0));

    let outcome = apply_one(
        &mut conn,
        world,
        player,
        Role::Player,
        take_reconnect_seq(world),
        change,
    );

    assert!(outcome.applied);
    assert!(outcome.discrepancy.is_none());
}

/// FR-067a, the expensive failure: a database error — a statement timeout
/// above all — means the server did not determine anything. Reading that
/// as "the server says otherwise" would accuse someone because a query
/// was slow.
#[test]
fn a_lookup_that_failed_determined_nothing() {
    assert!(
        determination_from_lookup(Err(diesel::result::Error::BrokenTransactionManager)).is_none()
    );
}

/// The same for a row that is not there — a roll the server has no record
/// of, or one belonging to another world. Nothing to compare is nothing to
/// report.
#[test]
fn a_missing_record_determined_nothing() {
    assert!(determination_from_lookup(Ok(None)).is_none());
    assert!(determination_from_lookup(Ok(Some(f64::NAN))).is_none());
}

/// A reported outcome the server cannot read is a parse failure, and a
/// parse failure is an ambiguity. None of these may ever become a claimed
/// value the server then "disagrees" with.
#[test]
fn an_unreadable_report_is_never_a_mismatch() {
    let record = Uuid::now_v7();

    let mut unknown_kind = dice_report(record, 20.0);
    unknown_kind.kind = "vibes".to_string();
    assert!(parse_reported_outcome(&unknown_kind).is_none());

    let mut no_record = dice_report(record, 20.0);
    no_record.record_id = None;
    assert!(parse_reported_outcome(&no_record).is_none());

    let mut no_value = dice_report(record, 20.0);
    no_value.value = None;
    assert!(parse_reported_outcome(&no_value).is_none());

    assert!(
        parse_reported_outcome(&dice_report(record, f64::NAN)).is_none(),
        "a value that is not a number cannot be compared to one"
    );
}

/// A client one release ahead of this server is reporting in a format we
/// cannot read. That is a version mismatch, which is an ambiguity — the
/// worst possible response is to compare the fields we recognise and
/// accuse whoever upgraded first.
#[test]
fn a_report_in_a_format_this_server_does_not_know_is_never_a_mismatch() {
    let mut ahead = dice_report(Uuid::now_v7(), 20.0);
    ahead.version = REPORTED_OUTCOME_VERSION + 1;
    assert!(parse_reported_outcome(&ahead).is_none());
}

/// The structural claim the module doc makes, asserted directly: with no
/// determination in hand there is no discrepancy, whatever was claimed —
/// the same answer agreement gives. If these two ever diverge, every
/// ambiguity above becomes an accusation.
#[test]
fn no_determination_and_agreement_are_the_same_silence() {
    let reported = ReportedOutcome {
        record_id: Uuid::now_v7(),
        claimed: 20.0,
    };
    let user = Uuid::now_v7();

    assert!(discrepancy_between(user, &reported, None).is_none());
    assert!(discrepancy_between(user, &reported, Some(DeterminedValue(20.0))).is_none());
    assert!(
        discrepancy_between(user, &reported, Some(DeterminedValue(20.0 + 1e-12))).is_none(),
        "a float round trip is not a discrepancy"
    );
    assert!(
        discrepancy_between(user, &reported, Some(DeterminedValue(7.0))).is_some(),
        "and a genuine mismatch still gets reported, or none of this is worth having"
    );
}
