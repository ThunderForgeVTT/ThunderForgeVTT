-- Spec 046 Phase 6 (research R1, R2, R8, R15; ADR-101): an attack is aimed at
-- something, resolved on the server, and its damage is an offer.
--
-- `world_attacks` is the record every seat's view of an attack is built from,
-- per viewer, at read time (contract §3). Nothing in it is sent as it stands:
-- the attacker's and the target's identity are withheld from a viewer who
-- cannot see them, so the labels kept here are the server's, never a payload.
--
-- `world_offers` is the damage a hit offers to whoever controls the target.
-- Pending until a controller or a Game Master takes or declines it, once
-- (C6); `applied` when auto-apply took it at creation (C5).

-- ---------------------------------------------------------------------------
-- Auto-apply: a world default, off, and a per-encounter override (FR-006).
-- ---------------------------------------------------------------------------
ALTER TABLE worlds
    ADD COLUMN auto_apply_npc_damage BOOLEAN NOT NULL DEFAULT false;

-- NULL is "use the world's". The override ends with the encounter because it
-- lives on the encounter's row.
ALTER TABLE world_combats
    ADD COLUMN auto_apply BOOLEAN NULL;

-- ---------------------------------------------------------------------------
-- What an ability or an item is as an attack (research R2). All optional, so
-- every existing row is unchanged in meaning: an action, needing line of
-- sight, with no declared reach (which Phase 7 flags, never refuses).
-- ---------------------------------------------------------------------------
ALTER TABLE world_abilities
    ADD COLUMN reach DOUBLE PRECISION NULL CHECK (reach IS NULL OR reach >= 0),
    ADD COLUMN range_normal DOUBLE PRECISION NULL CHECK (range_normal IS NULL OR range_normal >= 0),
    ADD COLUMN range_long DOUBLE PRECISION NULL CHECK (range_long IS NULL OR range_long >= 0),
    ADD COLUMN needs_line_of_sight BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN action_cost TEXT NOT NULL DEFAULT 'action'
        CHECK (action_cost IN ('action', 'bonus_action', 'reaction', 'legendary', 'free')),
    ADD COLUMN legendary_cost INTEGER NOT NULL DEFAULT 1 CHECK (legendary_cost >= 0),
    ADD COLUMN multiattack UUID[] NOT NULL DEFAULT '{}',
    ADD CONSTRAINT world_abilities_range_long_not_short
        CHECK (range_long IS NULL OR range_normal IS NULL OR range_long >= range_normal);

ALTER TABLE world_items
    ADD COLUMN reach DOUBLE PRECISION NULL CHECK (reach IS NULL OR reach >= 0),
    ADD COLUMN range_normal DOUBLE PRECISION NULL CHECK (range_normal IS NULL OR range_normal >= 0),
    ADD COLUMN range_long DOUBLE PRECISION NULL CHECK (range_long IS NULL OR range_long >= 0),
    ADD COLUMN needs_line_of_sight BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN action_cost TEXT NOT NULL DEFAULT 'action'
        CHECK (action_cost IN ('action', 'bonus_action', 'reaction', 'legendary', 'free')),
    ADD COLUMN legendary_cost INTEGER NOT NULL DEFAULT 1 CHECK (legendary_cost >= 0),
    ADD COLUMN multiattack UUID[] NOT NULL DEFAULT '{}',
    ADD CONSTRAINT world_items_range_long_not_short
        CHECK (range_long IS NULL OR range_normal IS NULL OR range_long >= range_normal);

-- ---------------------------------------------------------------------------
-- The attack record.
-- ---------------------------------------------------------------------------
CREATE TABLE world_attacks (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
    -- The encounter it was made in, when one was running.
    combat_id UUID NULL REFERENCES world_combats(id) ON DELETE SET NULL,
    -- Null only for a lair action (Phase 9), or once the token is deleted.
    attacker_token_id UUID NULL REFERENCES tokens(token_id) ON DELETE SET NULL,
    -- Null when made with no target ("a roll into the air"): nothing is
    -- offered or applied. Also null once the token is deleted; `outcome`
    -- still says whether there was one.
    target_token_id UUID NULL REFERENCES tokens(token_id) ON DELETE SET NULL,
    -- The names as they were when the attack was made. Server-side only: a
    -- viewer is sent one only when they may see that token (contract §3).
    attacker_label TEXT NOT NULL,
    target_label TEXT NULL,
    ability_id UUID NULL REFERENCES world_abilities(id) ON DELETE SET NULL,
    item_id UUID NULL REFERENCES world_items(id) ON DELETE SET NULL,
    ability_name TEXT NOT NULL,
    -- The parent attack of a multiattack's parts: the first part, which the
    -- others point at.
    multiattack_of UUID NULL REFERENCES world_attacks(id) ON DELETE CASCADE,
    to_hit_roll_id UUID NULL REFERENCES world_roll_records(id) ON DELETE SET NULL,
    damage_roll_id UUID NULL REFERENCES world_roll_records(id) ON DELETE SET NULL,
    -- The value the total was compared with; null when the target has none.
    defence INTEGER NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('hit', 'miss', 'no_defence', 'no_target')),
    -- System units, footprint to footprint. Filled by Phase 7.
    distance DOUBLE PRECISION NULL,
    flags TEXT[] NOT NULL DEFAULT '{}',
    action_cost TEXT NOT NULL
        CHECK (action_cost IN ('action', 'bonus_action', 'reaction', 'legendary', 'free')),
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- One of the two, or neither once the one it named is deleted.
    CONSTRAINT world_attacks_ability_or_item CHECK (ability_id IS NULL OR item_id IS NULL)
);

-- `sceneAttacks(sceneId, before)`: newest first, a page at a time.
CREATE INDEX world_attacks_scene_idx ON world_attacks (scene_id, created_at DESC, id DESC);
CREATE INDEX world_attacks_multiattack_idx ON world_attacks (multiattack_of)
    WHERE multiattack_of IS NOT NULL;

-- ---------------------------------------------------------------------------
-- The offer.
-- ---------------------------------------------------------------------------
CREATE TABLE world_offers (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
    -- Null for a Game Master's direct offer of healing.
    attack_id UUID NULL REFERENCES world_attacks(id) ON DELETE CASCADE,
    -- An offer against a token that is gone is moot, so it goes with it.
    -- Controllers are resolved at read and resolve time (research R7).
    target_token_id UUID NOT NULL REFERENCES tokens(token_id) ON DELETE CASCADE,
    -- Whether the target was linked when the offer was made. Taking an offer
    -- whose token has since been relinked or unlinked is refused: the damage
    -- was rolled against one record of hit points and would land on another
    -- (research "offers and relinking", contract C6a).
    target_linked BOOLEAN NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('damage', 'healing')),
    amount INTEGER NOT NULL CHECK (amount >= 0),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'taken', 'declined', 'applied')),
    resolved_by UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    resolved_on_behalf BOOLEAN NOT NULL DEFAULT false,
    resolved_at TIMESTAMP NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- "Your pending offers" on reconnect (FR-008).
CREATE INDEX world_offers_pending_idx ON world_offers (target_token_id)
    WHERE status = 'pending';
CREATE INDEX world_offers_world_pending_idx ON world_offers (world_id)
    WHERE status = 'pending';
CREATE INDEX world_offers_attack_idx ON world_offers (attack_id);
