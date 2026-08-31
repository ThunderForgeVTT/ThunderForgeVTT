-- Two properties a door has that a wall does not.
--
-- `locked` governs *who may change the state*, not the state itself. It is
-- deliberately not a third door state: as one enum, "open, and players cannot
-- close it" — a spiked-open portcullis — becomes inexpressible, and opening a
-- locked door forces a decision about what happens to the lock that a separate
-- flag never raises.
--
-- `secret` affects presentation only. The geometry still reaches every client;
-- it is the drawing that differs. Per the spec, that is a table concern, not a
-- security boundary.
--
-- Both default false, so every wall that exists today stays exactly what it is.
ALTER TABLE walls ADD COLUMN locked BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE walls ADD COLUMN secret BOOLEAN NOT NULL DEFAULT false;
