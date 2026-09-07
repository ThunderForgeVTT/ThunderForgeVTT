//! Which client of an account is at the table (spec 036 US3a, ADR-073).
//!
//! # One claim per account, and it is not a row
//!
//! An account may hold several live sessions — that is what ADR-073 removed
//! the login-time revoke for — but exactly one of its clients is the play
//! field. The rest are companion surfaces: a character sheet on the other
//! screen, the compendium, the lore. Two play fields for one person would
//! mean two engines, two event appliers and two peer endpoints for one human
//! being, which is where the sync problems would come from and which nobody
//! asked for.
//!
//! **The claim must never outlive the client holding it.** A window that
//! crashes, is closed, or loses its network must not lock its own account out
//! of the table behind a timeout nobody can see. So this is an in-process
//! registry whose entry is owned by a guard, and the guard is owned by the
//! stream that registered it — exactly the shape `peer_signaling.rs` uses,
//! and for the same reason it records there:
//!
//! > enforced by construction rather than by a cleanup job that can be
//! > forgotten, misconfigured, or skipped during a crash.
//!
//! A claim in a table needs a heartbeat, a timeout and a reaper, and its
//! failure mode is a person locked out by a window that is already gone.
//!
//! # Known boundary
//!
//! Per process, as presence and the peer registry already are. A
//! multi-process deployment needs a shared registry for all three together.
//! This does not introduce that constraint and does not fix it; ADR-073
//! records it rather than leaving it to be discovered.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use tokio::sync::broadcast;
use uuid::Uuid;

/// How many claim changes an account's listener may fall behind before it is
/// told to catch up rather than kept waiting. A person's own claim moves when
/// they move a window; a handful is generous.
const CLAIM_CHANNEL_CAPACITY: usize = 16;

/// Who holds the table for one account, right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The opaque per-page-load id the client generated. Never persisted,
    /// never linked to anything — the same notion `peer_signaling` uses.
    pub client_id: String,
    pub world_id: Uuid,
    pub claimed_at: chrono::NaiveDateTime,
}

struct Held {
    claim: Claim,
    /// Distinguishes this registration from a later one for the same account,
    /// so a guard dropped after being displaced removes nothing.
    token: u64,
}

#[derive(Default)]
pub struct PlayFieldRegistry {
    accounts: Mutex<HashMap<Uuid, Held>>,
    listeners: Mutex<HashMap<Uuid, broadcast::Sender<Option<Claim>>>>,
    next_token: AtomicU64,
}

/// The registration itself. Dropping it releases the claim — unless it has
/// already been displaced by a newer one, in which case it releases nothing.
pub struct PlayFieldGuard {
    registry: Arc<PlayFieldRegistry>,
    account_id: Uuid,
    token: u64,
}

impl PlayFieldRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the play field for this account.
    ///
    /// Always succeeds for a caller the resolver has already authorised: the
    /// newest claim wins and any previous holder is displaced. Refusing the
    /// new window instead would reproduce the defect this whole feature
    /// removes one level up — "you are already playing somewhere else" is the
    /// same dead end as "you have been signed out", and the person cannot
    /// always reach the other window to release it.
    pub fn claim(
        self: &Arc<Self>,
        account_id: Uuid,
        client_id: String,
        world_id: Uuid,
    ) -> (Claim, PlayFieldGuard) {
        let claim = Claim {
            client_id,
            world_id,
            claimed_at: chrono::Utc::now().naive_utc(),
        };
        let token = self.next_token.fetch_add(1, Ordering::Relaxed);

        // One guarded swap, so however many windows ask at once exactly one
        // holds it afterwards.
        self.accounts
            .lock()
            .expect("play field registry mutex poisoned")
            .insert(
                account_id,
                Held {
                    claim: claim.clone(),
                    token,
                },
            );

        self.announce(account_id, Some(claim.clone()));

        (
            claim,
            PlayFieldGuard {
                registry: Arc::clone(self),
                account_id,
                token,
            },
        )
    }

    /// Who holds this account's play field, if anybody does.
    pub fn current(&self, account_id: Uuid) -> Option<Claim> {
        self.accounts
            .lock()
            .expect("play field registry mutex poisoned")
            .get(&account_id)
            .map(|held| held.claim.clone())
    }

    /// Release a claim, but only if `token` still identifies the holder.
    ///
    /// A guard whose claim was taken over releases nothing: the entry it
    /// would remove belongs to whoever displaced it.
    fn release(&self, account_id: Uuid, token: u64) {
        let released = {
            let mut accounts = self
                .accounts
                .lock()
                .expect("play field registry mutex poisoned");
            match accounts.get(&account_id) {
                Some(held) if held.token == token => {
                    accounts.remove(&account_id);
                    true
                }
                _ => false,
            }
        };
        if released {
            self.announce(account_id, None);
        }
    }

    /// Watch this account's claim. Fires whenever it moves, so a demoted
    /// client learns it has become a companion.
    ///
    /// Prunes senders nobody is listening to on the way past. Without it the
    /// map grows by one entry per account that has ever watched and never
    /// shrinks — slow, but unbounded, and `peer_signaling` now calls this on
    /// every peer registration rather than only when a play field mounts,
    /// which is what made a slow leak worth closing. A scan is affordable
    /// because this runs when a window opens a table or starts a transfer,
    /// not per request, and the map is keyed by account rather than by
    /// connection.
    pub fn watch(&self, account_id: Uuid) -> broadcast::Receiver<Option<Claim>> {
        let mut listeners = self
            .listeners
            .lock()
            .expect("play field listeners mutex poisoned");
        listeners.retain(|watched, tx| *watched == account_id || tx.receiver_count() > 0);
        listeners
            .entry(account_id)
            .or_insert_with(|| broadcast::channel(CLAIM_CHANNEL_CAPACITY).0)
            .subscribe()
    }

    fn announce(&self, account_id: Uuid, claim: Option<Claim>) {
        if let Some(tx) = self
            .listeners
            .lock()
            .expect("play field listeners mutex poisoned")
            .get(&account_id)
        {
            // Nobody listening is the ordinary case, not a failure.
            let _ = tx.send(claim);
        }
    }
}

