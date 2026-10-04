-- Spec 035 (T004/T005), governed by ADR-072 (the instance decides admission
-- before ADR-042 decides provisioning).
--
-- Note what is deliberately ABSENT, as the spec-025 and spec-026 share
-- migrations do: any index that would let invitations be looked up by code
-- outside redemption. FR-019's no-enumeration property is structural — only
-- the redemption path resolves a code, and it consumes a use doing so.

-- ---------------------------------------------------------------------------
-- The policy. A singleton, exactly like auth_security_settings.
-- ---------------------------------------------------------------------------
CREATE TABLE instance_access_settings (
    id INTEGER PRIMARY KEY,
    access_policy TEXT NOT NULL,
    updated_by UUID REFERENCES users(id),
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT instance_access_settings_singleton CHECK (id = 1),
    CONSTRAINT instance_access_settings_policy_known
        CHECK (access_policy IN ('open', 'invite_only', 'closed'))
);

-- FR-013 and FR-013a: the seeded value depends on whether this instance is new.
--
-- A fresh install gets invite_only: it admits no stranger, while letting the
-- operator issue a working invitation as their first act. (closed would refuse
-- even a valid invitation, so issuing one on a fresh instance would fail
-- confusingly.)
--
-- An instance that already has users gets open, preserving the admission
-- behaviour it had before this upgrade. Silently closing a running community
-- because it upgraded would be a worse failure than the one this feature
-- prevents; closing is an explicit operator act.
--
-- This branch must live here rather than in a lazy ensure_-on-first-read:
-- after the fact, nothing can tell a fresh instance from an upgraded one.
INSERT INTO instance_access_settings (id, access_policy)
VALUES (
    1,
    CASE WHEN EXISTS (SELECT 1 FROM users) THEN 'open' ELSE 'invite_only' END
);

-- ---------------------------------------------------------------------------
-- The invitation. Mirrors world_invites minus world_id.
-- ---------------------------------------------------------------------------
CREATE TABLE instance_invitations (
    id UUID PRIMARY KEY,
    invite_code VARCHAR(32) NOT NULL UNIQUE,
    max_uses INTEGER NOT NULL,
    used_count INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMP,
    note TEXT,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT instance_invitations_max_uses_positive CHECK (max_uses >= 1),
    CONSTRAINT instance_invitations_used_within_cap CHECK (used_count <= max_uses)
);

-- ---------------------------------------------------------------------------
-- Who came in on which link. A table rather than a redeemed_by column,
-- because max_uses may exceed 1 and FR-018 requires every redeemer.
-- ---------------------------------------------------------------------------
CREATE TABLE instance_invitation_redemptions (
    id UUID PRIMARY KEY,
    invitation_id UUID NOT NULL REFERENCES instance_invitations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    route TEXT NOT NULL,
    redeemed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT instance_invitation_redemptions_once UNIQUE (invitation_id, user_id)
);

CREATE INDEX instance_invitation_redemptions_invitation_idx
    ON instance_invitation_redemptions(invitation_id);

-- ---------------------------------------------------------------------------
-- The audit trail. Append-only. FR-004 and FR-012.
--
-- There is deliberately NO email or identifier column. FR-012 forbids
-- recording submitted credentials, and an audit log that cannot express the
-- violation is easier to keep honest than one that merely promises not to.
-- Recording the address would also turn this into a list of people who tried
-- to join, which is the accumulation US5 exists to prevent.
-- ---------------------------------------------------------------------------
CREATE TABLE instance_access_events (
    id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,
    occurred_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    previous_policy TEXT,
    new_policy TEXT,
    attempted_route TEXT,
    policy_at_attempt TEXT,
    CONSTRAINT instance_access_events_type_known
        CHECK (event_type IN ('policy_changed', 'admission_refused', 'invitation_redeemed'))
);

CREATE INDEX instance_access_events_occurred_at_idx
    ON instance_access_events(occurred_at DESC);

-- ---------------------------------------------------------------------------
-- FR-016: an invitation must survive the OAuth round trip.
--
-- It rides the authorization session the flow already creates and consumes,
-- rather than a cookie of its own: the session is already the thing that
-- carries per-attempt state (`return_to`) across the provider redirect, it is
-- already single-use, and it already expires. A cookie would be a second
-- mechanism with its own lifetime for the same job.
-- ---------------------------------------------------------------------------
ALTER TABLE oauth_authorization_sessions
    ADD COLUMN invitation_code VARCHAR(32);
