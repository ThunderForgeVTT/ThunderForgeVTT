use super::*;

#[test]
fn recognises_canvas_asset_urls() {
    let parsed =
        parse_canvas_asset_path("/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp")
            .expect("canvas asset url");
    assert_eq!(parsed.extension, "webp");
    assert_eq!(
        parsed.asset_id,
        Uuid::parse_str("2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e").unwrap()
    );
}

/// Bevy asks for `<uuid>.webp.meta` before the image itself. That is not
/// a canvas asset, and routing it through the cache would look up a
/// fingerprint for an id that does not parse.
#[test]
fn rejects_meta_and_foreign_paths() {
    assert!(
        parse_canvas_asset_path(
            "/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp.meta"
        )
        .is_none()
    );
    assert!(parse_canvas_asset_path("/assets/tokens/goblin.png").is_none());
    assert!(parse_canvas_asset_path("/api/canvas-assets/not-a-uuid.webp").is_none());
    assert!(parse_canvas_asset_path("/api/canvas-assets/").is_none());
}

/// An unconfigured cache must behave exactly like no cache at all.
#[test]
fn unconfigured_cache_never_intercepts() {
    let mut cache = CanvasAssetCache::default();
    let mut images = Assets::<Image>::default();
    assert!(!cache.is_ready());
    assert!(
        try_cached(
            &mut cache,
            "/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp",
            &mut images,
        )
        .is_none()
    );
}

fn asset_url(id: Uuid) -> String {
    format!("/api/canvas-assets/{id}.webp")
}

/// A ready cache with a promise does intercept — the positive control
/// for the three fall-through tests around it, so "returns `None`"
/// cannot be passing for the wrong reason.
#[test]
fn a_promised_asset_is_intercepted() {
    let asset = Uuid::from_u128(3);
    let mut cache = CanvasAssetCache {
        readiness: Readiness::Ready,
        world_id: Some(Uuid::nil()),
        ..default()
    };
    apply_control(
        &mut cache,
        Control::Fingerprints(vec![(asset, Fingerprint::of_bytes(b"art"))]),
    );
    let mut images = Assets::<Image>::default();
    assert!(try_cached(&mut cache, &asset_url(asset), &mut images).is_some());
}

/// The eviction case, which is what a sync's `evict` list turns into
/// locally. Once the authoritative set no longer names an asset, the
/// read path must stop touching it entirely — otherwise the next load
/// re-fetches and re-stores it, quietly undoing the eviction (FR-015).
#[test]
fn a_replaced_fingerprint_set_stops_an_evicted_asset_being_cached() {
    let kept = Uuid::from_u128(1);
    let evicted = Uuid::from_u128(2);
    let mut cache = CanvasAssetCache {
        readiness: Readiness::Ready,
        world_id: Some(Uuid::nil()),
        ..default()
    };
    apply_control(
        &mut cache,
        Control::Fingerprints(vec![
            (kept, Fingerprint::of_bytes(b"kept")),
            (evicted, Fingerprint::of_bytes(b"gone")),
        ]),
    );
    let mut images = Assets::<Image>::default();
    assert!(try_cached(&mut cache, &asset_url(evicted), &mut images).is_some());

    // A sync answers with `kept` only.
    apply_control(
        &mut cache,
        Control::ReplaceFingerprints(vec![(kept, Fingerprint::of_bytes(b"kept"))]),
    );
    assert!(try_cached(&mut cache, &asset_url(evicted), &mut images).is_none());
    assert!(try_cached(&mut cache, &asset_url(kept), &mut images).is_some());
}

