//! The durable offline outbox: edits made while disconnected (T072).
//!
//! Spec 028 FR-037/FR-038/FR-041, data-model.md.
//!
//! [`thunderforge_cache_core::queue`] owns the *rules* — what order changes
//! replay in, and which of them the server failed to account for. This module
//! owns the only thing that cannot be decided without a browser: making them
//! survive the tab closing.
//!
//! # Durability is the entire feature
//!
//! An outbox that loses entries is worse than no outbox, because the user was
//! told their edit was accepted. So a change is written to IndexedDB **before
//! the caller acknowledges it locally** (FR-037), and [`OutboxStore::append`]
//! is `async` and fallible for that reason: a caller that ignores its result
//! and reports success anyway has reintroduced the bug this exists to
//! prevent. The write is one `put` and completes in the same interaction the
//! user is already waiting on.
//!
//! # Commands are stored verbatim, and never interpreted here
//!
//! `QueuedChange::command` is the emitted world-store command as a string.
//! Nothing in this crate parses it, and that is Constitution Principle I: on
//! reconnect the server replays it through the ordinary mutation path, so
//! authorization is traversed identically to an online edit and FR-042 costs
//! no separate mechanism. The moment this module started understanding
//! commands it would be a second simulator, and the two would drift.
//!
//! # Why the sequence is derived and not stored
//!
//! `enqueued_seq` orders a user's own edits against each other — replaying
//! "move right, then move up" out of order lands the token somewhere they
//! never put it. The counter is recovered by reading the high-water mark off
//! the stored rows, exactly as `index::high_water` does for reads, so it
//! cannot drift out of step with the rows it orders. A separately persisted
//! counter that was written and then lost in a crash would hand out sequence
//! numbers that compare as older than entries already queued.

use thunderforge_cache_core::queue::{QueuedChange, ReconcileOutcome};
use uuid::Uuid;

/// The next sequence number to hand out, given what is already queued.
///
/// One past the highest seen, and `0` for an empty outbox. Pure so the
/// ordering guarantee can be tested without a browser.
pub fn next_seq(queued: &[QueuedChange]) -> u64 {
    queued
        .iter()
        .map(|change| change.enqueued_seq)
        .max()
        .map_or(0, |high| high.saturating_add(1))
}

/// Entries for one world, in the order they must be replayed.
///
/// Wraps `queue::replay_order` rather than re-deriving the ordering, so there
/// is exactly one definition of "what order do these go in" and it is the
/// tested one.
pub fn replay_for_world(queued: &[QueuedChange], world_id: Uuid) -> Vec<QueuedChange> {
    thunderforge_cache_core::queue::replay_order(queued)
        .into_iter()
        .filter(|change| change.world_id == world_id)
        .cloned()
        .collect()
}

/// The storage key for one queued change.
///
/// The local id, which is generated client-side and never reused, so an
/// append cannot silently overwrite a pending edit.
pub fn outbox_key(local_id: Uuid) -> String {
    local_id.to_string()
}

/// Local ids the server accounted for, in the order given.
///
/// Separated from the deletion loop so "which rows may go" is decidable
/// without a database — the mistake worth guarding against is deleting a row
/// the server said nothing about, which is silent loss of the user's work
/// (FR-041).
pub fn resolved_ids(outcomes: &[ReconcileOutcome]) -> Vec<Uuid> {
    outcomes.iter().map(|outcome| outcome.local_id).collect()
}

