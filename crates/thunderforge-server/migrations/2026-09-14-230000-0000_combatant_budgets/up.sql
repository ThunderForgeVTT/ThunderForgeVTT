-- Spec 046 Phase 8 (US5, FR-040–FR-045, research R13, contract C9): what a
-- creature has spent of its turn.
--
-- One row per combatant, created with it. Spent, never "remaining": what a
-- turn affords is the pack's (`turnStructure.budget`) and the creature's (its
-- speed), resolved when the tracker is read, so a sheet whose speed changes
-- mid-fight is shown against its new speed rather than the one it joined with.
--
-- Nothing here is capped (decision 2). Two actions spent of one is a visible
-- overspend, and a Game Master may call it a debt; a tracker that refused it
-- would be wrong about who is in charge.
CREATE TABLE world_combatant_budgets (
    combatant_id UUID PRIMARY KEY REFERENCES world_combatants(id) ON DELETE CASCADE,
    action_spent INTEGER NOT NULL DEFAULT 0,
    bonus_action_spent INTEGER NOT NULL DEFAULT 0,
    reaction_spent INTEGER NOT NULL DEFAULT 0,
    -- System units (feet for 5e).
    movement_spent DOUBLE PRECISION NOT NULL DEFAULT 0,
    -- Phase 9: from the pack's declared field when the combatant was added,
    -- refilled at the start of its turn; may go negative.
    legendary_per_round INTEGER NULL,
    legendary_remaining INTEGER NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Every combatant already in a fight starts with nothing spent, attributed to
-- whoever started its encounter.
INSERT INTO world_combatant_budgets (combatant_id, created_by, updated_by)
SELECT c.id, wc.created_by, wc.created_by
FROM world_combatants c
JOIN world_combats wc ON wc.id = c.combat_id
ON CONFLICT (combatant_id) DO NOTHING;
