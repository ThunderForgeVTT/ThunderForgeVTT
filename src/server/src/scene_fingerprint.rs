//! Keeping a scene's content fingerprint current.
//!
//! Spec 028 FR-005/FR-006, ADR-052. The server is the authority on what a
//! scene's fingerprint *is*; clients only ever verify against it.
//!
//! The hash itself is computed by `thunderforge_cache_core`, shared with the
//! engine, so the two sides cannot disagree about what a given scene state
//! hashes to. This module's job is narrower: read the scene's rows, hand them
//! to the shared canonical form, and persist the result.

use chrono::Utc;
use diesel::prelude::*;
use thunderforge_cache_core::manifest::{CANONICAL_VERSION, CanonicalEntity, CanonicalSceneState};
use uuid::Uuid;

use crate::schema::{scene_state_fingerprints, tokens};

/// Recompute and store the fingerprint for one scene.
///
/// Called after any change that alters what
/// [`CanonicalSceneState`] covers. Cheap enough to run inline: it is one
/// indexed read plus an upsert, and it must not lag the change that caused
/// it — a stale fingerprint would tell a client its copy is current when it
/// is not, which is the one failure this whole feature must never produce.
///
/// Best-effort by design. A failure here means clients re-fetch a scene they
/// might not have needed to, which is slow rather than wrong, so it must
/// never fail the mutation that triggered it. The alternative — refusing a
/// GM's token move because a derived-data write failed — trades a correctness
/// non-issue for a real one.
pub fn recompute_scene_fingerprint(
    conn: &mut PgConnection,
    scene_id: Uuid,
    user_id: Uuid,
) -> Result<String, diesel::result::Error> {
    let rows: Vec<(Uuid, f64, f64, f64, f64)> = tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .select((
            tokens::token_id,
            tokens::x,
            tokens::y,
            tokens::rotation,
            tokens::scale,
        ))
        .load(conn)?;

    // Ordering is not requested here on purpose: `CanonicalSceneState::new`
    // sorts by id, so whatever order Postgres returns cannot reach the hash.
    // Relying on that rather than on an ORDER BY means the guarantee lives in
    // one tested place instead of at every call site.
    let entities: Vec<CanonicalEntity> = rows
        .into_iter()
        .map(|(id, x, y, rotation, scale)| CanonicalEntity {
            id,
            x_milli: CanonicalSceneState::quantize(x as f32),
            y_milli: CanonicalSceneState::quantize(y as f32),
            rotation_milli: CanonicalSceneState::quantize(rotation as f32),
            scale_milli: CanonicalSceneState::quantize(scale as f32),
        })
        .collect();

    let hash = CanonicalSceneState::new(scene_id, entities)
        .fingerprint()
        .to_hex();
    let now = Utc::now().naive_utc();

    diesel::insert_into(scene_state_fingerprints::table)
        .values((
            scene_state_fingerprints::scene_id.eq(scene_id),
            scene_state_fingerprints::content_hash.eq(&hash),
            scene_state_fingerprints::canonical_version.eq(CANONICAL_VERSION as i32),
            scene_state_fingerprints::computed_at.eq(now),
            scene_state_fingerprints::updated_by.eq(user_id),
        ))
        .on_conflict(scene_state_fingerprints::scene_id)
        .do_update()
        .set((
            scene_state_fingerprints::content_hash.eq(&hash),
            scene_state_fingerprints::canonical_version.eq(CANONICAL_VERSION as i32),
            scene_state_fingerprints::computed_at.eq(now),
            scene_state_fingerprints::updated_by.eq(user_id),
        ))
        .execute(conn)?;

    Ok(hash)
}

/// Recompute, swallowing failure.
///
/// The form call sites use, so a derived-data problem can never fail the
/// user's actual change. Logged rather than silent, because a fingerprint
/// that stops updating would otherwise present much later as a cache that
/// mysteriously never hits.
pub fn refresh_scene_fingerprint(conn: &mut PgConnection, scene_id: Uuid, user_id: Uuid) {
    if let Err(e) = recompute_scene_fingerprint(conn, scene_id, user_id) {
        tracing::warn!(
            scene_id = %scene_id,
            error = %e,
            "failed to refresh scene fingerprint; clients will re-fetch this scene"
        );
    }
}
