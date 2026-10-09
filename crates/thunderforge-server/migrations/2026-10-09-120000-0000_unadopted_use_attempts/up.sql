-- Spec 048 FR-034: a player using staged content the GM has not adopted is
-- refused, and the attempt is recorded for the GM, once per window rather
-- than once per click. Attempts suppressed as stale are not stored: they are
-- counted in telemetry only, and are not anyone's record.

CREATE TABLE world_unadopted_use_attempts (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id        uuid NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    actor_id        uuid NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    staged_id       uuid NOT NULL REFERENCES world_staged_content(id) ON DELETE CASCADE,
    user_id         uuid NOT NULL REFERENCES users(id),
    -- The GraphQL root field.
    operation       varchar(64) NOT NULL,
    first_at        timestamp NOT NULL DEFAULT now(),
    last_at         timestamp NOT NULL DEFAULT now(),
    attempts        integer NOT NULL DEFAULT 1,
    reported        boolean NOT NULL,
    chat_message_id uuid REFERENCES world_chat_messages(id) ON DELETE SET NULL,
    created_by      uuid NOT NULL REFERENCES users(id),
    updated_by      uuid NOT NULL REFERENCES users(id)
);
CREATE INDEX unadopted_attempts_window
    ON world_unadopted_use_attempts (actor_id, staged_id, last_at DESC);
