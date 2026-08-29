//! Who is at the table right now, held in memory rather than in Postgres.
//!
//! # Why this moved out of the database
//!
//! Every connected client sends a heartbeat every five seconds, and each beat
//! used to upsert a row into `players_online`. That is 0.2 writes per second
//! per client, forever, and it does not scale with anything about the game: a
//! table sitting idle in a tavern scene costs exactly as much as one in
//! combat. At a thousand six-player tables it is roughly **1,200 writes per
//! second**, six times what actual play generates — and every one of them
//! creates a dead tuple for autovacuum to collect later.
//!
//! It also bought nothing that memory does not. Presence is read to answer
//! "who is here *now*", a question whose answer is worthless one beat later.
//!
//! # Why in-memory is more correct, not merely cheaper
//!
//! A presence row that survives a restart is stale by definition. The
//! process died, so every socket died with it, but the rows sit there
//! claiming a table full of people. A registry that lives in the process can
//! only ever describe that process's live clients, and clients re-establish
//! themselves within one beat.
//!
//! # The two thresholds, and why there are two
//!
//! - [`PRESENCE_TIMEOUT`] — silence past this and somebody is **shown as
//!   disconnected**. They are still listed.
//! - [`FORGET_AFTER`] — silence past this and they are **dropped entirely**.
//!
//! Collapsing these into one would make a player who dropped simply vanish
//! from the Game Master's list, which reads as "they left" rather than "their
//! connection died" — a materially different thing to tell a table mid-session.
//! Keeping them forever would make this unbounded, so the second threshold is
//! what makes "memory-bound" a bound.
//!
//! # What this deliberately is not
//!
//! Not authorization. The caller checks membership before recording a beat;
//! this registry believes whatever it is told. Not durable. And not
//! cross-instance: a second app instance has its own registry and its own
//! answer, which is fine while there is one instance and needs a gossip
//! channel before there are two.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use uuid::Uuid;

/// How long silence lasts before a client is shown as disconnected.
///
/// Three missed beats at the client's five-second interval. One missed beat
/// is a garbage collection pause or a train tunnel; three is somebody who
/// stopped being there. Erring long is right because the cost is asymmetric:
/// announcing that a player dropped when they have not is visible to the whole
/// table and momentarily wrong in public, while noticing fifteen seconds late
/// costs nothing anyone can see.
pub const PRESENCE_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a departed client stays listed before being forgotten.
///
/// Long enough that a Game Master glancing up after a fight still sees who
/// dropped out of it; short enough that memory is bounded by the people who
/// were recently here rather than by everyone who ever visited.
pub const FORGET_AFTER: Duration = Duration::from_secs(5 * 60);

/// One participant, as the registry holds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Beat {
    scene_id: Option<Uuid>,
    last_seen: Instant,
}

/// One participant, as everyone else sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presence {
    pub user_id: Uuid,
    /// Which scene they are looking at, when they have said.
    pub scene_id: Option<Uuid>,
    /// How long since their last beat.
    pub since_seen: Duration,
    /// Whether that silence has passed [`PRESENCE_TIMEOUT`].
    pub connected: bool,
}

/// Who is present, per world.
///
/// Sharded by world so a busy table's beats never contend with another
/// table's — the same reason the event router is sharded.
#[derive(Debug, Default)]
pub struct PresenceRegistry {
    worlds: DashMap<Uuid, HashMap<Uuid, Beat>>,
}

impl PresenceRegistry {
    pub fn new() -> Self {
        Self {
            worlds: DashMap::new(),
        }
    }

    /// Record that `user_id` is still present in `world_id`.
    ///
    /// The caller must already have established that this person is a member
    /// of this world — see the note on authorization in the module docs.
    pub fn beat(&self, world_id: Uuid, user_id: Uuid, scene_id: Option<Uuid>, now: Instant) {
        self.worlds.entry(world_id).or_default().insert(
            user_id,
            Beat {
                scene_id,
                last_seen: now,
            },
        );
    }

