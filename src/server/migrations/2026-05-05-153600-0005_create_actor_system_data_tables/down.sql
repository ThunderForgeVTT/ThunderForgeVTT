-- Rollback Phase 4.8.1 tables

DROP TRIGGER IF EXISTS actor_system_data_notify_trigger ON world_actor_system_data;
DROP FUNCTION IF EXISTS audit_actor_system_data_change();

DROP TABLE IF EXISTS world_actor_system_data;
DROP TABLE IF EXISTS world_actors;

