-- Spec 039 US6 (FR-023, contracts/takedown-reach.md): a copy disabled because
-- its source was taken down gets a case of its own, pointing back at the
-- notice's case.
--
-- Its own case, not more rows in the notice's: `strikes_by_account` judges a
-- case by its latest event, so copy rows in the parent's case would make the
-- sharer's strike depend on which copy was written last. The child case
-- carries no `account_id`, which is also what keeps it from ever being the
-- adopter's strike (FR-023b).
--
-- NULL on every row that is not a copy's — which is every row written before
-- this migration.
ALTER TABLE content_moderation_actions ADD COLUMN parent_case_id UUID;

CREATE INDEX content_moderation_actions_parent_case_idx
    ON content_moderation_actions (parent_case_id)
    WHERE parent_case_id IS NOT NULL;