    /// Everyone currently listed for a world, connected or recently gone.
    ///
    /// Expiry happens here rather than on a timer. Presence is read rarely
    /// and written constantly, so sweeping on read costs nothing on the hot
    /// path and removes a background task that would otherwise have to exist,
    /// be scheduled, and be reasoned about when it stops running.
    pub fn in_world(&self, world_id: Uuid, now: Instant) -> Vec<Presence> {
        let Some(mut entry) = self.worlds.get_mut(&world_id) else {
            return Vec::new();
        };

        entry.retain(|_, beat| now.duration_since(beat.last_seen) < FORGET_AFTER);

        let mut people: Vec<Presence> = entry
            .iter()
            .map(|(user_id, beat)| {
                let since_seen = now.duration_since(beat.last_seen);
                Presence {
                    user_id: *user_id,
                    scene_id: beat.scene_id,
                    since_seen,
                    connected: since_seen <= PRESENCE_TIMEOUT,
                }
            })
            .collect();

        // A stable order, so a Game Master's list does not reshuffle itself
        // on every poll. `HashMap` iteration order is arbitrary and varies
        // run to run.
        people.sort_by_key(|person| person.user_id);
        people
    }

    /// Forget one participant immediately — a deliberate leave, not a timeout.
    pub fn forget(&self, world_id: Uuid, user_id: Uuid) {
        if let Some(mut entry) = self.worlds.get_mut(&world_id) {
            entry.remove(&user_id);
        }
    }

    /// Drop worlds nobody has been seen in for [`FORGET_AFTER`].
    ///
    /// [`Self::in_world`] prunes people, but only for worlds somebody asks
    /// about; a world nobody ever queries again would keep its map forever.
    /// Returns how many worlds were dropped.
    ///
    /// Deliberately separate from the read path and expected to run on its own
    /// schedule: this walks every shard, and the event router learned the hard
    /// way that an all-shards scan does not belong on a hot path.
    pub fn sweep(&self, now: Instant) -> usize {
        let mut dropped = 0;
        self.worlds.retain(|_, people| {
            people.retain(|_, beat| now.duration_since(beat.last_seen) < FORGET_AFTER);
            let keep = !people.is_empty();
            if !keep {
                dropped += 1;
            }
            keep
        });
        dropped
    }

