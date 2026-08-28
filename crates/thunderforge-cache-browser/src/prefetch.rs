//! Warming the cache for scenes the user has not opened yet.
//!
//! Spec 028 FR-069–FR-073 (T116, T117), user story 8.
//!
//! # What this is, and what it deliberately is not
//!
//! Switching to a scene nobody has visited is a cold load: the plan named its
//! assets, nothing fetched them, and the user waits. This module decides,
//! item by item, whether to spend a moment fetching one *before* they ask.
//!
//! It needs no Service Worker, no push subscription and no background-sync
//! registration, and FR-073 forbids introducing one. Every input it uses is
//! already in the tab: the page is open, the wasm is running, and the sync
//! plan the caller just received names exactly what is missing. Nothing here
//! can run while the application is closed, because nothing here exists while
//! the application is closed — which is a privacy property, not merely an
//! implementation detail.
//!
//! # The three rules, and where each is enforced
//!
//! 1. **Only the caller's own plan** (FR-072). [`PrefetchQueue::from_plan`]
//!    is the only constructor, and there is no method that adds an item. The
//!    entitlement is therefore the server's, exactly as it is for a demand
//!    fetch: the plan was computed against this caller's permissions, so a
//!    queue built from it cannot name content they may not see.
//! 2. **Only the open world** (FR-073). The queue is stamped with the world
//!    it was built for and re-checks it on every step, so a task left running
//!    across a world switch stops at the next item rather than fetching into
//!    a world the user has left.
//! 3. **Always yields** (FR-070, FR-071). Demand work in flight yields;
//!    a superseded plan stops; a full store stops rather than evicting, via
//!    [`budget::admit_speculative`].
//!
//! # Why the decision is a pure function
//!
//! [`PrefetchQueue::step`] performs no I/O and takes every input as a value.
//! The caller does the fetching and reports back what it stored. That keeps
//! the whole of the policy — the part that can be wrong in ways nobody
//! notices until a user's cache is full or a stale world is being fetched —
//! under plain `cargo test`, and leaves the browser half with nothing to
//! decide.

use std::collections::VecDeque;

use thunderforge_cache_core::budget::{self, Speculation};
use thunderforge_cache_core::delta::SyncPlan;
use thunderforge_cache_core::{Fingerprint, ItemId};
use uuid::Uuid;

/// How much one world open may pull ahead of demand.
///
/// A cold world's plan lists every asset in it, and fetching all of them is
/// how the *next* visit becomes free. But an unbounded prefetch would let a
/// large world spend more of this visit's bandwidth than the visit itself
/// needs, competing with the scene the user is waiting on. Past the ceiling
/// the remainder is left to the demand path, which caches items as they are
/// used — slower to warm, never wrong.
pub const VISIT_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

/// One thing worth fetching before it is asked for.
///
/// Constructible from outside — the fields are plain data — but that buys
/// nobody anything, because there is no way to put one *into* a queue. The
/// only items a queue ever yields are the ones [`PrefetchQueue::from_plan`]
/// read out of a plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PrefetchItem {
    pub asset_id: Uuid,
    pub fingerprint: Fingerprint,
    pub byte_size: u64,
}

/// Everything outside the queue that can change what it should do next.
///
/// Passed on every step rather than captured at construction, because every
/// one of these moves *while* a prefetch is running: the user opens another
/// world, a live update arrives, a demand fetch starts, the store fills up.
/// A queue that read them once would be deciding on the state of the world
/// at the moment the last sync finished.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pressure {
    /// The world the user is looking at *now*.
    pub open_world: Uuid,
    /// Bumped whenever a newer plan or a live fingerprint update lands.
    pub plan_epoch: u64,
    /// User-initiated loads currently outstanding. Any at all yields.
    pub demand_in_flight: usize,
    /// Plaintext bytes the index accounts for, including anything this
    /// prefetch has already stored.
    pub in_use_bytes: u64,
    /// The budget ceiling from the last pass. Zero when the platform
    /// declined to estimate, which stops speculation (FR-071).
    pub limit_bytes: u64,
    /// The FR-024 verdict: false when the store has no room for writes at
    /// all and loads are being served unfiled.
    pub may_store: bool,
}

/// What the caller should do next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Fetch and store this, then report the outcome and ask again.
    Fetch(PrefetchItem),
    /// Something the user is waiting on is in flight. Come back later; the
    /// item stays queued.
    Yield,
    /// Nothing more will come out of this queue.
    Stop(StopReason),
}

/// Why a prefetch ended. Every variant is an ordinary outcome — none is an
/// error, and none is surfaced to the user.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StopReason {
    /// Everything the plan named has been offered. The happy ending.
    PlanExhausted,
    /// This visit's speculative allowance is spent (FR-069).
    VisitBudget,
    /// The user opened a different world (FR-073).
    WorldChanged,
    /// A newer plan or a live update arrived, so this queue's entitlement is
    /// stale (FR-070, FR-072).
    PlanSuperseded,
    /// The store is full. Speculation stops rather than evicting (FR-071).
    NoRoom,
    /// The budget pass found no room for any write at all (FR-024).
    StoreClosed,
}

