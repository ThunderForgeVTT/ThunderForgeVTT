-- Spec 046 Phase 9 (US6, FR-053, research R14): a lair takes its place in the
-- turn order.
--
-- A lair is a combatant that is not a creature: no token, no actor, at
-- initiative count 20, losing ties (tiebreak -1). The Game Master adds it from
-- the tracker and acts for it when its count comes round.
ALTER TABLE world_combatants
    ADD COLUMN kind TEXT NOT NULL DEFAULT 'creature'
        CHECK (kind IN ('creature', 'lair')),
    ADD CONSTRAINT world_combatants_lair_has_no_creature
        CHECK (kind <> 'lair' OR (token_id IS NULL AND actor_id IS NULL));

-- Who made an attack when there is no token to say: a lair's action has no
-- attacker token, and neither has a creature's attack once its token is
-- deleted. The lair's label is told to every seat (the tracker already shows
-- it to everyone, and there is nothing of it to see or hide); a deleted
-- creature is told to nobody but the Game Master. This column is what tells
-- the two apart.
ALTER TABLE world_attacks
    ADD COLUMN attacker_kind TEXT NOT NULL DEFAULT 'creature'
        CHECK (attacker_kind IN ('creature', 'lair'));
