//! Spec 048 T058: a refused use is recorded and posted once per window, as
//! facts; a client that may only be stale is never reported (FR-038b); a
//! piece never delivered is always reported.

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use super::{LastEvent, Outcome, WINDOW_MINUTES, record_attempt, report_sentence};
use crate::graphql::mutations_staged_content::tests::{World, world};
use crate::schema::{world_chat_messages, world_events, world_unadopted_use_attempts as attempts};
use crate::staged_content::decide::decline;

fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

/// A character holding a staged spell the player brought.
fn held(w: &World) -> (Uuid, Uuid) {
    let actor = w.actor();
    let spell = w.stage(w.player, "spell", "Ember Lance", json!({ "level": 1 }));
    w.link_ability(actor, spell, "Ember Lance");
    (actor, spell)
}

fn attempt(w: &World, actor: Uuid, spell: Uuid, seen: LastEvent, at: NaiveDateTime) -> Outcome {
    record_attempt(
        &mut w.conn(),
        w.player,
        spell,
        Some(actor),
        "makeAttack",
        seen,
        at,
    )
    .expect("recorded")
    .expect("a character to report on")
}

fn gm_messages(w: &World) -> Vec<(String, bool)> {
    world_chat_messages::table
        .filter(world_chat_messages::world_id.eq(w.world))
        .select((world_chat_messages::body, world_chat_messages::gm_only))
        .load(&mut w.conn())
        .unwrap()
}

fn rows(w: &World, spell: Uuid) -> Vec<(i32, bool, Option<Uuid>, String)> {
    attempts::table
        .filter(attempts::staged_id.eq(spell))
        .order(attempts::first_at.asc())
        .select((
            attempts::attempts,
            attempts::reported,
            attempts::chat_message_id,
            attempts::operation,
        ))
        .load(&mut w.conn())
        .unwrap()
}

/// The newest event in the world, or 0 when none has been written yet.
fn newest_event(w: &World) -> i64 {
    world_events::table
        .filter(world_events::world_id.eq(w.world))
        .select(diesel::dsl::max(world_events::id))
        .first::<Option<i64>>(&mut w.conn())
        .unwrap()
        .unwrap_or(0)
}

#[test]
fn the_first_attempt_is_recorded_and_posts_one_gm_only_message_of_facts() {
    let w = world();
    let (actor, spell) = held(&w);

    let outcome = attempt(&w, actor, spell, LastEvent(None), now());
    let Outcome::Reported { chat_message_id } = outcome else {
        panic!("expected a report, got {outcome:?}");
    };

    let messages = gm_messages(&w);
    assert_eq!(messages.len(), 1);
    let (body, gm_only) = &messages[0];
    assert!(gm_only, "the report is for the GM only");
    assert!(body.contains("Ember Lance"), "{body}");
    assert!(body.contains("has not been adopted"), "{body}");
    for verdict in ["cheat", "hack", "tamper", "suspicious"] {
        assert!(
            !body.to_lowercase().contains(verdict),
            "a fact, not a verdict: {body}"
        );
    }
    assert_eq!(
        rows(&w, spell),
        vec![(1, true, Some(chat_message_id), "makeAttack".to_string())]
    );
}

#[test]
fn a_second_attempt_within_the_window_is_counted_and_not_posted() {
    let w = world();
    let (actor, spell) = held(&w);
    let at = now();
    attempt(&w, actor, spell, LastEvent(None), at);
    assert_eq!(
        attempt(&w, actor, spell, LastEvent(None), at + Duration::minutes(3)),
        Outcome::Counted
    );
    assert_eq!(gm_messages(&w).len(), 1);
    assert_eq!(rows(&w, spell)[0].0, 2);

    // Once the window has passed, the next one is reported again.
    let later = at + Duration::minutes(WINDOW_MINUTES + 1);
    assert!(matches!(
        attempt(&w, actor, spell, LastEvent(None), later),
        Outcome::Reported { .. }
    ));
    assert_eq!(gm_messages(&w).len(), 2);
}

#[test]
fn a_client_that_has_not_seen_the_decline_is_not_reported() {
    let w = world();
    let (actor, spell) = held(&w);
    let before = newest_event(&w);
    decline(&mut w.conn(), w.gm, spell).expect("declined");
    let declined_at = newest_event(&w);
    assert!(declined_at > before);

    assert_eq!(
        attempt(&w, actor, spell, LastEvent(Some(before)), now()),
        Outcome::SuppressedStale,
        "a header older than the decline"
    );
    assert_eq!(
        attempt(&w, actor, spell, LastEvent(None), now()),
        Outcome::SuppressedStale,
        "a missing header"
    );
    assert!(rows(&w, spell).is_empty(), "nothing stored");
    assert!(gm_messages(&w).is_empty());

    // A client that had applied the decline has no excuse.
    assert!(matches!(
        attempt(&w, actor, spell, LastEvent(Some(declined_at)), now()),
        Outcome::Reported { .. }
    ));
}

#[test]
fn a_piece_never_delivered_is_always_reported() {
    let w = world();
    let (actor, spell) = held(&w);
    assert!(matches!(
        attempt(&w, actor, spell, LastEvent(None), now()),
        Outcome::Reported { .. }
    ));
}

#[test]
fn the_header_is_read_leniently_and_the_sentence_names_who_what_and_when() {
    assert_eq!(LastEvent::from_header(Some("42")), LastEvent(Some(42)));
    assert_eq!(LastEvent::from_header(Some(" 7 ")), LastEvent(Some(7)));
    for junk in [None, Some(""), Some("abc"), Some("-1")] {
        assert_eq!(LastEvent::from_header(junk), LastEvent(None));
    }
    let at = NaiveDateTime::parse_from_str("2026-10-09 20:15:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert_eq!(
        report_sentence("aria", "Moonblade", "Aria Vell", at),
        "aria tried to use Moonblade on Aria Vell at 2026-10-09 20:15 UTC. It came in with the character and has not been adopted."
    );
}
