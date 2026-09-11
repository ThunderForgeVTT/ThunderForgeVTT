-- Spec 039 US6 and ADR-079 (accepted 2026-09-10): the link
-- `collections/copy.rs` deliberately refused to keep until now. A row says a
-- copy happened — what was copied, what it became, who took it, and when — so
-- that a takedown can reach the copy.
--
-- THE RULE THIS TABLE MUST NEVER BREAK: no user-facing surface reads it. No
-- query, field, subscription, route or admin listing, in either direction —
-- no "what came from this" and no "what did this world take". The only reader
-- is `moderation::reach`, entered from a takedown or a counter-notice that has
-- already named an entity. ADR-069's determination that a collection share is
-- not a repository rests on this table being unenumerable, and
-- `graphql/adoption_surface_tests.rs` is what keeps it so: every reference to
-- this table outside `moderation/reach.rs` fails that test.
--
-- Entity types are the moderation vocabulary (`world_actor`, `world_item`,
-- `world_ability`, `world_lore_entry`), as `collections::moderation_entity_type`
-- gives them. A scene maps to no moderation entity type, so a copied scene
-- records no adoption and a takedown cannot reach it. That is spec 015's open
-- item T042 arriving here unchanged — recorded, not forgotten.
CREATE TABLE content_adoptions (
    id UUID PRIMARY KEY,

    source_entity_type TEXT NOT NULL,
    -- No foreign key: the source may be deleted, and the walk must still pass
    -- through it to reach the copies that were made of it.
    source_entity_id UUID NOT NULL,

    copy_entity_type TEXT NOT NULL,
    copy_entity_id UUID NOT NULL,

    -- The adopter's world. Deleting it takes its copies with it, so the record
    -- of them goes too: there is nothing left for a takedown to reach.
    destination_world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,

    -- No foreign key, as in the moderation log: the fact outlives the account.
    adopted_by UUID NOT NULL,

    adopted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- From an accused entity to the copies of it: the takedown's walk.
CREATE INDEX content_adoptions_source_idx
    ON content_adoptions (source_entity_type, source_entity_id);

-- From a copy to what it was copied from. What makes the walk transitive — a
-- copy of a copy is a row whose source is itself a copy — and what lets a
-- restoration find the adopter to tell.
CREATE INDEX content_adoptions_copy_idx
    ON content_adoptions (copy_entity_type, copy_entity_id);
