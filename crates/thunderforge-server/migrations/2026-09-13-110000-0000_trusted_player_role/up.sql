-- ADR-099: a fourth world role, Trusted Player, between Player and Game
-- Master.
--
-- The column's CHECK is the database's list of roles, and it has to agree
-- with `thunderforge_authz::Role::as_stored`. Without this the server can
-- decide somebody is a Trusted Player and the write is refused underneath
-- it, which surfaces as a failed role change with a constraint name in it.
--
-- The constraint is replaced rather than altered because Postgres cannot
-- alter a CHECK in place. Both statements run in the migration's one
-- transaction, so there is no moment at which the column accepts anything.

ALTER TABLE world_members DROP CONSTRAINT world_members_role_check;

ALTER TABLE world_members
    ADD CONSTRAINT world_members_role_check
    CHECK (role IN ('Owner', 'GM', 'TrustedPlayer', 'Player'));
