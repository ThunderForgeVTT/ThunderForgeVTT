//! Spec 028 T116/T117: what a prefetch is allowed to want, and what it must
//! get out of the way of.
//!
//! The queue performs no I/O, so all of it is exercised here: entitlement
//! (only the caller's own plan, only the open world), precedence (demand work
//! and live updates outrank speculation), and the rule that speculation stops
//! rather than evicting.

use thunderforge_cache_browser::prefetch::{
    PrefetchQueue, Pressure, Step, StopReason, VISIT_BUDGET_BYTES,
};
use thunderforge_cache_core::delta::{PlanItem, SyncPlan};
use thunderforge_cache_core::{Fingerprint, ItemId};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn fingerprint(n: u8) -> Fingerprint {
    Fingerprint::of_bytes(&[n])
}

fn asset(n: u128, size: u64) -> PlanItem {
    PlanItem {
        id: ItemId::CanvasAsset(id(n)),
        fingerprint: fingerprint(n as u8),
        byte_size: size,
    }
}

fn scene(n: u128, size: u64) -> PlanItem {
    PlanItem {
        id: ItemId::SceneState(id(n)),
        fingerprint: fingerprint(n as u8),
        byte_size: size,
    }
}

/// A tab with room to spare and nothing the user is waiting on: every rule
/// off, so that a test which flips one is testing only that one.
fn calm(world: Uuid) -> Pressure {
    Pressure {
        open_world: world,
        plan_epoch: 1,
        demand_in_flight: 0,
        in_use_bytes: 0,
        limit_bytes: u64::MAX / 2,
        may_store: true,
    }
}

/// Drain a queue under unchanging pressure, collecting what it asked for.
fn drain(queue: &mut PrefetchQueue, pressure: &Pressure) -> (Vec<Uuid>, StopReason) {
    let mut fetched = Vec::new();
    loop {
        match queue.step(pressure) {
            Step::Fetch(item) => {
                queue.record_stored(item.byte_size);
                fetched.push(item.asset_id);
            }
            Step::Yield => panic!("nothing in this fixture should cause a yield"),
            Step::Stop(reason) => return (fetched, reason),
        }
    }
}

// --- FR-072: the plan is the entitlement --------------------------------

#[test]
fn prefetch_only_ever_asks_for_something_the_server_put_in_the_plan() {
    // FR-072, and the reason `from_plan` is the only constructor: the plan
    // was computed against this caller's permissions, so anything the queue
    // can name is something the server already agreed to serve them. There is
    // no method that adds an item, so this cannot be violated by a caller
    // either.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10), asset(2, 20), asset(3, 30)],
        evict: vec![ItemId::CanvasAsset(id(99))],
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let (fetched, reason) = drain(&mut queue, &calm(world));

    assert_eq!(fetched, vec![id(1), id(2), id(3)]);
    assert_eq!(reason, StopReason::PlanExhausted);
    for asked in &fetched {
        assert!(
            plan.fetch
                .iter()
                .any(|item| item.id == ItemId::CanvasAsset(*asked)),
            "{asked} was requested but the plan never named it"
        );
    }
}

