-- Phase 4.10.A: World membership table tracking which users belong to which worlds with assigned roles
-- Supports Owner/GM/Player role hierarchy for permission management

CREATE TABLE world_members (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL,
    user_id UUID NOT NULL,
    role VARCHAR(32) NOT NULL DEFAULT 'Player' CHECK (role IN ('Owner', 'GM', 'Player')),
    joined_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Constraints: one membership per user per world
    UNIQUE(world_id, user_id),
    FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Indexes for fast lookups
CREATE INDEX idx_world_members_world_id ON world_members(world_id);
CREATE INDEX idx_world_members_user_id ON world_members(user_id);
CREATE INDEX idx_world_members_role ON world_members(role);

-- Trigger to update updated_at
CREATE TRIGGER world_members_updated_at_trigger
BEFORE UPDATE ON world_members
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Audit trigger: notify on member join/role change
CREATE OR REPLACE FUNCTION notify_world_members_change()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'world_events_channel',
    json_build_object(
      'world_id', NEW.world_id,
      'event_type', 'member_' || LOWER(TG_OP),
      'user_id', NEW.user_id,
      'role', NEW.role
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_members_notify_trigger
AFTER INSERT OR UPDATE ON world_members
FOR EACH ROW
EXECUTE FUNCTION notify_world_members_change();

-- Trigger to notify on member removal
CREATE OR REPLACE FUNCTION notify_world_members_delete()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'world_events_channel',
    json_build_object(
      'world_id', OLD.world_id,
      'event_type', 'member_delete',
      'user_id', OLD.user_id
    )::text
  );
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_members_delete_notify_trigger
AFTER DELETE ON world_members
FOR EACH ROW
EXECUTE FUNCTION notify_world_members_delete();
