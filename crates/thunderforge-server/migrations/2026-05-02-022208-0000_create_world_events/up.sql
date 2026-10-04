CREATE TABLE world_events (
    id BIGSERIAL PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id),
    event_code INTEGER NOT NULL,
    token_event JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION notify_world_event()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('world_events', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_events_notify_trigger
AFTER INSERT ON world_events
FOR EACH ROW EXECUTE PROCEDURE notify_world_event();
