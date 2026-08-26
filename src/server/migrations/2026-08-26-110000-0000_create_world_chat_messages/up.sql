-- Play-view Chat: world-scoped, persisted messages.
--
-- Deliberately NOT a new network transport. The repo already has a
-- world-scoped realtime bus — `world_events` + `pg_notify` fanned out to
-- the `worldEventsCreated(worldId)` GraphQL subscription every world
-- member already holds open (src/server/src/world_events.rs). Chat rides
-- that with its own event code, exactly as spec 018's Genie session state
-- does. This table is the history/backscroll behind it; the bus only
-- carries the "something changed" nudge.
--
-- `body` is stored raw and rendered as plain text by the client — no
-- markdown/HTML pipeline here, so there is no stored-XSS surface to
-- sanitize on read.
CREATE TABLE world_chat_messages (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    -- Scene-scoped when set, world-wide when NULL. Nullable rather than
    -- NOT NULL: a message sent from staging (no scene launched yet) still
    -- belongs to the world's log.
    scene_id UUID NULL REFERENCES scenes(scene_id) ON DELETE SET NULL,
    author_user_id UUID NOT NULL REFERENCES users(id),
    -- Denormalized display name, captured at send time. Chat history must
    -- keep reading correctly after a rename, and a JOIN per message on a
    -- hot backscroll query is the wrong trade here.
    author_label TEXT NOT NULL,
    body TEXT NOT NULL,
    -- GM-only whisper: hidden from non-GM members. Enforced server-side in
    -- the query, never by the client omitting it.
    gm_only BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- The backscroll query is "latest N for this world", so the index is
-- (world_id, created_at DESC) rather than world_id alone.
CREATE INDEX world_chat_messages_world_created_idx
    ON world_chat_messages (world_id, created_at DESC);