#[test]
fn content_the_plan_told_us_to_release_is_never_prefetched() {
    // A prefetcher that read `evict` would spend bandwidth fetching exactly
    // what it had just been instructed to throw away — including content the
    // server withdrew because this caller is no longer permitted to see it
    // (FR-015).
    let world = id(7);
    let plan = SyncPlan {
        fetch: Vec::new(),
        evict: vec![ItemId::CanvasAsset(id(1)), ItemId::SceneState(id(2))],
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    assert_eq!(queue.remaining(), 0);
    assert_eq!(
        queue.step(&calm(world)),
        Step::Stop(StopReason::PlanExhausted)
    );
}

#[test]
fn an_unchanged_world_prefetches_nothing_at_all() {
    // Silence in a plan means "current" — the win the whole feature is built
    // on. A prefetch must not undo it by refetching what is already held.
    let world = id(7);
    let mut queue = PrefetchQueue::from_plan(world, 1, &SyncPlan::default());

    assert_eq!(queue.remaining(), 0);
    assert_eq!(
        queue.step(&calm(world)),
        Step::Stop(StopReason::PlanExhausted)
    );
}

#[test]
fn scene_state_is_skipped_and_costs_the_allowance_nothing() {
    // Scene state has no byte route of its own; it arrives with the GraphQL
    // scene load. Queueing it would spend a visit's allowance on bytes this
    // module never fetches, quietly starving the assets that do need it.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![scene(1, 900), asset(2, 100), scene(3, 900)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan).with_visit_budget(200);

    let (fetched, reason) = drain(&mut queue, &calm(world));

    assert_eq!(fetched, vec![id(2)], "only the canvas asset is fetchable");
    assert_eq!(reason, StopReason::PlanExhausted);
    assert_eq!(queue.spent_bytes(), 100, "the scenes spent nothing");
}

// --- FR-073: confined to the open world ---------------------------------

#[test]
fn switching_worlds_stops_a_prefetch_that_is_already_running() {
    // FR-073. The task outlives the visit: it was spawned for world 7 and is
    // still awaiting a fetch when the user opens world 8. Without this check
    // it would keep pulling world 7's content down the wire — and filling the
    // budget — for a world nobody is looking at.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10), asset(2, 20)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    assert!(matches!(queue.step(&calm(world)), Step::Fetch(_)));

    let elsewhere = Pressure {
        open_world: id(8),
        ..calm(world)
    };
    assert_eq!(queue.step(&elsewhere), Step::Stop(StopReason::WorldChanged));
    assert_eq!(
        queue.step(&elsewhere),
        Step::Stop(StopReason::WorldChanged),
        "and it stays stopped rather than resuming on the next poll"
    );
}

#[test]
fn returning_to_the_world_does_not_revive_a_stale_queue() {
    // The queue is not merely paused while away. Coming back re-syncs and
    // builds a fresh queue from a fresh plan; this one's entitlement is from
    // before the trip and must not be reused. Belt and braces with the epoch
    // check below, because a world switch that happens to return before the
    // next poll would otherwise slip through on the world id alone.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let back_again = Pressure {
        plan_epoch: 2,
        ..calm(world)
    };
    assert_eq!(
        queue.step(&back_again),
        Step::Stop(StopReason::PlanSuperseded)
    );
}

// --- FR-070: everything the user is doing outranks this -----------------

#[test]
fn a_demand_fetch_yields_the_prefetch_without_dropping_its_work() {
    // FR-070. Yielding is not dropping: the item is still worth having, just
    // not while the user is waiting on something else. A queue that discarded
    // on yield would leave a busy tab with a permanently cold cache.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let busy = Pressure {
        demand_in_flight: 1,
        ..calm(world)
    };
    assert_eq!(queue.step(&busy), Step::Yield);
    assert_eq!(queue.remaining(), 1, "the item is still queued");

    match queue.step(&calm(world)) {
        Step::Fetch(item) => assert_eq!(
            item.asset_id,
            id(1),
            "the same item comes back once the tab is quiet"
        ),
        other => panic!("expected the yielded item to be offered again, got {other:?}"),
    }
}

#[test]
fn a_yield_costs_the_visit_allowance_nothing() {
    // Otherwise a tab busy enough to yield repeatedly would burn its
    // speculative allowance on work it never did.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);
    let busy = Pressure {
        demand_in_flight: 3,
        ..calm(world)
    };

    for _ in 0..100 {
        assert_eq!(queue.step(&busy), Step::Yield);
    }
    assert_eq!(queue.spent_bytes(), 0);
    assert_eq!(queue.remaining(), 1);
}

#[test]
fn a_live_update_supersedes_queued_speculative_work() {
    // FR-070/FR-072. A live fingerprint change means the plan this queue was
    // built from is no longer what the server would answer. Continuing would
    // fetch a superseded fingerprint — bandwidth spent on content that is
    // already stale before it lands.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10), asset(2, 20)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);
    assert!(matches!(queue.step(&calm(world)), Step::Fetch(_)));

    let after_update = Pressure {
        plan_epoch: 2,
        ..calm(world)
    };
    assert_eq!(
        queue.step(&after_update),
        Step::Stop(StopReason::PlanSuperseded)
    );
}

#[test]
fn a_world_switch_outranks_a_yield() {
    // Ordering, not merely coverage: there is no point politely yielding to a
    // demand fetch in a world the user has already left. The queue must end,
    // not wait for a quiet moment that would let it resume in the wrong world.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 10)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let both = Pressure {
        open_world: id(8),
        demand_in_flight: 1,
        ..calm(world)
    };
    assert_eq!(queue.step(&both), Step::Stop(StopReason::WorldChanged));
}

