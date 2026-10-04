-- Phase 4.8.1: System-Agnostic Actor Data Architecture
-- Three-layer foundation: Universal → System-Specific → Rendering

-- ============================================================================
-- LAYER 1: world_actors - Universal Actor Registry
-- ============================================================================
-- Stores actor identity, ownership, location, type
-- Same schema for D&D 5e characters, Pathfinder NPCs, hazards, props, light sources
-- game_system_id is nullable: NULL for props/hazards, 'dnd5e'/'pathfinder2e' for game objects

CREATE TABLE world_actors (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
  
  -- Actor identification
  actor_type VARCHAR NOT NULL CHECK (actor_type IN ('character', 'npc', 'hazard', 'prop', 'light_source', 'vehicle')),
  game_system_id VARCHAR,  -- NULL for non-game objects (props, hazards), 'dnd5e'/'pathfinder2e' for game systems
  label TEXT NOT NULL,
  
  -- Ownership & Permissions (ADR-010 enforcement)
  created_by UUID NOT NULL REFERENCES users(id),
  owned_by UUID NOT NULL REFERENCES users(id),
  is_public BOOLEAN NOT NULL DEFAULT FALSE,
  is_npc BOOLEAN NOT NULL DEFAULT FALSE,
  
  -- Audit timestamps
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
  
  -- Constraints
  CONSTRAINT actor_system_type_consistency CHECK (
    CASE
      WHEN actor_type IN ('hazard', 'prop', 'light_source') THEN game_system_id IS NULL
      WHEN actor_type IN ('character', 'npc', 'vehicle') THEN game_system_id IS NOT NULL OR actor_type = 'vehicle'
      ELSE TRUE
    END
  )
);

-- Indexes for common queries
CREATE INDEX idx_world_actors_world_id ON world_actors(world_id);
CREATE INDEX idx_world_actors_scene_id ON world_actors(scene_id);
CREATE INDEX idx_world_actors_game_system_id ON world_actors(game_system_id);
CREATE INDEX idx_world_actors_owned_by ON world_actors(owned_by);
CREATE INDEX idx_world_actors_created_by ON world_actors(created_by);
CREATE INDEX idx_world_actors_actor_type ON world_actors(actor_type);
CREATE INDEX idx_world_actors_world_scene ON world_actors(world_id, scene_id);

-- ============================================================================
-- LAYER 2: world_actor_system_data - System-Specific Data (KEY INNOVATION)
-- ============================================================================
-- Five semantic JSONB columns: same column names for all systems, different content structure
-- Example:
--   D&D 5e:      ability_data = { "strength": 10, "dexterity": 12, ... }
--   Pathfinder:  ability_data = { "strength_mod": 0, "reflex_mod": 2, ... }
--   CoC 7e:      ability_data = { "str": 65, "con": 58, "pow": 72, ... }
-- One row per actor IF game_system_id IS NOT NULL

CREATE TABLE world_actor_system_data (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  actor_id UUID NOT NULL UNIQUE REFERENCES world_actors(id) ON DELETE CASCADE,
  game_system_id VARCHAR NOT NULL,  -- 'dnd5e', 'pathfinder2e', 'coc7e', etc.
  
  -- Type-indexed JSONB columns (partition by data category, not system)
  ability_data JSONB,          -- Base ability scores/modifiers (structure varies by system)
  resource_data JSONB,         -- HP, mana, sanity, focus, etc. (varies by system)
  proficiency_data JSONB,      -- Skills, weapon/armor proficiencies, languages (varies by system)
  trait_data JSONB,            -- Class, subclass, feats, backgrounds, spells known (varies by system)
  spell_data JSONB,            -- Spellbook, slots, prepared spells (varies by system)
  
  -- Ownership & Audit (ADR-010 enforcement)
  created_by UUID NOT NULL REFERENCES users(id),
  updated_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
  
  -- Constraints
  CONSTRAINT actor_system_data_consistency CHECK (actor_id IS NOT NULL AND game_system_id IS NOT NULL)
);

-- Indexes for common queries
CREATE INDEX idx_actor_system_data_actor_id ON world_actor_system_data(actor_id);
CREATE INDEX idx_actor_system_data_game_system ON world_actor_system_data(game_system_id);
CREATE INDEX idx_actor_system_data_updated_at ON world_actor_system_data(updated_at DESC);

-- ============================================================================
-- AUDIT INTEGRATION: world_events trigger for system-agnostic data mutations
-- ============================================================================
-- Insert audit records for any actor or system data changes

CREATE OR REPLACE FUNCTION audit_actor_system_data_change()
RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify(
    'world_events_channel',
    json_build_object(
      'event_type', TG_OP,
      'table', TG_TABLE_NAME,
      'actor_id', NEW.actor_id,
      'game_system_id', NEW.game_system_id,
      'id', NEW.id
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on INSERT/UPDATE to world_actor_system_data
CREATE TRIGGER actor_system_data_notify_trigger
AFTER INSERT OR UPDATE ON world_actor_system_data
FOR EACH ROW EXECUTE FUNCTION audit_actor_system_data_change();
