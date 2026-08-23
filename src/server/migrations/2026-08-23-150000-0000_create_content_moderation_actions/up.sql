-- Spec 015: DMCA notice-and-takedown moderation log. Deliberately NO
-- foreign keys with ON DELETE CASCADE to worlds/users/content tables —
-- FR-013 requires moderation history to survive deletion of the world,
-- account, or entity it references.
CREATE TABLE content_moderation_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL,
    action_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    world_id UUID NOT NULL,
    account_id UUID,
    claimant_name TEXT NOT NULL DEFAULT '',
    claimant_contact TEXT NOT NULL DEFAULT '',
    copyrighted_work_description TEXT NOT NULL DEFAULT '',
    infringing_material_location TEXT NOT NULL DEFAULT '',
    good_faith_statement BOOLEAN NOT NULL DEFAULT FALSE,
    accuracy_statement BOOLEAN NOT NULL DEFAULT FALSE,
    signature TEXT NOT NULL DEFAULT '',
    validity_result TEXT,
    missing_elements TEXT[],
    counter_notice_id UUID,
    restoration_due_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by UUID
);

CREATE INDEX content_moderation_actions_case_id_idx ON content_moderation_actions(case_id);
CREATE INDEX content_moderation_actions_entity_idx ON content_moderation_actions(entity_type, entity_id);
CREATE INDEX content_moderation_actions_account_id_idx ON content_moderation_actions(account_id);
