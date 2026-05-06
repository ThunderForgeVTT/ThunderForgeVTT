-- Phase 4.9.B.1: Player presence tracking table
-- Tracks which players are connected to which worlds/scenes
-- Auto-cleaned by session lifecycle management

CREATE TABLE players_online (
    id BIGSERIAL PRIMARY KEY,
    player_id UUID NOT NULL,
    world_id UUID NOT NULL,
    scene_id UUID,
    
    -- Tracking
    connected_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMP NOT NULL DEFAULT NOW(),
    idle_duration_secs INT NOT NULL DEFAULT 0,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(player_id, world_id),
    FOREIGN KEY (player_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
);

-- Indexes for fast lookups
CREATE INDEX idx_players_online_world_id ON players_online(world_id);
CREATE INDEX idx_players_online_player_id ON players_online(player_id);
CREATE INDEX idx_players_online_last_seen ON players_online(last_seen);

-- Trigger to update updated_at
CREATE TRIGGER players_online_updated_at_trigger
BEFORE UPDATE ON players_online
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Trigger to notify on new/updated connections (for presence subscription)
CREATE OR REPLACE FUNCTION notify_players_online_change()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'players_online_channel',
    json_build_object(
      'world_id', NEW.world_id,
      'player_id', NEW.player_id,
      'action', TG_OP
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER players_online_notify_trigger
AFTER INSERT OR UPDATE ON players_online
FOR EACH ROW
EXECUTE FUNCTION notify_players_online_change();

-- Trigger to notify on disconnection
CREATE OR REPLACE FUNCTION notify_players_online_delete()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'players_online_channel',
    json_build_object(
      'world_id', OLD.world_id,
      'player_id', OLD.player_id,
      'action', 'DELETE'
    )::text
  );
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER players_online_delete_notify_trigger
AFTER DELETE ON players_online
FOR EACH ROW
EXECUTE FUNCTION notify_players_online_delete();
