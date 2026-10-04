-- Spec 067 FR-032: how a roll came out is stored with the roll.
--
-- `{ "verdict": "...", "label": "..." }` as the system's adjudicator returned
-- it. NULL is a roll nothing judged, which is every roll before this column
-- and every roll of a system with no adjudicator.
ALTER TABLE world_roll_records ADD COLUMN outcome JSONB;
