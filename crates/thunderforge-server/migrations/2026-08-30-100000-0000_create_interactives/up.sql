-- Things on a scene that respond: a prop that opens a lore entry, a lever
-- that toggles lights, a door, a region that fires when crossed.
--
-- Scene-scoped. Interactives do not travel between scenes, because the thing
-- they are attached to does not either.
CREATE TABLE interactives (
    interactive_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,

    -- 'prop' | 'door' | 'region'
    subject_kind VARCHAR NOT NULL,

    -- The token for a prop, the wall for a door, null for a region. Not a
    -- foreign key to either table because it is one column pointing at two,
    -- and which one is decided by subject_kind. The cascade that a foreign key
    -- would give is done explicitly on subject deletion instead — a door on a
    -- deleted wall is not a thing.
    subject_ref UUID,

    -- The bounded area, for a region only. Null otherwise.
    geometry JSONB,

    -- The declared effect id, namespaced by contributor ('door.set_state').
    -- Deliberately not constrained: the registry is assembled from what is
    -- compiled in, not from anything this database knows. Null is legitimate —
    -- an interactive with no effect is scenery.
    effect_id VARCHAR,
    effect_config JSONB,

    -- 'click' | 'enter'
    trigger VARCHAR NOT NULL,
    -- 'anyone' | 'gm_only' | 'requires_approval'
    activation VARCHAR NOT NULL,
    -- 'always' | 'once'
    fire_mode VARCHAR NOT NULL DEFAULT 'always',
    -- When a 'once' interactive fired. Null means it has not. Resettable.
    fired_at TIMESTAMP,

    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),

    -- Exactly one of subject_ref and geometry, decided by subject_kind. A
    -- region carrying a subject reference, or a door without one, is invalid
    -- rather than tolerated — and the database is the last place that stays
    -- true when some future caller forgets.
    CONSTRAINT interactives_subject_shape CHECK (
        (subject_kind = 'region' AND subject_ref IS NULL AND geometry IS NOT NULL)
        OR (subject_kind IN ('prop', 'door') AND subject_ref IS NOT NULL AND geometry IS NULL)
    ),

    -- A book cannot be crossed.
    CONSTRAINT interactives_enter_is_region CHECK (
        trigger <> 'enter' OR subject_kind = 'region'
    )
);

-- The read is always "every interactive in this scene", never a scan by
-- effect or subject, so the scene is the leading column.
CREATE INDEX interactives_scene_idx ON interactives (scene_id);

-- Deleting a subject deletes its interactive. Done as an index rather than a
-- constraint because subject_ref points at two tables.
CREATE INDEX interactives_subject_idx ON interactives (subject_ref);
