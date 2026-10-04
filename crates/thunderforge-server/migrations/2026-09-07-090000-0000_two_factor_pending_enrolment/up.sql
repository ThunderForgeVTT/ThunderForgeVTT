-- Spec 041 FR-013, governed by ADR-081 (a confirmed second factor is replaced,
-- never disarmed).
--
-- `two_factor_setup_start` wrote the new secret straight over the live one and
-- set `two_factor_enabled = false` in the same statement, on the strength of a
-- password alone. So the only route *off* a second factor was also a route
-- *round* it: anybody holding the password of an enrolled account could start
-- an enrolment, never confirm it, and leave the account back on passwords
-- only — the second factor removed by somebody who proved only the first.
--
-- An enrolment in progress now lives beside the live one instead of on top of
-- it. The live columns are untouched until a code proves the new secret works,
-- at which point the pending value is promoted and cleared.
--
-- Two columns on `users` rather than a table: this shape needs no expiry, no
-- history and no second row per account, and `users` already carries the four
-- columns this pair sits beside. A pending-enrolment *table* is the right home
-- if the flow ever needs attempts or a lifetime; it does not today.
ALTER TABLE users
    ADD COLUMN two_factor_pending_secret_encrypted TEXT,
    ADD COLUMN two_factor_pending_started_at TIMESTAMP;
