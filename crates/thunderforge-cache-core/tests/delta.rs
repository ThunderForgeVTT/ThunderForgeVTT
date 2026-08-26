//! Spec 028 T021: every branch of the plan computation from
//! contracts/graphql-delta-sync.md.

use std::collections::BTreeMap;

use thunderforge_cache_core::delta::{CurrentItem, compute_plan};
use thunderforge_cache_core::{Fingerprint, HeldItem, ItemId};
use uuid::Uuid;

fn asset(n: u128) -> ItemId {
    ItemId::CanvasAsset(Uuid::from_u128(n))
}

fn fp(s: &str) -> Fingerprint {
    Fingerprint::of_bytes(s.as_bytes())
}

fn current(f: Option<Fingerprint>) -> CurrentItem {
    CurrentItem {
        fingerprint: f,
        byte_size: 1024,
    }
}

#[test]
fn matching_fingerprint_is_omitted_entirely() {
    // The win: an unchanged world says nothing about anything.
    let mut server = BTreeMap::new();
    server.insert(asset(1), current(Some(fp("same"))));

    let plan = compute_plan(
        &[HeldItem {
            id: asset(1),
            fingerprint: fp("same"),
        }],
        &server,
    );

    assert!(plan.fetch.is_empty());
    assert!(plan.evict.is_empty());
}

#[test]
fn differing_fingerprint_is_fetched_with_the_current_value() {
    let mut server = BTreeMap::new();
    server.insert(asset(1), current(Some(fp("new"))));

    let plan = compute_plan(
        &[HeldItem {
            id: asset(1),
            fingerprint: fp("old"),
        }],
        &server,
    );

    assert_eq!(plan.fetch.len(), 1);
    assert_eq!(plan.fetch[0].fingerprint, fp("new"));
    assert!(plan.evict.is_empty());
}

#[test]
fn null_server_fingerprint_is_fetched_never_assumed_unchanged() {
    // An un-backfilled row. Guessing "unchanged" would serve stale content
    // indefinitely, so the safe default is the expensive one.
    let mut server = BTreeMap::new();
    server.insert(asset(1), current(None));

    let plan = compute_plan(
        &[HeldItem {
            id: asset(1),
            fingerprint: fp("whatever"),
        }],
        &server,
    );

    assert_eq!(plan.fetch.len(), 1, "NULL must mean fetch");
}

#[test]
fn item_no_longer_present_is_evicted() {
    let plan = compute_plan(
        &[HeldItem {
            id: asset(1),
            fingerprint: fp("held"),
        }],
        &BTreeMap::new(),
    );

    assert_eq!(plan.evict, vec![asset(1)]);
    assert!(plan.fetch.is_empty());
}

#[test]
fn revoked_and_deleted_are_indistinguishable_to_the_client() {
    // Both arrive as an absence from `authorized_current`, so both land in
    // `evict`. That is deliberate: the client cannot tell whether the item
    // is gone or merely forbidden, which discloses nothing about its
    // existence, and cache correctness and FR-015 come out of one mechanism.
    let deleted = compute_plan(
        &[HeldItem {
            id: asset(1),
            fingerprint: fp("x"),
        }],
        &BTreeMap::new(),
    );
    let revoked = compute_plan(
        &[HeldItem {
            id: asset(1),
            fingerprint: fp("x"),
        }],
        &BTreeMap::new(),
    );
    assert_eq!(deleted, revoked);
}

#[test]
fn a_claim_on_an_unauthorized_item_reveals_nothing() {
    // The client claims asset 99, which it may not see, so the server never
    // put it in `authorized_current`. It must not appear in `fetch` — that
    // would hand over content — and its presence in `evict` is the same
    // signal a deleted item produces, so nothing is learned either way.
    let mut server = BTreeMap::new();
    server.insert(asset(1), current(Some(fp("ok"))));

    let plan = compute_plan(
        &[
            HeldItem {
                id: asset(1),
                fingerprint: fp("ok"),
            },
            HeldItem {
                id: asset(99),
                fingerprint: fp("secret"),
            },
        ],
        &server,
    );

    assert!(
        plan.fetch.is_empty(),
        "an unauthorized item must never be offered for fetch"
    );
    assert_eq!(plan.evict, vec![asset(99)]);
}

#[test]
fn empty_manifest_yields_the_full_plan() {
    let mut server = BTreeMap::new();
    server.insert(asset(1), current(Some(fp("a"))));
    server.insert(asset(2), current(Some(fp("b"))));

    let plan = compute_plan(&[], &server);

    assert_eq!(plan.fetch.len(), 2, "cold start fetches everything");
    assert!(plan.evict.is_empty());
}

#[test]
fn plan_is_deterministic() {
    let mut server = BTreeMap::new();
    for n in 1..=5 {
        server.insert(asset(n), current(Some(fp("v"))));
    }
    let held: Vec<HeldItem> = (1..=5)
        .map(|n| HeldItem {
            id: asset(n),
            fingerprint: fp("old"),
        })
        .collect();

    assert_eq!(compute_plan(&held, &server), compute_plan(&held, &server));
}