    /// How many worlds are currently held, for metrics and tests.
    pub fn worlds_tracked(&self) -> usize {
        self.worlds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic ids: a failing assertion should name the same person
    /// every run, and the stable-order test would be meaningless against
    /// values that change between runs.
    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn world() -> Uuid {
        id(999)
    }

    #[test]
    fn a_beat_makes_somebody_present() {
        let registry = PresenceRegistry::new();
        let (w, u, now) = (world(), id(1), Instant::now());

        registry.beat(w, u, None, now);

        let people = registry.in_world(w, now);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].user_id, u);
        assert!(people[0].connected);
    }

    #[test]
    fn a_world_nobody_has_beaten_in_is_empty_rather_than_missing() {
        let registry = PresenceRegistry::new();
        assert!(registry.in_world(world(), Instant::now()).is_empty());
    }

    /// The timeout boundary, from both sides.
    ///
    /// Exactly the kind of rule that is untestable when the clock is real and
    /// the threshold is fifteen seconds.
    #[test]
    fn silence_past_the_timeout_shows_as_disconnected_but_still_listed() {
        let registry = PresenceRegistry::new();
        let (w, u, start) = (world(), id(1), Instant::now());
        registry.beat(w, u, None, start);

        let just_inside = registry.in_world(w, start + PRESENCE_TIMEOUT);
        assert!(
            just_inside[0].connected,
            "the boundary itself is still present"
        );

        let just_outside =
            registry.in_world(w, start + PRESENCE_TIMEOUT + Duration::from_millis(1));
        assert_eq!(just_outside.len(), 1, "they must still be listed");
        assert!(!just_outside[0].connected, "shown as gone, not removed");
    }

    /// Why there are two thresholds and not one.
    #[test]
    fn somebody_long_gone_is_forgotten_rather_than_listed_forever() {
        let registry = PresenceRegistry::new();
        let (w, u, start) = (world(), id(1), Instant::now());
        registry.beat(w, u, None, start);

        assert_eq!(
            registry
                .in_world(w, start + FORGET_AFTER - Duration::from_secs(1))
                .len(),
            1
        );
        assert!(
            registry.in_world(w, start + FORGET_AFTER).is_empty(),
            "memory-bound means somebody eventually leaves the map"
        );
    }

    #[test]
    fn a_later_beat_refreshes_somebody_who_had_gone_quiet() {
        let registry = PresenceRegistry::new();
        let (w, u, start) = (world(), id(1), Instant::now());
        registry.beat(w, u, None, start);

        let lapsed = start + PRESENCE_TIMEOUT + Duration::from_secs(1);
        assert!(!registry.in_world(w, lapsed)[0].connected);

        registry.beat(w, u, None, lapsed);
        assert!(registry.in_world(w, lapsed)[0].connected);
    }

    #[test]
    fn a_beat_updates_which_scene_somebody_is_looking_at() {
        let registry = PresenceRegistry::new();
        let (w, u, now) = (world(), id(1), Instant::now());
        let tavern = id(1);
        let dungeon = id(1);

        registry.beat(w, u, Some(tavern), now);
        assert_eq!(registry.in_world(w, now)[0].scene_id, Some(tavern));

        registry.beat(w, u, Some(dungeon), now);
        assert_eq!(registry.in_world(w, now)[0].scene_id, Some(dungeon));
        assert_eq!(registry.in_world(w, now).len(), 1, "still one person");
    }

    #[test]
    fn one_world_never_reports_another_worlds_people() {
        let registry = PresenceRegistry::new();
        let (a, b, now) = (id(0xA), id(0xB), Instant::now());
        let alice = id(1);
        let bob = id(2);

        registry.beat(a, alice, None, now);
        registry.beat(b, bob, None, now);

        assert_eq!(registry.in_world(a, now).len(), 1);
        assert_eq!(registry.in_world(a, now)[0].user_id, alice);
        assert_eq!(registry.in_world(b, now)[0].user_id, bob);
    }

    #[test]
    fn leaving_deliberately_removes_somebody_at_once() {
        let registry = PresenceRegistry::new();
        let (w, u, now) = (world(), id(1), Instant::now());
        registry.beat(w, u, None, now);

        registry.forget(w, u);

        assert!(registry.in_world(w, now).is_empty());
    }

    /// The list must not reshuffle between polls.
    #[test]
    fn the_order_is_stable_across_reads() {
        let registry = PresenceRegistry::new();
        let (w, now) = (world(), Instant::now());
        for n in 0..16 {
            registry.beat(w, id(100 + n), None, now);
        }

        let first = registry.in_world(w, now);
        let second = registry.in_world(w, now);
        assert_eq!(first, second);
    }

    /// Without this, a world nobody queries again holds its map forever.
    #[test]
    fn sweeping_drops_worlds_everyone_has_left() {
        let registry = PresenceRegistry::new();
        let (w, start) = (world(), Instant::now());
        registry.beat(w, id(1), None, start);
        assert_eq!(registry.worlds_tracked(), 1);

        assert_eq!(
            registry.sweep(start + Duration::from_secs(1)),
            0,
            "still occupied"
        );
        assert_eq!(registry.sweep(start + FORGET_AFTER), 1);
        assert_eq!(registry.worlds_tracked(), 0);
    }

    #[test]
    fn sweeping_keeps_a_world_that_still_has_somebody_in_it() {
        let registry = PresenceRegistry::new();
        let (w, start) = (world(), Instant::now());
        let left = id(1);
        let stayed = id(2);
        registry.beat(w, left, None, start);
        registry.beat(w, stayed, None, start + FORGET_AFTER);

        registry.sweep(start + FORGET_AFTER);

        assert_eq!(registry.worlds_tracked(), 1);
        let people = registry.in_world(w, start + FORGET_AFTER);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].user_id, stayed, "the one who left is gone");
    }
}
