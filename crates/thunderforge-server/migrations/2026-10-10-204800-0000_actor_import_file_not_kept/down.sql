-- NOT VALID: records whose file is no longer kept may exist by now.
ALTER TABLE actor_imports
    DROP CONSTRAINT IF EXISTS actor_imports_kind_shape,
    ADD CONSTRAINT actor_imports_check CHECK (
        (kind = 'import' AND version_id IS NOT NULL AND restored_from IS NULL)
        OR (kind = 'rollback' AND restored_from IS NOT NULL)
    ) NOT VALID;
