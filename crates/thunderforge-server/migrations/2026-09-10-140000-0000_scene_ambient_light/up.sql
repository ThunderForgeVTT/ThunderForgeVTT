-- Playtest 2026-09-10 P9: a scene's baseline light, which the Game Master
-- sets per scene and an imported map takes from its own file.
--
-- Bright by default, which is how every scene has always rendered: the
-- darkness layer draws nothing in daylight, so no existing scene changes.
ALTER TABLE scenes
    ADD COLUMN ambient_light TEXT NOT NULL DEFAULT 'bright'
    CONSTRAINT scenes_ambient_light_level CHECK (ambient_light IN ('bright', 'dim', 'dark'));
