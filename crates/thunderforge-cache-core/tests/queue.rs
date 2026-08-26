//! Spec 028 T071: the outbox must make silent loss of user work impossible
//! to overlook.

use thunderforge_cache_core::conflict::Role;
use thunderforge_cache_core::queue::{
    QueuedChange, ReconcileOutcome, RejectionReason, apply_outcomes, enqueue, replay_order,
};
use uuid::Uuid;

fn change(local: u128, world: u128, seq: u64) -> QueuedChange {
    QueuedChange {
        local_id: Uuid::from_u128(local),
        world_id: Uuid::from_u128(world),
        command: format!("move:{seq}"),
        enqueued_seq: seq,
        actor_role_hint: Role::Player,
    }
}

fn applied(local: u128) -> ReconcileOutcome {
    ReconcileOutcome {
        local_id: Uuid::from_u128(local),
        applied: true,
        reason: None,
    }
}

#[test]
fn replay_preserves_enqueue_order_within_a_world() {
    // "Move right, then move up" replayed out of order lands the token
    // somewhere the user never put it.
    let mut outbox = Vec::new();
    enqueue(&mut outbox, change(1, 1, 3));
    enqueue(&mut outbox, change(2, 1, 1));
    enqueue(&mut outbox, change(3, 1, 2));

    let order: Vec<u64> = replay_order(&outbox)
        .iter()
        .map(|c| c.enqueued_seq)
        .collect();
    assert_eq!(order, vec![1, 2, 3]);
}

#[test]
fn resolved_changes_are_drained() {
    let mut outbox = vec![change(1, 1, 1), change(2, 1, 2)];
    let unresolved = apply_outcomes(&mut outbox, &[applied(1), applied(2)]);

    assert!(unresolved.is_empty());
    assert!(outbox.is_empty());
}

#[test]
fn a_change_the_server_said_nothing_about_is_returned_not_dropped() {
    // FR-041. This return value existing is the enforcement: silent loss
    // becomes a value the caller must handle rather than an omission it can
    // quietly overlook.
    let mut outbox = vec![change(1, 1, 1), change(2, 1, 2)];
    let unresolved = apply_outcomes(&mut outbox, &[applied(1)]);

    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].0.local_id, Uuid::from_u128(2));
    assert_eq!(
        outbox.len(),
        1,
        "an unresolved change stays queued rather than vanishing"
    );
}

#[test]
fn a_rejected_change_is_still_resolved() {
    // Rejection is an answer. Only silence leaves work in limbo.
    let mut outbox = vec![change(1, 1, 1)];
    let unresolved = apply_outcomes(
        &mut outbox,
        &[ReconcileOutcome {
            local_id: Uuid::from_u128(1),
            applied: false,
            reason: Some(RejectionReason::Superseded),
        }],
    );

    assert!(unresolved.is_empty());
    assert!(outbox.is_empty());
}

#[test]
fn an_empty_outcome_set_leaves_everything_unresolved() {
    // The connection dropped again mid-submission. Nothing may be lost.
    let mut outbox = vec![change(1, 1, 1), change(2, 1, 2)];
    let unresolved = apply_outcomes(&mut outbox, &[]);

    assert_eq!(unresolved.len(), 2);
    assert_eq!(outbox.len(), 2);
}
