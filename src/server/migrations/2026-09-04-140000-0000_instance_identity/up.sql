-- Spec 034 (FR-036g): a stable identity for this deployment.
--
-- The binding record written onto a repository names which world AND which
-- instance is writing there, because the same world id restored onto a second
-- deployment would otherwise look like the same writer. The instance half is
-- what makes "this world, but somewhere else" a conflict rather than a match.
--
-- Singleton, following `admin_bootstrap_setup` and `auth_security_settings`:
-- one row, enforced by a CHECK rather than by convention, so a second row is
-- refused by the database instead of by whichever code path remembered.
CREATE TABLE instance_identity (
    id INTEGER PRIMARY KEY DEFAULT 1,
    CONSTRAINT instance_identity_is_singleton CHECK (id = 1),

    -- Generated on first launch. **v4, not v7**, deliberately: this value is
    -- written into an issue on a repository that may be public, and a v7 UUID
    -- front-loads a timestamp — it would publish when the instance was first
    -- started to anyone who reads it. ADR-049 makes the same call about share
    -- codes for the same reason.
    instance_id UUID NOT NULL,

    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
