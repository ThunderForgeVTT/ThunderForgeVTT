-- Restoring the constraint is conditional, and deliberately so.
--
-- By the time this runs, rows may legitimately hold a type a game system
-- declared — `enchantment`, say. Re-adding the constraint unconditionally
-- would either fail with a message about a check violation that names no
-- cause, or tempt whoever hits it into deleting a Game Master's abilities to
-- make a schema fit.
--
-- So: restore it only if every row still holds a built-in, and otherwise fail
-- loudly with the reason. A down migration that cannot run cleanly should say
-- why rather than destroy content to succeed.
DO $$
DECLARE
    foreign_types text;
BEGIN
    SELECT string_agg(DISTINCT classification, ', ')
      INTO foreign_types
      FROM world_abilities
     WHERE classification NOT IN ('spell', 'feat', 'power', 'talent');

    IF foreign_types IS NOT NULL THEN
        RAISE EXCEPTION
            'Cannot restore the classification CHECK: abilities exist of system-declared types (%). Re-type or remove them first — this migration will not delete content to fit a constraint.',
            foreign_types;
    END IF;

    ALTER TABLE world_abilities
        ADD CONSTRAINT world_abilities_classification_check
        CHECK (classification IN ('spell', 'feat', 'power', 'talent'));
END $$;
