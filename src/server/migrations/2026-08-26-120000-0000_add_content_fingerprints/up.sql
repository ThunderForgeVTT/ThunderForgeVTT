-- Spec 028, ADR-052: content fingerprints, so a client can be told what it
-- already has rather than being sent all of it again.
--
-- Two kinds of thing get fingerprinted, and they are stored differently
-- because they change for different reasons: an asset's bytes are immutable
-- once written (replacing art means a new asset row), while a scene's state
-- changes constantly. So the asset hash rides on the asset row, and scene
-- hashes get their own table that can be rewritten without touching scenes.

-- The hash of an asset's STORED bytes — the WebP that `transcode_to_webp`
-- produced, not what was uploaded. Hashing the upload would produce a value
-- no client could ever verify, since the client never receives the original.
--
-- Nullable on purpose. Existing rows have no hash until the backfill reaches
-- them, and NULL is read by `compute_plan` as "client must fetch" rather than
-- "unchanged" — the safe reading is the expensive one, which lets this ship
-- ahead of the backfill instead of behind it.
ALTER TABLE canvas_image_assets
    ADD COLUMN content_hash TEXT NULL;

-- Peer transfer and cross-scene dedup both look content up BY hash rather
-- than by id, so this index is on the read path, not just for reporting.
CREATE INDEX canvas_image_assets_content_hash_idx
    ON canvas_image_assets (content_hash)
    WHERE content_hash IS NOT NULL;

-- Lowercase hex SHA-256, or nothing. Enforced here rather than trusted from
-- the application: a hash that is silently the wrong case or length would
-- make every client re-fetch forever while looking perfectly valid in a
-- query, which is a much worse failure than a rejected insert.
ALTER TABLE canvas_image_assets
    ADD CONSTRAINT canvas_image_assets_content_hash_format
    CHECK (content_hash IS NULL OR content_hash ~ '^[0-9a-f]{64}$');

-- A scene's logical state, hashed over the canonical form defined in
-- `thunderforge_cache_core::manifest::CanonicalSceneState`.
--
-- Separate table rather than a column on `scenes` because this is derived
-- data with a different lifecycle: it is recomputed on every scene-mutating
-- event, and keeping it apart means that write never contends with an
-- ordinary scene update.
CREATE TABLE scene_state_fingerprints (
    scene_id UUID PRIMARY KEY REFERENCES scenes(scene_id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    -- The canonical-serialization version the hash was computed under.
    -- Stored so a format change can invalidate old rows by comparison
    -- instead of requiring a migration to wipe them.
    canonical_version INTEGER NOT NULL,
    computed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    -- Constitution Principle III / ADR-009: every persisted table carries
    -- who touched it, derived data included.
    updated_by UUID NOT NULL REFERENCES users(id),
    CONSTRAINT scene_state_fingerprints_content_hash_format
        CHECK (content_hash ~ '^[0-9a-f]{64}$')
);
