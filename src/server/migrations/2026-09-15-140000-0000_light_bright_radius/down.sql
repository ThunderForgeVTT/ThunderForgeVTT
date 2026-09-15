-- Back to one radius. A light's dim reach was always `radius`, so nothing of
-- its outer edge is lost; a bright reach other than half of it is.
ALTER TABLE light_sources
    DROP CONSTRAINT IF EXISTS light_sources_bright_within_dim,
    DROP COLUMN IF EXISTS bright_radius;