impl Drop for PlayFieldGuard {
    fn drop(&mut self) {
        self.registry.release(self.account_id, self.token);
    }
}

/// The process's registry of play-field claims.
pub fn registry() -> &'static Arc<PlayFieldRegistry> {
    static REGISTRY: LazyLock<Arc<PlayFieldRegistry>> =
        LazyLock::new(|| Arc::new(PlayFieldRegistry::new()));
    &REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_for_test() -> Arc<PlayFieldRegistry> {
        Arc::new(PlayFieldRegistry::new())
    }

    /// FR-027. One account, one table.
    #[test]
    fn the_newest_claim_wins_and_there_is_only_ever_one() {
        let registry = registry_for_test();
        let account = Uuid::now_v7();
        let world = Uuid::now_v7();

        let (_first, _first_guard) = registry.claim(account, "window-a".into(), world);
        let (second, _second_guard) = registry.claim(account, "window-b".into(), world);

        let held = registry.current(account).expect("somebody holds it");
        assert_eq!(held.client_id, "window-b");
        assert_eq!(held, second);
    }

    /// FR-030. The requirement a table could not satisfy without a reaper:
    /// a claim never outlives the client that made it.
    #[test]
    fn dropping_the_guard_releases_the_claim() {
        let registry = registry_for_test();
        let account = Uuid::now_v7();

        {
            let (_claim, _guard) = registry.claim(account, "window-a".into(), Uuid::now_v7());
            assert!(registry.current(account).is_some());
        }

        assert!(
            registry.current(account).is_none(),
            "a closed or crashed client must not keep holding the table"
        );
    }

    /// The subtle one. A displaced guard is still alive — its stream has not
    /// ended — and when it does end it must not take the new holder's claim
    /// with it.
    #[test]
    fn a_displaced_guard_releases_nothing_when_it_finally_drops() {
        let registry = registry_for_test();
        let account = Uuid::now_v7();
        let world = Uuid::now_v7();

        let (_first, first_guard) = registry.claim(account, "window-a".into(), world);
        let (_second, _second_guard) = registry.claim(account, "window-b".into(), world);

        drop(first_guard);

        let held = registry
            .current(account)
            .expect("the newer claim must survive the older guard being dropped");
        assert_eq!(held.client_id, "window-b");
    }

    /// The leak `peer_signaling` found: a sender per account, kept forever.
    #[test]
    fn watchers_nobody_is_listening_to_are_not_kept() {
        let registry = registry_for_test();
        let account = Uuid::now_v7();

        // A hundred accounts watch and go away — the shape of a hundred peer
        // registrations, each of which now watches to answer "is this client
        // at the table?".
        for _ in 0..100 {
            let _ = registry.watch(Uuid::now_v7());
        }
        // One that is still listening.
        let _live = registry.watch(account);

        let held = registry.listeners.lock().expect("listeners").len();
        assert_eq!(
            held, 1,
            "only the account with a live listener should still have a sender, got {held}"
        );
    }

    /// One person's table is not another's.
    #[test]
    fn accounts_do_not_share_a_claim() {
        let registry = registry_for_test();
        let (one, two) = (Uuid::now_v7(), Uuid::now_v7());
        let world = Uuid::now_v7();

        let (_a, _guard_a) = registry.claim(one, "window-a".into(), world);
        let (_b, _guard_b) = registry.claim(two, "window-b".into(), world);

        assert_eq!(registry.current(one).unwrap().client_id, "window-a");
        assert_eq!(registry.current(two).unwrap().client_id, "window-b");
    }

    /// FR-029. A demoted client is told, which is what lets it say so rather
    /// than carrying on as though it were still at the table.
    #[tokio::test]
    async fn a_displaced_client_is_told_the_claim_moved() {
        let registry = registry_for_test();
        let account = Uuid::now_v7();
        let world = Uuid::now_v7();

        let (_first, _first_guard) = registry.claim(account, "window-a".into(), world);
        let mut watching = registry.watch(account);

        let (_second, _second_guard) = registry.claim(account, "window-b".into(), world);

        let moved = watching.recv().await.expect("a claim change");
        assert_eq!(
            moved.expect("somebody holds it").client_id,
            "window-b",
            "the notification must name the new holder"
        );
    }

    /// And releasing is a change too — otherwise a companion offering to take
    /// the table back would never learn that nobody holds it.
    #[tokio::test]
    async fn releasing_is_announced_as_nobody_holding_it() {
        let registry = registry_for_test();
        let account = Uuid::now_v7();
        let mut watching = registry.watch(account);

        {
            let (_claim, _guard) = registry.claim(account, "window-a".into(), Uuid::now_v7());
        }

        assert!(watching.recv().await.expect("the claim").is_some());
        assert!(
            watching.recv().await.expect("the release").is_none(),
            "a released claim must be announced as nobody holding it"
        );
    }
}
