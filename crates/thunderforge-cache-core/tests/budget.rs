//! Spec 028 T060: the budget rule, and the one it must never break.

use thunderforge_cache_core::ItemId;
use thunderforge_cache_core::budget::{
    IndexEntry, MAX_BUDGET_BYTES, Speculation, admit_speculative, limit_bytes, plan_eviction,
    speculative_headroom,
};
use uuid::Uuid;

fn world(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn entry(item: u128, w: u128, size: u64, seq: u64) -> IndexEntry {
    IndexEntry {
        id: ItemId::CanvasAsset(Uuid::from_u128(item)),
        world_id: world(w),
        byte_size: size,
        last_read_seq: seq,
    }
}

#[test]
fn budget_is_half_the_reported_quota() {
    assert_eq!(limit_bytes(1000), 500);
}

#[test]
fn budget_is_capped_regardless_of_quota() {
    // A 4TB workstation does not get a 2TB cache.
    assert_eq!(limit_bytes(u64::MAX), MAX_BUDGET_BYTES);
}

#[test]
fn budget_scales_across_orders_of_magnitude() {
    // The reason it is proportional rather than a shipped constant.
    assert_eq!(limit_bytes(2 * 1024 * 1024 * 1024), 1024 * 1024 * 1024);
    assert_eq!(limit_bytes(200 * 1024 * 1024), 100 * 1024 * 1024);
}

#[test]
fn nothing_is_evicted_when_it_already_fits() {
    let index = vec![entry(1, 1, 100, 0)];
    let plan = plan_eviction(&index, 1000, 100, world(1));
    assert!(plan.evict.is_empty());
    assert!(!plan.insufficient);
}

#[test]
fn least_recently_used_world_goes_first() {
    let index = vec![
        entry(1, 1, 500, 10), // open world — untouchable
        entry(2, 2, 500, 1),  // oldest
        entry(3, 3, 500, 5),
    ];
    // in_use 1500 + incoming 200 over a 1200 limit needs 500 freed, which
    // world 2 supplies exactly — so world 3 must be spared. Sizing it this
    // way is what makes the assertion about *ordering* rather than volume.
    let plan = plan_eviction(&index, 1200, 200, world(1));

    assert_eq!(
        plan.evict,
        vec![ItemId::CanvasAsset(Uuid::from_u128(2))],
        "the least recently used world goes first, and no more than needed"
    );
    assert!(!plan.insufficient);
}

#[test]
fn eviction_spills_to_the_next_world_when_one_is_not_enough() {
    let index = vec![
        entry(1, 1, 500, 10), // open world
        entry(2, 2, 500, 1),  // oldest
        entry(3, 3, 500, 5),
    ];
    // Needs 700 freed; world 2 supplies only 500, so world 3 follows.
    let plan = plan_eviction(&index, 1000, 200, world(1));

    let evicted: Vec<Uuid> = plan.evict.iter().map(ItemId::uuid).collect();
    assert_eq!(evicted, vec![Uuid::from_u128(2), Uuid::from_u128(3)]);
    assert!(!plan.insufficient);
}

#[test]
fn the_open_world_is_never_evicted_even_under_pressure() {
    // FR-023. The user is looking at this content; evicting it would be
    // visibly wrong in exactly the moment it matters most.
    let index = vec![entry(1, 1, 900, 0), entry(2, 1, 900, 1)];
    let plan = plan_eviction(&index, 1000, 500, world(1));

    assert!(
        plan.evict.is_empty(),
        "nothing outside the open world exists to release"
    );
    assert!(
        plan.insufficient,
        "must report honestly that it cannot fit rather than break the rule"
    );
}

#[test]
fn whole_worlds_are_released_before_individual_items() {
    // A half-cached world is the worst outcome available: slow *and*
    // occupying space.
    let index = vec![
        entry(1, 1, 100, 9),
        entry(2, 2, 100, 1),
        entry(3, 2, 100, 2),
        entry(4, 2, 100, 3),
    ];
    let plan = plan_eviction(&index, 350, 100, world(1));

    let evicted: Vec<Uuid> = plan.evict.iter().map(ItemId::uuid).collect();
    assert_eq!(evicted.len(), 3, "world 2 released entire");
    for n in 2..=4u128 {
        assert!(evicted.contains(&Uuid::from_u128(n)));
    }
}

#[test]
fn eviction_plan_is_deterministic() {
    // Ties break by world then item id, so the same index always yields the
    // same plan and this test can assert on it exactly.
    let index = vec![
        entry(1, 1, 100, 0),
        entry(2, 2, 100, 5),
        entry(3, 3, 100, 5),
    ];
    let a = plan_eviction(&index, 150, 100, world(1));
    let b = plan_eviction(&index, 150, 100, world(1));
    assert_eq!(a, b);
}

// --- T118 / FR-071: speculation stops, it never evicts -------------------

#[test]
fn speculative_content_is_admitted_only_into_room_that_is_already_spare() {
    // 600 in use of 1000 leaves 400. Exactly 400 fits; one byte more does not.
    assert_eq!(admit_speculative(600, 1000, 400), Speculation::Admit);
    assert_eq!(admit_speculative(600, 1000, 401), Speculation::Stop);
    assert_eq!(speculative_headroom(600, 1000), 400);
}

#[test]
fn prefetching_stops_rather_than_evicting_a_world_it_could_have_freed() {
    // The sharp one (FR-071). This index is *full* of releasable content: two
    // cold worlds, neither of them open, exactly what `plan_eviction` exists
    // to reclaim. A demand fetch of the same size would be admitted by
    // releasing world 3 — and the speculative answer must still be Stop.
    let index = vec![
        entry(1, 1, 100, 9), // open world
        entry(2, 2, 400, 1), // cold, releasable
        entry(3, 3, 400, 2), // cold, releasable
    ];
    let in_use: u64 = index.iter().map(|e| e.byte_size).sum();
    let limit = 1000;
    let incoming = 300;

    let demand = plan_eviction(&index, limit, incoming, world(1));
    assert!(
        !demand.evict.is_empty(),
        "precondition: for a demand fetch this index has plenty to release"
    );

    assert_eq!(
        admit_speculative(in_use, limit, incoming),
        Speculation::Stop,
        "speculative content must never displace content the user actually has"
    );
}

#[test]
fn a_refused_storage_estimate_stops_the_prefetch() {
    // `enforce_budget` leaves `limit_bytes` at zero when the platform will
    // not estimate, and evicts nothing. Speculation reads that same zero as
    // Stop: room that cannot be demonstrated is not room. Demand loads are
    // unaffected — they never consult this function.
    assert_eq!(admit_speculative(0, 0, 1), Speculation::Stop);
    assert_eq!(speculative_headroom(0, 0), 0);
}

#[test]
fn a_store_already_over_its_limit_admits_nothing_speculative() {
    // The quota shrank under a store that was legitimately filled. Eviction
    // is the pass that fixes that; a prefetch must not add to it meanwhile,
    // and must not report negative headroom by underflowing.
    assert_eq!(admit_speculative(2000, 1000, 1), Speculation::Stop);
    assert_eq!(speculative_headroom(2000, 1000), 0);
}

#[test]
fn a_zero_byte_speculative_item_never_overflows_the_check() {
    // Sizes come off the server's plan, so the arithmetic must survive one
    // that is absurd rather than wrap into an accidental Admit.
    assert_eq!(admit_speculative(u64::MAX, u64::MAX, 0), Speculation::Admit);
    assert_eq!(admit_speculative(u64::MAX, u64::MAX, 1), Speculation::Stop);
}

#[test]
fn what_an_eviction_frees_is_admitted_afterwards() {
    // The bug this guards, end to end in arithmetic: a pass evicts a world to
    // make room for the open one, and the prefetch that would store the open
    // world's art is then told how full the store was *before* the eviction.
    // `admit_speculative` refuses on that figure, the queue stops with
    // NoRoom, and the open world's art is never written — by the very pass
    // performed to fit it.
    //
    // Sized from the real failure: two ~179KB worlds against a 250,000-byte
    // limit, one of which must go.
    let held = 179_424;
    let incoming = 179_342;
    let limit = 250_000;

    let index = vec![entry(1, 1, held, 0)];
    let plan = plan_eviction(&index, limit, incoming, world(2));
    assert_eq!(plan.evict.len(), 1, "the idle world should be released");
    assert!(!plan.insufficient, "releasing it leaves room for the incoming");

    // Before the pass — what the old code handed the prefetch.
    assert_eq!(
        admit_speculative(plan.in_use_bytes, limit, incoming),
        Speculation::Stop,
        "pre-eviction occupancy refuses the very bytes the eviction freed",
    );

    // After it, which is what the store actually holds.
    let freed: u64 = index.iter().map(|e| e.byte_size).sum();
    let occupied = plan.in_use_bytes - freed;
    assert_eq!(
        admit_speculative(occupied, limit, incoming),
        Speculation::Admit,
        "the open world's own art must fit in the room just made for it",
    );
    assert_eq!(speculative_headroom(occupied, limit), limit);
}
