-- Rollback: Drop tokens table
DROP TRIGGER IF EXISTS update_tokens_updated_at ON tokens;
DROP TABLE IF EXISTS tokens CASCADE;
