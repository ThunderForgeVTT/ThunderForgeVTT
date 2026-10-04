ALTER TABLE worlds
    ADD COLUMN created_by UUID,
    ADD COLUMN updated_by UUID;

ALTER TABLE world_tokens
    ADD COLUMN created_by UUID,
    ADD COLUMN updated_by UUID;

ALTER TABLE world_events
    ADD COLUMN updated_at TIMESTAMP,
    ADD COLUMN created_by UUID,
    ADD COLUMN updated_by UUID;

ALTER TABLE policies
    ADD COLUMN created_by UUID,
    ADD COLUMN updated_by UUID;

DO $$
DECLARE
    fallback_user UUID;
    has_existing_domain_rows BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM worlds
        UNION ALL
        SELECT 1 FROM world_tokens
        UNION ALL
        SELECT 1 FROM world_events
        UNION ALL
        SELECT 1 FROM policies
        LIMIT 1
    ) INTO has_existing_domain_rows;

    SELECT id
    INTO fallback_user
    FROM users
    ORDER BY is_admin DESC, created_at ASC
    LIMIT 1;

    IF fallback_user IS NULL AND has_existing_domain_rows THEN
        RAISE EXCEPTION 'Cannot backfill created_by/updated_by without at least one existing user';
    END IF;

    IF fallback_user IS NOT NULL THEN
        UPDATE worlds
        SET created_by = fallback_user,
            updated_by = fallback_user
        WHERE created_by IS NULL
           OR updated_by IS NULL;

        UPDATE world_tokens
        SET created_by = fallback_user,
            updated_by = fallback_user
        WHERE created_by IS NULL
           OR updated_by IS NULL;

        UPDATE world_events
        SET updated_at = COALESCE(updated_at, created_at),
            created_by = fallback_user,
            updated_by = fallback_user
        WHERE updated_at IS NULL
           OR created_by IS NULL
           OR updated_by IS NULL;

        UPDATE policies
        SET created_by = fallback_user,
            updated_by = fallback_user
        WHERE created_by IS NULL
           OR updated_by IS NULL;
    END IF;
END $$;

ALTER TABLE worlds
    ALTER COLUMN created_by SET NOT NULL,
    ALTER COLUMN updated_by SET NOT NULL,
    ADD CONSTRAINT worlds_created_by_fkey FOREIGN KEY (created_by) REFERENCES users (id),
    ADD CONSTRAINT worlds_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users (id);

ALTER TABLE world_tokens
    ALTER COLUMN created_by SET NOT NULL,
    ALTER COLUMN updated_by SET NOT NULL,
    ADD CONSTRAINT world_tokens_created_by_fkey FOREIGN KEY (created_by) REFERENCES users (id),
    ADD CONSTRAINT world_tokens_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users (id);

ALTER TABLE world_events
    ALTER COLUMN updated_at SET NOT NULL,
    ALTER COLUMN created_by SET NOT NULL,
    ALTER COLUMN updated_by SET NOT NULL,
    ADD CONSTRAINT world_events_created_by_fkey FOREIGN KEY (created_by) REFERENCES users (id),
    ADD CONSTRAINT world_events_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users (id);

ALTER TABLE policies
    ALTER COLUMN created_by SET NOT NULL,
    ALTER COLUMN updated_by SET NOT NULL,
    ADD CONSTRAINT policies_created_by_fkey FOREIGN KEY (created_by) REFERENCES users (id),
    ADD CONSTRAINT policies_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users (id);

CREATE INDEX idx_worlds_created_by ON worlds (created_by);
CREATE INDEX idx_world_tokens_created_by ON world_tokens (created_by);
CREATE INDEX idx_world_events_created_by ON world_events (created_by);
CREATE INDEX idx_policies_created_by ON policies (created_by);

ALTER TABLE world_events
    DROP CONSTRAINT IF EXISTS world_events_world_id_fkey;

ALTER TABLE world_events
    ADD CONSTRAINT world_events_world_id_fkey
    FOREIGN KEY (world_id) REFERENCES worlds (id) ON DELETE CASCADE;
