-- Phase 4.10.A: Invite codes table for multiplayer campaign onboarding
-- Allows world owners to generate shareable links for players to join campaigns
-- Supports expiry and usage limits

CREATE TABLE world_invites (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL,
    invite_code VARCHAR(32) NOT NULL UNIQUE,
    max_uses INT NOT NULL DEFAULT 0,
    used_count INT NOT NULL DEFAULT 0,
    expires_at TIMESTAMP,
    created_by UUID NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Constraints
    FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE CASCADE,
    
    -- Check constraints
    CONSTRAINT positive_max_uses CHECK (max_uses > 0),
    CONSTRAINT valid_used_count CHECK (used_count >= 0 AND used_count <= max_uses)
);

-- Indexes for fast lookups
CREATE INDEX idx_world_invites_world_id ON world_invites(world_id);
CREATE INDEX idx_world_invites_created_by ON world_invites(created_by);
CREATE INDEX idx_world_invites_invite_code ON world_invites(invite_code);
CREATE INDEX idx_world_invites_expires_at ON world_invites(expires_at);

-- Trigger to update updated_at
CREATE TRIGGER world_invites_updated_at_trigger
BEFORE UPDATE ON world_invites
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Audit trigger: notify on invite creation/update
CREATE OR REPLACE FUNCTION notify_world_invites_change()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'world_events_channel',
    json_build_object(
      'world_id', NEW.world_id,
      'event_type', 'invite_' || LOWER(TG_OP),
      'invite_id', NEW.id,
      'created_by', NEW.created_by
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_invites_notify_trigger
AFTER INSERT OR UPDATE ON world_invites
FOR EACH ROW
EXECUTE FUNCTION notify_world_invites_change();
