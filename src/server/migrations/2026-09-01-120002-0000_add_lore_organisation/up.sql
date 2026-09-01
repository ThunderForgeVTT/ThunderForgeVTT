-- Spec 031 (T013, US8/FR-038): lore gains a tree and tags.
--
-- `parent_id` is a nullable self-reference: a null parent is a root entry.
--
-- ON DELETE SET NULL rather than CASCADE — deleting a parent must not silently
-- take its children's content with it. The children become roots, which is
-- recoverable; a cascade is not. Re-parenting to the deleted node's own parent
-- is the application's job when it wants to preserve the shape.
--
-- Cycles are NOT enforced here. A self-reference can be caught by a CHECK, but
-- a longer cycle (a -> b -> a) cannot be without a trigger, and half an
-- invariant in the database is worse than a whole one in one place. Cycle
-- rejection lives in application code at the data boundary, in the lore entry
-- mutation that sets a parent (src/server/src/graphql/mutations_lore.rs).
ALTER TABLE world_lore_entries
    ADD COLUMN parent_id UUID;

ALTER TABLE world_lore_entries
    ADD CONSTRAINT world_lore_entries_parent_id_fkey
    FOREIGN KEY (parent_id) REFERENCES world_lore_entries(id) ON DELETE SET NULL;

-- "Give me this entry's children" is the tree read.
CREATE INDEX world_lore_entries_parent_id_idx ON world_lore_entries(parent_id);

-- Tags are many-to-many and append-only: a tag is added or removed, never
-- edited in place, so provenance is created_by/created_at only — matching
-- world_lore_image_assets rather than the fuller world_abilities shape.
--
-- `tag` is stored normalised for comparison by the caller; the unique
-- constraint is what makes that normalisation load-bearing.
CREATE TABLE world_lore_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lore_entry_id UUID NOT NULL REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (lore_entry_id, tag)
);

CREATE INDEX world_lore_tags_lore_entry_id_idx ON world_lore_tags(lore_entry_id);
-- "Which entries carry this tag" is the other direction of the same question.
CREATE INDEX world_lore_tags_tag_idx ON world_lore_tags(tag);
