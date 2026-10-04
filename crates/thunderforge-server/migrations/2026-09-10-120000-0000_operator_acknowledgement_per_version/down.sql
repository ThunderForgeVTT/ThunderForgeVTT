ALTER TABLE admin_bootstrap_oauth_sessions DROP COLUMN IF EXISTS operator_terms_version_id;

DROP INDEX IF EXISTS attestations_one_operator_per_version_idx;

-- The one-per-instance index cannot hold once an upgrade has been
-- acknowledged. Refuse rather than delete: an acknowledgement is a record of
-- what somebody took on, and a downgrade is not a reason to destroy evidence.
DO $$
BEGIN
    IF (SELECT count(*) FROM attestations WHERE purpose = 'operator') > 1 THEN
        RAISE EXCEPTION 'more than one operator acknowledgement exists; this migration cannot be reverted without deleting a record';
    END IF;
END $$;

CREATE UNIQUE INDEX attestations_one_operator_idx
    ON attestations((TRUE)) WHERE purpose = 'operator';
