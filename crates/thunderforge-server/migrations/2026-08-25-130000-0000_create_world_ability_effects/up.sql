-- Spec 025 (T036, FR-015/FR-016/FR-020): structured, system-agnostic effects
-- on an ability.
--
-- Structurally identical to world_item_effects (spec 013) — same four effect
-- types, same free-text formula/target, same nullable trigger_kind scaffold,
-- same sort_order display ordering. That symmetry is deliberate: a future
-- resolution engine (spec 014's dice crate) should be able to consume item and
-- ability effects through one code path.
--
-- `trigger_kind` is scaffolded per FR-020 and evaluated by nothing in this
-- pass. `formula` and `target` are free text — FR-019 forbids this spec from
-- resolving, rolling, or applying an effect, so validation is structural only.
CREATE TABLE world_ability_effects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ability_id UUID NOT NULL REFERENCES world_abilities(id) ON DELETE CASCADE,
    effect_type VARCHAR(16) NOT NULL
        CHECK (effect_type IN ('heal', 'damage', 'modifier', 'attack_roll')),
    formula TEXT NOT NULL,
    target TEXT NOT NULL,
    trigger_kind VARCHAR(16)
        CHECK (trigger_kind IS NULL OR trigger_kind IN ('on_use', 'passive')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_ability_effects_ability_id_idx ON world_ability_effects(ability_id);
