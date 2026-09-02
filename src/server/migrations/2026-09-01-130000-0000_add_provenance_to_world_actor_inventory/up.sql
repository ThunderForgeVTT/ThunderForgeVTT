-- Provenance for world_actor_inventory: who put this row here, and who last
-- changed it. Constitution Principle III asks for created_by/updated_by on
-- tables that record mutations; this table predates that and carried only
-- created_at/updated_at, so this is a deliberate retrofit of an existing
-- table rather than a new one.
--
-- NULLABLE, unlike world_actor_images and world_abilities, which are NOT
-- NULL. Those tables were born with these columns and every row they hold
-- has a real author. This one already holds rows written before provenance
-- was tracked, and nobody knows who created them. NOT NULL would force a
-- backfill, and any backfilled value — the world owner, a system user, the
-- first admin — would state an author we do not have. A null says the true
-- thing: this row predates the recording of who wrote it.
--
-- The nullability is a fact about history, not a licence for new writes.
-- Every write site in src/server sets both columns; a null in a row created
-- after this migration is a bug, not an allowed case.
ALTER TABLE world_actor_inventory
    ADD COLUMN created_by UUID REFERENCES users(id),
    ADD COLUMN updated_by UUID REFERENCES users(id);
