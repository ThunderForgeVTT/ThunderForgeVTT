-- Spec 044 phase (d), FR-035 and ADR-106: a stored image remembers the hero
-- spec that drew it.
--
-- On the image row, not the actor: the spec describes this picture, so
-- replacing the picture with an uploaded file clears it in the same write
-- (contract B8) and the two can never disagree. NULL means "not drawn from a
-- spec", which is every image stored before this column existed.
--
-- The column takes any JSON; what may be written is decided in Rust before
-- the write (`heroes/spec_schema.rs`, contract B7): an object, at most 4 KB,
-- passing HERO_SPEC_SCHEMA.
ALTER TABLE world_actor_images
    ADD COLUMN hero_spec JSONB NULL;
