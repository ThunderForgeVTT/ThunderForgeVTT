-- Spec 025 (T058, FR-028/FR-031): abilities become a fourth in-text link
-- target. Mirrors spec 013's add_item_target_to_world_lore_links exactly —
-- four operations, widening the existing 3-way target to 4-way.
--
-- ON DELETE SET NULL (never RESTRICT/CASCADE) is load-bearing: FR-031 requires
-- deleting an ability to succeed even when lore links to it, with the row
-- surviving and rendering unresolved.
--
-- The "at most one target" CHECK stays deliberately looser than "target_kind
-- must match the non-null column". A stricter constraint would be re-evaluated
-- when ON DELETE SET NULL fires and would block exactly the deletions FR-031
-- requires to succeed. target_kind is authoritative only at insert time; every
-- read path treats a null FK as unresolved regardless of the stored label.
ALTER TABLE world_lore_links
    ADD COLUMN target_ability_id UUID REFERENCES world_abilities(id) ON DELETE SET NULL;

CREATE INDEX world_lore_links_target_ability_id_idx ON world_lore_links(target_ability_id);

ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_target_kind_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_target_kind_check
    CHECK (target_kind IN ('lore_entry', 'actor', 'item', 'ability', 'unresolved'));

ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_check
    CHECK (
        (CASE WHEN target_lore_entry_id IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_actor_id      IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_item_id       IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_ability_id    IS NOT NULL THEN 1 ELSE 0 END) <= 1
    );
