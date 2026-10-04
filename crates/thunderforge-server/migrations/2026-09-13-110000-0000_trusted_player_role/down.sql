-- Taking the Trusted Player role back out of the column.
--
-- Refused outright while anybody holds it. The two quiet alternatives are
-- both wrong: re-adding the narrow CHECK would fail on the first such row
-- with an error that names a constraint rather than the reason, and
-- demoting those members to Player first would take a table's trust away
-- from people without anybody deciding to. Somebody who wants this rolled
-- back decides what those members become, and then runs it again.

DO $$
DECLARE
    holders BIGINT;
BEGIN
    SELECT count(*) INTO holders FROM world_members WHERE role = 'TrustedPlayer';
    IF holders > 0 THEN
        RAISE EXCEPTION
            'cannot remove the TrustedPlayer role: % world member(s) still hold it (ADR-099). Change their role first.',
            holders;
    END IF;
END
$$;

ALTER TABLE world_members DROP CONSTRAINT world_members_role_check;

ALTER TABLE world_members
    ADD CONSTRAINT world_members_role_check
    CHECK (role IN ('Owner', 'GM', 'Player'));
