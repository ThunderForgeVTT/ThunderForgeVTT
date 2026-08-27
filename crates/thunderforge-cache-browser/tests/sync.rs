//! Spec 028 T028: the delta-sync request and reply, exercised natively.
//!
//! These run under plain `cargo test` because the manifest build and the plan
//! parse are deliberately free of browser I/O — which is the whole reason the
//! contract's failure modes can be tested at all.

use thunderforge_cache_browser::index::IndexEntry;
use thunderforge_cache_browser::sync::{
    SyncError, manifest_for_world, parse_sync_plan, sync_request_body,
};
use thunderforge_cache_core::{Fingerprint, ItemId};
use uuid::Uuid;

fn fp(s: &str) -> Fingerprint {
    Fingerprint::of_bytes(s.as_bytes())
}

fn asset(n: u128) -> ItemId {
    ItemId::CanvasAsset(Uuid::from_u128(n))
}

fn entry(world: u128, f: &str) -> IndexEntry {
    IndexEntry::new(
        fp(f),
        1024,
        Uuid::from_u128(world),
        thunderforge_cache_browser::ReadSeq(1),
    )
}

#[test]
fn manifest_contains_only_the_requested_world() {
    // A manifest leaking another world's items would hand the server a claim
    // about content this request has nothing to do with.
    let entries = vec![
        (asset(1), entry(10, "a")),
        (asset(2), entry(20, "b")),
        (asset(3), entry(10, "c")),
    ];
    let manifest = manifest_for_world(Uuid::from_u128(10), &entries);

    assert_eq!(manifest.len(), 2);
    assert_eq!(manifest.get(&asset(1)), Some(fp("a")));
    assert_eq!(manifest.get(&asset(3)), Some(fp("c")));
    assert_eq!(manifest.get(&asset(2)), None);
}

#[test]
fn request_body_is_deterministic() {
    // Two clients in identical states must produce identical bytes, or the
    // wire format is nondeterministic and nothing about it is assertable.
    let entries = vec![(asset(2), entry(10, "b")), (asset(1), entry(10, "a"))];
    let a = sync_request_body(&manifest_for_world(Uuid::from_u128(10), &entries));

    let reversed = vec![(asset(1), entry(10, "a")), (asset(2), entry(10, "b"))];
    let b = sync_request_body(&manifest_for_world(Uuid::from_u128(10), &reversed));

    assert_eq!(a, b, "insertion order must not reach the wire");
}

#[test]
fn parses_a_well_formed_plan() {
    let body = format!(
        r#"{{"data":{{"worldSyncPlan":{{
             "fetch":[{{"id":"asset:{}","fingerprint":"{}","byteSize":42,"peerAvailable":false}}],
             "evict":["scene:{}"],
             "canonicalVersion":1}}}}}}"#,
        Uuid::from_u128(1),
        fp("new").to_hex(),
        Uuid::from_u128(2),
    );

    let outcome = parse_sync_plan(&body).expect("should parse");
    assert_eq!(outcome.canonical_version, 1);
    assert_eq!(outcome.plan.fetch.len(), 1);
    assert_eq!(outcome.plan.fetch[0].id, asset(1));
    assert_eq!(outcome.plan.fetch[0].fingerprint, fp("new"));
    assert_eq!(outcome.plan.fetch[0].byte_size, 42);
    assert_eq!(
        outcome.plan.evict,
        vec![ItemId::SceneState(Uuid::from_u128(2))]
    );
}

#[test]
fn graphql_errors_are_surfaced_not_swallowed() {
    let body = r#"{"errors":[{"message":"user is not a member of this world"}]}"#;
    match parse_sync_plan(body) {
        Err(SyncError::Server(msg)) => assert!(msg.contains("not a member")),
        other => panic!("expected a server error, got {other:?}"),
    }
}

#[test]
fn a_malformed_item_fails_the_whole_plan() {
    // Deliberately not "skip the bad entry and carry on". A plan that quietly
    // loses items leaves the client believing it is current when it is not —
    // the single failure this feature must never produce, and far worse than
    // refusing the sync and re-fetching.
    let body = format!(
        r#"{{"data":{{"worldSyncPlan":{{
             "fetch":[{{"id":"asset:{}","fingerprint":"not-a-hash","byteSize":1,"peerAvailable":false}}],
             "evict":[],"canonicalVersion":1}}}}}}"#,
        Uuid::from_u128(1)
    );
    assert!(matches!(
        parse_sync_plan(&body),
        Err(SyncError::Malformed(_))
    ));
}

#[test]
fn an_unknown_item_kind_is_rejected() {
    // A client and server disagreeing about what an id means is worse than
    // failing to parse it.
    let body = format!(
        r#"{{"data":{{"worldSyncPlan":{{"fetch":[],"evict":["compendium:{}"],"canonicalVersion":1}}}}}}"#,
        Uuid::from_u128(1)
    );
    assert!(matches!(
        parse_sync_plan(&body),
        Err(SyncError::Malformed(_))
    ));
}

#[test]
fn missing_canonical_version_is_malformed() {
    // The version is what invalidates stored scene fingerprints wholesale.
    // Defaulting it would silently accept fingerprints computed under rules
    // we can no longer identify.
    let body = r#"{"data":{"worldSyncPlan":{"fetch":[],"evict":[]}}}"#;
    assert!(matches!(
        parse_sync_plan(body),
        Err(SyncError::Malformed(_))
    ));
}

#[test]
fn empty_plan_means_everything_is_current() {
    let body = r#"{"data":{"worldSyncPlan":{"fetch":[],"evict":[],"canonicalVersion":1}}}"#;
    let outcome = parse_sync_plan(body).expect("should parse");
    assert!(outcome.plan.fetch.is_empty());
    assert!(outcome.plan.evict.is_empty());
}