// --- FR-071 / FR-024: it stops, it never displaces ----------------------

#[test]
fn prefetching_stops_rather_than_evicting_when_the_budget_is_reached() {
    // FR-071, the sharp rule. The store has 50 bytes spare and the next item
    // needs 100. There is plenty that *could* be released to make room — that
    // is what `plan_eviction` is for — and speculation may not ask for it.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 100)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let nearly_full = Pressure {
        in_use_bytes: 950,
        limit_bytes: 1000,
        ..calm(world)
    };
    assert_eq!(queue.step(&nearly_full), Step::Stop(StopReason::NoRoom));
    assert_eq!(
        queue.remaining(),
        1,
        "and it took nothing off the queue on the way out"
    );
}

#[test]
fn a_full_store_stops_the_prefetch_before_it_fetches_anything() {
    // FR-024. When the budget pass found no room for writes at all, demand
    // loads still work: they fetch and deliver without filing. Prefetching
    // into that state is pure waste — bytes over the wire that cannot land.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 1)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let closed = Pressure {
        may_store: false,
        ..calm(world)
    };
    assert_eq!(queue.step(&closed), Step::Stop(StopReason::StoreClosed));
}

#[test]
fn a_refused_storage_estimate_stops_the_prefetch() {
    // The platform declining to estimate arrives here as a zero limit, and
    // the budget pass evicts nothing on it. Speculation reads the same zero
    // as "no demonstrated room" and stops. Room we cannot prove is not room.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 1)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);

    let unknown_quota = Pressure {
        limit_bytes: 0,
        ..calm(world)
    };
    assert_eq!(queue.step(&unknown_quota), Step::Stop(StopReason::NoRoom));
}

// --- FR-069: bounded to one visit's worth -------------------------------

#[test]
fn the_visit_allowance_bounds_how_far_ahead_of_demand_one_open_runs() {
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 60), asset(2, 60), asset(3, 60)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan).with_visit_budget(150);

    let (fetched, reason) = drain(&mut queue, &calm(world));

    assert_eq!(fetched, vec![id(1), id(2)]);
    assert_eq!(reason, StopReason::VisitBudget);
    assert_eq!(
        queue.remaining(),
        1,
        "the remainder is left to the demand path, not discarded twice"
    );
}

#[test]
fn the_allowance_stops_the_prefetch_rather_than_skipping_to_a_smaller_item() {
    // Skipping would make the allowance a suggestion — a world of small
    // assets behind one large one would drain indefinitely — and would fetch
    // items out of the order the plan named them.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 500), asset(2, 1)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan).with_visit_budget(100);

    let (fetched, reason) = drain(&mut queue, &calm(world));

    assert!(fetched.is_empty());
    assert_eq!(reason, StopReason::VisitBudget);
}

#[test]
fn only_bytes_actually_stored_spend_the_allowance() {
    // The caller reports what it stored, not what it planned: a failed fetch
    // and content already on disk under this fingerprint both cost zero
    // bandwidth, so neither may cost the allowance.
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, 100), asset(2, 100)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan).with_visit_budget(100);

    // First item: fetched, then found already present — nothing recorded.
    assert!(matches!(queue.step(&calm(world)), Step::Fetch(_)));
    // Second item still fits, because the first spent nothing.
    assert!(matches!(queue.step(&calm(world)), Step::Fetch(_)));
    assert_eq!(queue.spent_bytes(), 0);
}

#[test]
fn the_shipped_visit_allowance_is_the_documented_one() {
    // A regression guard on the number itself: it is the difference between
    // warming a world and saturating the connection the active scene is
    // loading over (SC-024).
    assert_eq!(VISIT_BUDGET_BYTES, 64 * 1024 * 1024);
    let world = id(7);
    let plan = SyncPlan {
        fetch: vec![asset(1, VISIT_BUDGET_BYTES + 1)],
        evict: Vec::new(),
    };
    let mut queue = PrefetchQueue::from_plan(world, 1, &plan);
    assert_eq!(
        queue.step(&calm(world)),
        Step::Stop(StopReason::VisitBudget)
    );
}
