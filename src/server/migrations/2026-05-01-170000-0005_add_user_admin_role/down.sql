DROP INDEX IF EXISTS idx_users_is_admin;

ALTER TABLE users DROP COLUMN is_admin;