#[cfg(target_arch = "wasm32")]
pub use wasm::OutboxStore;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use thunderforge_cache_core::queue::{QueuedChange, ReconcileOutcome};
    use uuid::Uuid;
    use wasm_bindgen::JsValue;

    use super::{next_seq, outbox_key, resolved_ids};
    use crate::idb::Db;
    use crate::{CacheError, Result, STORE_OUTBOX};

    /// The `outbox` object store.
    pub struct OutboxStore {
        db: Db,
    }

    impl OutboxStore {
        pub async fn open() -> Result<Self> {
            Ok(Self { db: Db::open().await? })
        }

        /// Every queued change, oldest first by sequence.
        ///
        /// A row that will not parse is skipped rather than failing the whole
        /// read: one unreadable entry must not make the rest of someone's
        /// queued work unreachable. It is also not deleted — see
        /// [`Self::forget_resolved`] for why nothing here removes a row the
        /// server has not spoken about.
        pub async fn all(&self) -> Result<Vec<QueuedChange>> {
            let mut changes: Vec<QueuedChange> = Vec::new();
            for (_, value) in self.db.entries(STORE_OUTBOX).await? {
                let Some(text) = value.as_string() else {
                    continue;
                };
                if let Ok(change) = serde_json::from_str::<QueuedChange>(&text) {
                    changes.push(change);
                }
            }
            changes.sort_by_key(|change| change.enqueued_seq);
            Ok(changes)
        }

        /// Persist one change, and answer with what was stored.
        ///
        /// The sequence number is assigned here, from the high-water mark of
        /// what is already queued, so callers cannot hand out two edits with
        /// the same order.
        ///
        /// **Await this before telling the user their edit was accepted.**
        /// That ordering is FR-037 and the whole point of the module; an
        /// optimistic local apply followed by a failed write is precisely the
        /// silent loss this is here to prevent.
        pub async fn append(
            &self,
            world_id: Uuid,
            local_id: Uuid,
            command: &str,
            actor_role_hint: thunderforge_cache_core::conflict::Role,
        ) -> Result<QueuedChange> {
            let queued = self.all().await?;
            let change = QueuedChange {
                local_id,
                world_id,
                command: command.to_owned(),
                enqueued_seq: next_seq(&queued),
                actor_role_hint,
            };
            let encoded = serde_json::to_string(&change)
                .map_err(|err| CacheError::Corrupt(err.to_string()))?;
            self.db
                .put(
                    STORE_OUTBOX,
                    &outbox_key(local_id),
                    &JsValue::from_str(&encoded),
                )
                .await?;
            Ok(change)
        }

        /// Drop the rows the server accounted for, and only those.
        ///
        /// Returns the changes still queued afterwards — every entry the
        /// server said nothing about. That return value is the enforcement of
        /// FR-041: an unresolved change becomes something the caller has to
        /// handle rather than an omission it can overlook. Nothing here
        /// deletes on any other basis, so a reconcile that answers partially
        /// costs the user nothing.
        pub async fn forget_resolved(
            &self,
            outcomes: &[ReconcileOutcome],
        ) -> Result<Vec<QueuedChange>> {
            for local_id in resolved_ids(outcomes) {
                // A delete that fails leaves the row queued, which replays it
                // next time. Replaying an applied change is safe — the server
                // gives one outcome per submitted change and the second pass
                // simply gets another — while dropping it is not recoverable.
                let _ = self.db.delete(STORE_OUTBOX, &outbox_key(local_id)).await;
            }
            self.all().await
        }

        /// Everything queued for one world, in replay order.
        pub async fn for_world(&self, world_id: Uuid) -> Result<Vec<QueuedChange>> {
            Ok(super::replay_for_world(&self.all().await?, world_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_cache_core::conflict::Role;
    use thunderforge_cache_core::queue::RejectionReason;

    fn change(local: u128, world: u128, seq: u64) -> QueuedChange {
        QueuedChange {
            local_id: Uuid::from_u128(local),
            world_id: Uuid::from_u128(world),
            command: format!("{{\"type\":\"move\",\"seq\":{seq}}}"),
            enqueued_seq: seq,
            actor_role_hint: Role::Player,
        }
    }

    #[test]
    fn the_first_change_starts_the_sequence_at_zero() {
        assert_eq!(next_seq(&[]), 0);
    }

    /// The counter is recovered from the rows rather than stored, so a reload
    /// mid-session must not hand out a number that sorts before work already
    /// queued — which would replay a user's edits in an order they never
    /// made them.
    #[test]
    fn the_sequence_resumes_above_everything_already_queued() {
        let queued = vec![change(1, 100, 0), change(2, 100, 7), change(3, 100, 3)];

        assert_eq!(next_seq(&queued), 8);
    }

    #[test]
    fn the_sequence_saturates_rather_than_wrapping_past_the_end() {
        let mut queued = vec![change(1, 100, 0)];
        queued[0].enqueued_seq = u64::MAX;

        assert_eq!(next_seq(&queued), u64::MAX);
    }

    /// Replay order is per world and by sequence. A user's sequential edits
    /// must not reorder against each other: "move right, then move up"
    /// replayed backwards lands the token somewhere they never put it.
    #[test]
    fn one_worlds_changes_replay_in_the_order_they_were_made() {
        let queued = vec![
            change(1, 100, 2),
            change(2, 200, 0),
            change(3, 100, 0),
            change(4, 100, 1),
        ];

        let order: Vec<u64> = replay_for_world(&queued, Uuid::from_u128(100))
            .iter()
            .map(|c| c.enqueued_seq)
            .collect();

        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn another_worlds_changes_are_not_replayed_into_this_one() {
        let queued = vec![change(1, 100, 0), change(2, 200, 1)];

        let ids: Vec<Uuid> = replay_for_world(&queued, Uuid::from_u128(200))
            .iter()
            .map(|c| c.local_id)
            .collect();

        assert_eq!(ids, vec![Uuid::from_u128(2)]);
    }

    /// Only what the server spoke about may be deleted. A row it omitted is
    /// the user's work, and dropping it is exactly what FR-041 forbids.
    #[test]
    fn only_the_ids_the_server_answered_are_resolved() {
        let outcomes = vec![
            ReconcileOutcome {
                local_id: Uuid::from_u128(1),
                applied: true,
                reason: None,
            },
            ReconcileOutcome {
                local_id: Uuid::from_u128(2),
                applied: false,
                reason: Some(RejectionReason::Superseded),
            },
        ];

        assert_eq!(
            resolved_ids(&outcomes),
            vec![Uuid::from_u128(1), Uuid::from_u128(2)]
        );
    }

    /// A rejected change is still *resolved*: the server accounted for it and
    /// said no. Treating rejection as unresolved would replay it forever.
    #[test]
    fn a_rejection_counts_as_resolved_and_is_not_replayed_again() {
        let outcomes = vec![ReconcileOutcome {
            local_id: Uuid::from_u128(9),
            applied: false,
            reason: Some(RejectionReason::PermissionDenied),
        }];

        assert!(resolved_ids(&outcomes).contains(&Uuid::from_u128(9)));
    }

    #[test]
    fn an_empty_reconcile_resolves_nothing() {
        assert!(resolved_ids(&[]).is_empty());
    }

    /// The key is the local id, which is generated client-side and never
    /// reused, so appending cannot overwrite a pending edit.
    #[test]
    fn each_change_is_keyed_by_its_own_local_id() {
        assert_eq!(
            outbox_key(Uuid::from_u128(5)),
            Uuid::from_u128(5).to_string()
        );
        assert_ne!(outbox_key(Uuid::from_u128(5)), outbox_key(Uuid::from_u128(6)));
    }
}
