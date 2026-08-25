-- Spec 022 (Scene Management Overhaul, US3/US1): a world-level default
-- grid type applied to newly created scenes, and a server-authoritative
-- "currently launched" scene for Play (ADR-046) — previously 100%
-- client-local state with no cross-client sync.
ALTER TABLE worlds
    ADD COLUMN default_scene_grid_type TEXT NOT NULL DEFAULT 'square'
        CHECK (default_scene_grid_type IN ('square', 'hex', 'gridless')),
    ADD COLUMN active_scene_id UUID REFERENCES scenes(scene_id) ON DELETE SET NULL;