/// A failed sync must leave the read path exactly where an unconfigured
/// one is: not intercepting. Configuring costs nothing on its own —
/// readiness only arrives when the backing store actually opens — so a
/// sync that dies before that point degrades to plain network loads.
#[test]
fn configuring_without_a_backing_store_does_not_intercept() {
    let asset = Uuid::from_u128(4);
    let mut cache = CanvasAssetCache::default();
    apply_control(
        &mut cache,
        Control::Configure {
            scope: "0123456789abcdef".to_owned(),
            world_id: Uuid::nil(),
        },
    );
    apply_control(
        &mut cache,
        Control::Fingerprints(vec![(asset, Fingerprint::of_bytes(b"art"))]),
    );
    let mut images = Assets::<Image>::default();
    assert!(!cache.is_ready());
    assert!(try_cached(&mut cache, &asset_url(asset), &mut images).is_none());

    // And a browser that cannot support the cache stays that way even
    // with everything else in place.
    apply_control(&mut cache, Control::Readiness(false));
    assert!(!cache.is_ready());
    assert!(try_cached(&mut cache, &asset_url(asset), &mut images).is_none());
}

/// A different user must not inherit the previous one's promises: the
/// scope is a directory, and a promise is the only thing that makes the
/// read path look in it.
#[test]
fn a_change_of_user_discards_every_promise() {
    let asset = Uuid::from_u128(5);
    let mut cache = CanvasAssetCache {
        readiness: Readiness::Ready,
        scope: Some("aaaa".to_owned()),
        world_id: Some(Uuid::nil()),
        ..default()
    };
    apply_control(
        &mut cache,
        Control::Fingerprints(vec![(asset, Fingerprint::of_bytes(b"art"))]),
    );
    apply_control(
        &mut cache,
        Control::Configure {
            scope: "bbbb".to_owned(),
            world_id: Uuid::nil(),
        },
    );
    let mut images = Assets::<Image>::default();
    assert!(cache.fingerprints.is_empty());
    assert!(try_cached(&mut cache, &asset_url(asset), &mut images).is_none());
}

/// Ready, but with no promise for this asset: still a plain fetch.
#[test]
fn unknown_fingerprint_falls_through() {
    let mut cache = CanvasAssetCache {
        readiness: Readiness::Ready,
        world_id: Some(Uuid::nil()),
        ..default()
    };
    let mut images = Assets::<Image>::default();
    assert!(cache.is_ready());
    assert!(
        try_cached(
            &mut cache,
            "/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp",
            &mut images,
        )
        .is_none()
    );
}
/// Sign-out (FR-016a). The read path must stop intercepting the moment
/// the session ends, without waiting on anything slow: the key discard
/// and the byte reclamation both happen off in the wasm module, and
/// neither is allowed to be what stops a departed user's content being
/// served out of this process.
#[test]
fn forgetting_stops_the_read_path_immediately() {
    let asset = Uuid::from_u128(9);
    let mut cache = CanvasAssetCache {
        readiness: Readiness::Ready,
        world_id: Some(Uuid::nil()),
        scope: Some("0123456789abcdef".to_owned()),
        ..default()
    };
    apply_control(
        &mut cache,
        Control::Fingerprints(vec![(asset, Fingerprint::of_bytes(b"art"))]),
    );
    let mut images = Assets::<Image>::default();
    assert!(try_cached(&mut cache, &asset_url(asset), &mut images).is_some());

    apply_control(&mut cache, Control::Forget);

    assert!(!cache.is_ready());
    assert!(cache.scope.is_none());
    assert!(cache.world_id.is_none());
    assert!(cache.fingerprints.is_empty());
    assert!(try_cached(&mut cache, &asset_url(asset), &mut images).is_none());
}

/// Signing back in must reopen the store rather than assume the handles
/// the previous session left behind are still good — they were dropped,
/// and the key they held was discarded. Clearing the scope is what makes
/// the identical-scope `Configure` take the reopen branch.
#[test]
fn signing_back_in_reopens_the_backing_store() {
    let scope = "0123456789abcdef".to_owned();
    let mut cache = CanvasAssetCache::default();
    apply_control(
        &mut cache,
        Control::Configure {
            scope: scope.clone(),
            world_id: Uuid::nil(),
        },
    );
    assert_eq!(cache.readiness, Readiness::Opening);

    apply_control(&mut cache, Control::Forget);
    assert_eq!(cache.readiness, Readiness::Unconfigured);

    apply_control(
        &mut cache,
        Control::Configure {
            scope,
            world_id: Uuid::nil(),
        },
    );
    assert_eq!(cache.readiness, Readiness::Opening);
}
