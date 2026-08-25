-- Spec 025 (T049, FR-021/FR-022/FR-023): which abilities an actor knows.
--
-- Mirrors world_actor_inventory's tombstone pattern, minus quantity:
--
--   * `ability_id` is NULLABLE with ON DELETE SET NULL. Deleting an ability
--     must never be blocked by actors knowing it (FR-023) — the entry row
--     survives with a null reference.
--   * `ability_name_snapshot` is captured at attach time so a tombstoned row
--     still renders "Fireball (deleted)" rather than an unidentifiable orphan.
--   * UNIQUE (actor_id, ability_id) enforces FR-021's no-duplicates rule in the
--     database. Note Postgres treats NULLs as distinct here, so multiple
--     deleted-ability rows per actor are permitted — which is correct: two
--     different deleted abilities must both remain listed.
--
-- No `quantity` column, deliberately: an actor either knows an ability or does
-- not. Slots, charges, and preparation are explicit Non-Goals.
CREATE TABLE world_actor_abilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    ability_id UUID REFERENCES world_abilities(id) ON DELETE SET NULL,
    ability_name_snapshot TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (actor_id, ability_id)
);

CREATE INDEX world_actor_abilities_actor_id_idx ON world_actor_abilities(actor_id);
CREATE INDEX world_actor_abilities_ability_id_idx ON world_actor_abilities(ability_id);
