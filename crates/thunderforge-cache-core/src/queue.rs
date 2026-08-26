//! The offline outbox: what was changed while disconnected, and what became
//! of it.
//!
//! Spec 028 FR-037/FR-038/FR-041, data-model.md.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::conflict::Role;

/// One edit made while disconnected, awaiting reconnection.
///
/// `command` holds the emitted world-store command **verbatim**. That is what
/// keeps this an outbox rather than a second simulator (Constitution
/// Principle I): replaying it traverses exactly the same mutation and
/// authorization path an online change would, so re-authorization at
/// reconnect time (FR-042) is automatic rather than a mechanism of its own to
/// get right.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct QueuedChange {
    pub local_id: Uuid,
    pub world_id: Uuid,
    /// Serialized world-store command. Opaque here by design — this crate
    /// orders and accounts for changes, it does not interpret them.
    pub command: String,
    /// Client-side ordering only. Never consulted by conflict resolution; see
    /// [`crate::conflict`].
    pub enqueued_seq: u64,
    /// The role the client believed it had. A hint for predicting the
    /// outcome; the server re-derives the truth and does not trust this.
    pub actor_role_hint: Role,
}

/// Why a queued change did not stand.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RejectionReason {
    /// The user may no longer make this change (FR-042).
    PermissionDenied,
    /// A Game Master's conflicting change took precedence. Distinguished from
    /// a generic failure so the UI can say what actually happened rather than
    /// reporting an error for a working rule.
    Superseded,
    /// The thing being changed no longer exists.
    GoneAway,
    /// Malformed or inapplicable.
    Invalid,
}

/// What became of one queued change.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReconcileOutcome {
    pub local_id: Uuid,
    pub applied: bool,
    pub reason: Option<RejectionReason>,
}

/// A queued change the server said nothing about.
///
/// Its existence as a return value is the enforcement of FR-041: silent loss
/// of someone's work becomes a value the caller has to handle rather than an
/// omission it can quietly overlook.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnresolvedChange(pub QueuedChange);

/// Append a change to the outbox.
pub fn enqueue(outbox: &mut Vec<QueuedChange>, change: QueuedChange) {
    outbox.push(change);
}

/// The order changes must be replayed in.
///
/// Enqueue order within a world is preserved: a user's own sequential edits
/// must not reorder against each other, or replaying "move right, then move
/// up" could land the token somewhere they never put it.
pub fn replay_order(outbox: &[QueuedChange]) -> Vec<&QueuedChange> {
    let mut ordered: Vec<&QueuedChange> = outbox.iter().collect();
    ordered.sort_by_key(|c| (c.world_id, c.enqueued_seq));
    ordered
}

/// Apply the server's outcomes, draining what it resolved.
///
/// Returns every change the server did not account for. Callers must surface
/// these to the user; discarding them silently is precisely what FR-041
/// prohibits.
pub fn apply_outcomes(
    outbox: &mut Vec<QueuedChange>,
    outcomes: &[ReconcileOutcome],
) -> Vec<UnresolvedChange> {
    let resolved: Vec<Uuid> = outcomes.iter().map(|o| o.local_id).collect();

    let (done, pending): (Vec<QueuedChange>, Vec<QueuedChange>) = outbox
        .drain(..)
        .partition(|c| resolved.contains(&c.local_id));

    let _ = done;
    let unresolved = pending.iter().cloned().map(UnresolvedChange).collect();
    *outbox = pending;
    unresolved
}
