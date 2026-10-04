-- Spec 025 (T005, FR-024/FR-026): the per-ability ownership block.
--
-- Governs EDIT RIGHTS ONLY. Visibility is world_abilities.gm_only — absence of
-- a row here means Viewer (read-only), never "hidden".
--
-- PK has no DB default: the app supplies Uuid::now_v7(), matching
-- world_item_permissions. UNIQUE (ability_id, user_id) is the upsert conflict
-- target for setAbilityPermission.
CREATE TABLE world_ability_permissions (
    id UUID PRIMARY KEY,
    ability_id UUID NOT NULL REFERENCES world_abilities(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    level VARCHAR(16) NOT NULL CHECK (level IN ('Viewer', 'Editor', 'Owner')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (ability_id, user_id)
);

CREATE INDEX world_ability_permissions_ability_id_idx ON world_ability_permissions(ability_id);
CREATE INDEX world_ability_permissions_user_id_idx ON world_ability_permissions(user_id);
