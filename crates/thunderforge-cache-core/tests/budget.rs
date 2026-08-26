//! Spec 028 T060: the budget rule, and the one it must never break.

use thunderforge_cache_core::ItemId;
use thunderforge_cache_core::budget::{IndexEntry, MAX_BUDGET_BYTES, limit_bytes, plan_eviction};
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