/// A world's unfetched plan items, handed out one at a time.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrefetchQueue {
    world_id: Uuid,
    plan_epoch: u64,
    queued: VecDeque<PrefetchItem>,
    spent_bytes: u64,
    visit_budget_bytes: u64,
}

impl PrefetchQueue {
    /// Build the queue for one world from the plan that world's sync
    /// returned (FR-069, FR-072, FR-073).
    ///
    /// The only constructor, and it reads only `plan.fetch`. An item the
    /// server did not put there — one this caller may not see, one they
    /// already hold, one belonging to another world — has no route into the
    /// queue, so "prefetch only requests what the plan named" is a property
    /// of the type rather than of the loop that drains it.
    ///
    /// `plan.evict` is ignored on purpose: it names content to *release*, and
    /// a prefetcher that read it would be fetching what it was just told to
    /// throw away.
    ///
    /// Scene state is skipped. It has no byte route of its own — it arrives
    /// through the existing GraphQL scene load — so there is nothing here to
    /// fetch, and counting its bytes against the visit budget would spend the
    /// allowance on work this module never performs.
    pub fn from_plan(world_id: Uuid, plan_epoch: u64, plan: &SyncPlan) -> Self {
        let queued = plan
            .fetch
            .iter()
            .filter_map(|item| match item.id {
                ItemId::CanvasAsset(asset_id) => Some(PrefetchItem {
                    asset_id,
                    fingerprint: item.fingerprint,
                    byte_size: item.byte_size,
                }),
                ItemId::SceneState(_) => None,
            })
            .collect();
        Self {
            world_id,
            plan_epoch,
            queued,
            spent_bytes: 0,
            visit_budget_bytes: VISIT_BUDGET_BYTES,
        }
    }

    /// Override this visit's allowance. Tests use it to reach the budget in
    /// a few small items instead of 64MB of them.
    #[must_use]
    pub fn with_visit_budget(mut self, bytes: u64) -> Self {
        self.visit_budget_bytes = bytes;
        self
    }

    /// The world this queue is entitled to fetch into.
    pub fn world_id(&self) -> Uuid {
        self.world_id
    }

    /// Items still waiting. Zero means the plan was fully offered, not that
    /// everything in it was stored — a fetch may fail, and its item is not
    /// retried this visit.
    pub fn remaining(&self) -> usize {
        self.queued.len()
    }

    /// Speculative bytes stored so far this visit.
    pub fn spent_bytes(&self) -> u64 {
        self.spent_bytes
    }

    /// Decide the next move, and dequeue the item if it is a [`Step::Fetch`].
    ///
    /// The checks are ordered by what outranks what, and the three that end
    /// the queue outright come first: there is no point yielding politely to
    /// a demand fetch in a world the user has already left.
    ///
    /// [`Step::Yield`] deliberately dequeues nothing. Yielding is not
    /// dropping — the item is still worth fetching, just not while somebody
    /// is waiting on something else (FR-070).
    pub fn step(&mut self, pressure: &Pressure) -> Step {
        if pressure.open_world != self.world_id {
            return Step::Stop(StopReason::WorldChanged);
        }
        if pressure.plan_epoch != self.plan_epoch {
            return Step::Stop(StopReason::PlanSuperseded);
        }
        if !pressure.may_store {
            return Step::Stop(StopReason::StoreClosed);
        }
        // FR-070. Checked before the queue is even looked at, so a busy tab
        // yields at zero cost rather than after deciding what it would have
        // fetched.
        if pressure.demand_in_flight > 0 {
            return Step::Yield;
        }

        let Some(item) = self.queued.front().copied() else {
            return Step::Stop(StopReason::PlanExhausted);
        };

        // Both remaining rules stop rather than skip. Skipping to a smaller
        // item that happens to fit would keep a full store churning and would
        // make the visit allowance a suggestion; ending the prefetch leaves
        // the rest to demand, which is the fallback this whole module is
        // allowed to degrade to.
        if self.spent_bytes.saturating_add(item.byte_size) > self.visit_budget_bytes {
            return Step::Stop(StopReason::VisitBudget);
        }
        // FR-071, and the one check that must never turn into an eviction.
        if budget::admit_speculative(pressure.in_use_bytes, pressure.limit_bytes, item.byte_size)
            == Speculation::Stop
        {
            return Step::Stop(StopReason::NoRoom);
        }

        self.queued.pop_front();
        Step::Fetch(item)
    }

    /// Count bytes actually written against this visit's allowance.
    ///
    /// Called with what was stored, not with what was planned: a fetch that
    /// failed, or content already on disk under this fingerprint, costs the
    /// allowance nothing, because the allowance exists to bound bandwidth and
    /// neither of those spent any.
    pub fn record_stored(&mut self, bytes: u64) {
        self.spent_bytes = self.spent_bytes.saturating_add(bytes);
    }
}
