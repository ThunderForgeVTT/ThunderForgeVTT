//! Spec 048 T076: `GET /api/sheet-imports/{versionId}/file`. The player who
//! brought the sheet and the GM where it was applied get it; anyone else gets
//! the same 404 a missing version does.

use axum::http::{StatusCode, header};

use super::*;
use crate::sheet_import::route::sheet_file;

async fn fetch(t: &Table, user: Uuid, version: Uuid) -> (StatusCode, Option<String>, Vec<u8>) {
    let response = sheet_file(&t.state, user, false, version).await;
    let status = response.status();
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, cache, body)
}

#[tokio::test]
async fn the_owner_and_the_gm_download_the_sheet_and_nobody_else_learns_it_is_there() {
    let t = table();
    let owner = t.claimant().await;
    let import = t.bring(owner, FIGHTER_WIZARD, &[]).await;
    let version = import.version_id.expect("an import has a version");

    for (who, user) in [("the owner", owner), ("the GM", t.gm)] {
        let (status, cache, body) = fetch(&t, user, version).await;
        assert_eq!(status, StatusCode::OK, "{who}");
        assert_eq!(cache.as_deref(), Some("private, no-store"), "{who}");
        assert_eq!(body, FIGHTER_WIZARD, "{who} gets the file as uploaded");
    }

    let trusted = t.member("TrustedPlayer");
    let (stranger, elsewhere_gm) = {
        let mut conn = t.state.db_pool.get().expect("conn");
        let stranger = insert_test_user(&mut conn);
        let gm = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, gm);
        insert_test_world_member(&mut conn, world, gm, "GM");
        (stranger, gm)
    };
    let missing = fetch(&t, owner, Uuid::now_v7()).await;
    assert_eq!(missing.0, StatusCode::NOT_FOUND);
    for (who, user) in [
        ("a Trusted Player", trusted),
        ("another world's GM", elsewhere_gm),
        ("a stranger", stranger),
    ] {
        let refused = fetch(&t, user, version).await;
        assert_eq!(refused.0, StatusCode::NOT_FOUND, "{who}");
        assert_eq!(refused.2, missing.2, "{who} learns nothing more");
    }
}
