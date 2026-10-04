-- Spec 063, FR-007: an access grant remembers whether a claim created it.
--
-- Claiming a character now grants the claiming player Editor on it, and
-- releasing the claim takes back exactly that. "Exactly that" needs the row
-- to say where it came from: without it a release could not tell the Editor
-- a claim gave from the Editor a Game Master gave by hand, and would have to
-- either strand the first or destroy the second.
--
-- A property of the row rather than a history table, because the feature
-- asks one question of it (did a claim create this?) and a hand edit is the
-- single event that changes the answer (FR-008). The DEFAULT backfills every
-- existing row with false: every grant made before this was made by hand.
ALTER TABLE world_actor_permissions
    ADD COLUMN granted_by_claim BOOLEAN NOT NULL DEFAULT false;

-- Characters already claimed when this ships. Their players hold a claim
-- that granted nothing, which is the defect being fixed, so they are given
-- what a claim made today would give (the spec's stated default).
--
-- The same three cases as the live path in `mutations_actor_claims.rs`: no
-- row becomes a claim-flagged Editor; a row below Editor is raised and
-- flagged; a row at Editor or Owner is the Game Master's and is left exactly
-- as it is (FR-005), which is what the WHERE on the conflict arm does.
INSERT INTO world_actor_permissions (id, actor_id, user_id, level, granted_by_claim)
SELECT gen_random_uuid(), claims.actor_id, members.user_id, 'Editor', true
FROM world_actor_claims AS claims
JOIN world_members AS members ON members.id = claims.world_member_id
ON CONFLICT (actor_id, user_id) DO UPDATE
    SET level = 'Editor',
        granted_by_claim = true,
        updated_at = CURRENT_TIMESTAMP
    WHERE world_actor_permissions.level NOT IN ('Editor', 'Owner');
