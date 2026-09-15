-- Spec 045 FR-061: a placed light has a bright reach and a dim reach.
--
-- `radius` stays what it has always been, the light's outer edge, and is now
-- read as the dim reach. `bright_radius` is the bright reach.
--
-- FR-062: a light saved before this, with a single radius, keeps its look.
-- The engine has drawn such a light bright to half its radius and dim to the
-- whole of it, so that is what every existing row is given.
ALTER TABLE light_sources ADD COLUMN bright_radius DOUBLE PRECISION;

UPDATE light_sources SET bright_radius = radius * 0.5;

ALTER TABLE light_sources
    ALTER COLUMN bright_radius SET NOT NULL,
    -- A light is not bright further out than it reaches at all.
    ADD CONSTRAINT light_sources_bright_within_dim
        CHECK (bright_radius >= 0 AND bright_radius <= radius);
