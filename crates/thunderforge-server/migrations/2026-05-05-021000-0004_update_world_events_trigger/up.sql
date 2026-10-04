-- Update the NOTIFY trigger to send only event_id and use correct channel
CREATE OR REPLACE FUNCTION notify_world_event()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('world_events_channel', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Recreate the trigger with the updated function
DROP TRIGGER IF EXISTS world_events_notify_trigger ON world_events;
CREATE TRIGGER world_events_notify_trigger
AFTER INSERT ON world_events
FOR EACH ROW EXECUTE PROCEDURE notify_world_event();
