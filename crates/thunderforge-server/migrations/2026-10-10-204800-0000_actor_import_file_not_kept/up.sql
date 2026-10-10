-- Spec 048 T085: an import record outlives the file it was read from.
--
-- `actor_imports.version_id` is ON DELETE SET NULL so that deleting the
-- account that uploaded a sheet leaves the record, shown as "file no longer
-- kept". The original check demanded a version on every import, so that
-- SET NULL could never succeed and the deletion was refused instead.
ALTER TABLE actor_imports
    DROP CONSTRAINT actor_imports_check,
    ADD CONSTRAINT actor_imports_kind_shape CHECK (
        (kind = 'import' AND restored_from IS NULL)
        OR (kind = 'rollback' AND restored_from IS NOT NULL)
    );
