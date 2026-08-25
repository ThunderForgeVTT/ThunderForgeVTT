-- Spec 022 (FR-002d, ADR-046): `worlds.active_scene_id` was added with no
-- backfill in the migration that introduced it — every existing world
-- would otherwise show Play's new empty/unloaded-canvas state until a GM
-- explicitly launches a scene, even though it already has a perfectly
-- playable scene from before this feature shipped. `create_world` now
-- sets this at creation time going forward (graphql.rs); this one-time
-- backfill gives every pre-existing world its oldest scene as a
-- reasonable "already playing this" default instead.
UPDATE worlds
SET active_scene_id = (
    SELECT scene_id FROM scenes
    WHERE scenes.world_id = worlds.id
    ORDER BY created_at ASC
    LIMIT 1
)
WHERE active_scene_id IS NULL;
