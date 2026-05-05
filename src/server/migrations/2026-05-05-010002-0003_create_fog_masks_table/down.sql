-- Rollback: Drop fog_masks table
DROP TRIGGER IF EXISTS update_fog_masks_updated_at ON fog_masks;
DROP TABLE IF EXISTS fog_masks CASCADE;
