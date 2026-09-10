-- Spec 039 T084/T085 (FR-041, FR-044).

-- One operator acknowledgement per VERSION of the words, not one ever.
--
-- The first index allowed a single operator row for the life of the instance,
-- which was right for "setup ran twice" and wrong for an upgrade: when the
-- operator statement's words change, the version changes (it is the hash of
-- the words, ADR-076), and FR-044 asks that the administrator be shown what
-- changed and acknowledge the new version rather than have it applied to them
-- silently. That is a second row. The same words twice is still refused.
DROP INDEX IF EXISTS attestations_one_operator_idx;
CREATE UNIQUE INDEX attestations_one_operator_per_version_idx
    ON attestations (terms_version_id) WHERE purpose = 'operator';

-- First-run setup through a trusted OAuth provider creates the administrator
-- in the callback, after a round trip to the provider. The version of the
-- operator statement the person acknowledged before leaving has to survive
-- that trip, so the callback can record it inside the transaction that
-- creates them. Nullable only because sessions started before this migration
-- exist; the callback refuses one without it.
ALTER TABLE admin_bootstrap_oauth_sessions ADD COLUMN operator_terms_version_id TEXT;
