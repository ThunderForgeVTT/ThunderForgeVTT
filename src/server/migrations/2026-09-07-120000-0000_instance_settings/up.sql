-- Spec 040 (T007), governed by ADR-091 (instance configuration is rows).
--
-- Two tables, and the shape of the first is the decision: a key/value row
-- rather than a column per setting. FR-012 requires a new setting to inherit
-- precedence, redaction and readiness by being declared, and FR-028 requires
-- an upgrade that introduces a required setting not to stop an instance
-- starting. A wide singleton makes both of those a schema question; this makes
-- them a question for `settings::registry`, which is where the typing, the
-- environment variable name and the redaction rule already have to live
-- together.
--
-- Note what is deliberately ABSENT: any column saying where a value came from.
-- Precedence is resolved per request against the environment (ADR-088), and a
-- stored `source` would be a second, staler answer to a question the resolver
-- already answers truthfully.

-- ---------------------------------------------------------------------------
-- The configured values. One row per setting an operator has actually set;
-- an unset setting has no row and resolves from its declared default.
--
-- A row whose key `settings::registry` does not declare is INERT, not invalid:
-- it does not resolve, it is reported in readiness as unrecognised, and it is
-- never deleted. An operator who downgraded and upgraded again keeps their
-- values — the same posture ADR-041 took for provider rows.
--
-- `value` is the ciphertext (`v1.<nonce>.<ct>`, `crypto.rs`) when the
-- declaration says the setting is a secret, and the literal otherwise. There
-- is no column recording which: the declaration decides, so a value cannot be
-- stored encrypted and read as though it were not.
--
-- Provenance is nullable (Principle III, ADR-009/ADR-010) because a row may be
-- written by the instance itself during an upgrade, with no actor. A null
-- renders as "the instance", never as a blank.
-- ---------------------------------------------------------------------------
CREATE TABLE instance_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- What changed, who changed it, and what it was before. Append-only.
--
-- FR-008 wants the previous value; FR-023 forbids ever printing a credential.
-- For a secret those are in direct conflict, and the resolution is that this
-- records the TRANSITION rather than the value: the literal previous address
-- for a notice contact, and the word 'set'/'not set' for an SMTP password.
--
-- `redacted` is STORED rather than recomputed from the declaration at read
-- time, so the record stays truthful if a declaration's `secret` flag is ever
-- changed. A row that says it is redacted was redacted when it was written.
--
-- There is deliberately no column for the submitted value of a REFUSED write.
-- A rejected password that is recorded anywhere is a rejected password that
-- has leaked.
-- ---------------------------------------------------------------------------
CREATE TABLE instance_setting_changes (
    id UUID PRIMARY KEY,
    key TEXT NOT NULL,
    previous_value TEXT,
    new_value TEXT,
    redacted BOOLEAN NOT NULL DEFAULT FALSE,
    changed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    changed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    source TEXT NOT NULL,
    CONSTRAINT instance_setting_changes_source_known
        CHECK (source IN ('setup', 'admin', 'system'))
);

-- The question is always "what happened to this setting", never "what happened
-- at 14:02", so the index leads with the key.
CREATE INDEX instance_setting_changes_key_changed_at_idx
    ON instance_setting_changes(key, changed_at DESC);
