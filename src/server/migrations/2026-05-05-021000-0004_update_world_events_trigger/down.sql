-- Revert to the original trigger (sends full event as JSON)
CREATE OR REPLACE FUNCTION notify_world_event()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('world_events', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Recreate the trigger with the original function
DROP TRIGGER IF EXISTS world_events_notify_trigger ON world_events;
CREATE TRIGGER world_events_notify_trigger
AFTER INSERT ON world_events
FOR EACH ROW EXECUTE PROCEDURE notify_world_event();
