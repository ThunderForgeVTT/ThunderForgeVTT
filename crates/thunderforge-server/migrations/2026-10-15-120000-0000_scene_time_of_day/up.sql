-- Spec 099 US5: the scene's time of day, which the Game Master sets and every
-- client at the table hands its engine's clock.
--
-- An hour from 0 to 24, or NULL: no clock, and the scene is lit by its
-- ambient level as it always was. No existing scene changes.
ALTER TABLE scenes
    ADD COLUMN time_of_day REAL
    CONSTRAINT scenes_time_of_day_hour CHECK (time_of_day >= 0 AND time_of_day < 24);
