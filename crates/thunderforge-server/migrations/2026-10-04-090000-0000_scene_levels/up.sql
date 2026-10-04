-- Scene levels: a scene owns an ordered set of levels, and every placed thing
-- stands on exactly one of them.
--
-- Owner decision 2026-10-03. A tavern with a cellar and an upstairs, or a
-- tavern interior stacked with the street outside it, is ONE scene with
-- several levels rather than several scenes: the party is still in one place
-- as far as combat, chat, the pause and the scene switcher are concerned, and
-- a staircase carries a token from one level to another without anybody
-- changing scene.
--
-- # Why `level_id` is NOT NULL everywhere, from the first commit
--
-- A nullable `level_id` would mean "the entry level, probably" at every read,
-- and each read would have to remember to say so. The visibility rule this
-- column exists for — a player is answered only for a level they stand on —
-- cannot rest on a reader remembering a COALESCE. So every existing scene is
-- given an entry level here, every existing row is put on it, and the column
-- is closed. Nothing nullable survives this migration.
--
-- # Why triggers fill it
--
-- Dozens of insert sites (scene copy, map import, seeds, a few hundred tests)
-- write these tables without knowing levels exist. A BEFORE INSERT trigger
-- that puts a row with no level on its scene's entry level keeps every one of
-- them correct without touching them, and cannot be forgotten by the next
-- insert site somebody writes. The same reasoning puts entry-level creation
-- in a trigger on `scenes`: there are at least four scene-creation paths
-- (createScene, the auto-created Starting Scene, map import, collection
-- copy) and a fifth would have been written without it.

CREATE TABLE scene_levels (
    level_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- Bottom to top, as a Game Master reads a building. Gaps are allowed;
    -- only the order means anything.
    sort_order INT NOT NULL,
    -- Where a token lands when nothing says otherwise, and what a player who
    -- has not placed a token yet is shown. Exactly one per scene.
    is_entry BOOLEAN NOT NULL DEFAULT false,
    -- Not listed to a player who is not standing on it.
    hidden BOOLEAN NOT NULL DEFAULT false,

    -- The same columns a scene carries for its own board, because a level IS
    -- a board. The entry level's copies are kept equal to the scene's by the
    -- trigger below, so the scene's columns go on answering every reader that
    -- predates levels.
    background_image_path TEXT,
    background_asset_id UUID REFERENCES canvas_image_assets(asset_id) ON DELETE SET NULL,
    width INT NOT NULL,
    height INT NOT NULL,
    ambient_light TEXT NOT NULL DEFAULT 'bright',

    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT scene_levels_name_length CHECK (char_length(name) BETWEEN 1 AND 80),
    CONSTRAINT scene_levels_valid_dimensions CHECK (width > 0 AND height > 0),
    CONSTRAINT scene_levels_ambient_light_level
        CHECK (ambient_light IN ('bright', 'dim', 'dark')),
    -- Deferred, so a reorder can pass through a state where two levels share
    -- a position on its way to one where none do.
    CONSTRAINT scene_levels_scene_order UNIQUE (scene_id, sort_order)
        DEFERRABLE INITIALLY DEFERRED,
    -- The target of every composite foreign key below: a placed thing names
    -- its level AND its scene, so it cannot stand on another scene's level.
    CONSTRAINT scene_levels_level_scene UNIQUE (level_id, scene_id)
);

CREATE UNIQUE INDEX scene_levels_one_entry ON scene_levels (scene_id) WHERE is_entry;

-- One entry level per existing scene, carrying the scene's own board.
INSERT INTO scene_levels (
    scene_id, name, sort_order, is_entry, hidden,
    background_image_path, background_asset_id, width, height, ambient_light,
    created_by, updated_by, created_at, updated_at
)
SELECT
    scene_id, 'Ground', 0, true, false,
    background_image_path, background_asset_id, width, height, ambient_light,
    owner_id, owner_id, created_at, updated_at
FROM scenes;

-- Every scene made from now on gets one too.
CREATE FUNCTION create_entry_level() RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO scene_levels (
        scene_id, name, sort_order, is_entry, hidden,
        background_image_path, background_asset_id, width, height, ambient_light,
        created_by, updated_by
    ) VALUES (
        NEW.scene_id, 'Ground', 0, true, false,
        NEW.background_image_path, NEW.background_asset_id, NEW.width, NEW.height,
        NEW.ambient_light, NEW.owner_id, NEW.owner_id
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER scenes_create_entry_level_trigger
AFTER INSERT ON scenes
FOR EACH ROW
EXECUTE FUNCTION create_entry_level();

-- The scene's board columns and its entry level's are one fact stored twice,
-- so one of them has to follow the other. The scene leads: every writer that
-- predates levels (map import, updateScene, setSceneAmbientLight, scene copy)
-- writes the scene, and this carries it to the entry level. Code that edits
-- the entry level writes the scene and lets this do the rest.
CREATE FUNCTION mirror_scene_board_to_entry_level() RETURNS TRIGGER AS $$
BEGIN
    UPDATE scene_levels
    SET background_image_path = NEW.background_image_path,
        background_asset_id = NEW.background_asset_id,
        width = NEW.width,
        height = NEW.height,
        ambient_light = NEW.ambient_light,
        updated_at = CURRENT_TIMESTAMP
    WHERE scene_id = NEW.scene_id AND is_entry;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER scenes_mirror_board_trigger
AFTER UPDATE OF background_image_path, background_asset_id, width, height, ambient_light
ON scenes
FOR EACH ROW
WHEN (
    OLD.background_image_path IS DISTINCT FROM NEW.background_image_path
    OR OLD.background_asset_id IS DISTINCT FROM NEW.background_asset_id
    OR OLD.width IS DISTINCT FROM NEW.width
    OR OLD.height IS DISTINCT FROM NEW.height
    OR OLD.ambient_light IS DISTINCT FROM NEW.ambient_light
)
EXECUTE FUNCTION mirror_scene_board_to_entry_level();

-- A row written with no level stands on the level of the thing it belongs
-- to, and failing that on its scene's entry level.
--
-- "The thing it belongs to" is read from the row itself, by column name, so
-- one function serves all six tables: a light carried by a token stands
-- where the token stands, and a door or prop interactive stands where its
-- wall or token does. A light left on the entry level while its token was
-- upstairs would light the wrong room.
CREATE FUNCTION fill_entry_level() RETURNS TRIGGER AS $$
DECLARE
    subject UUID;
BEGIN
    IF NEW.level_id IS NOT NULL THEN
        RETURN NEW;
    END IF;

    IF TG_TABLE_NAME = 'light_sources' THEN
        subject := NEW.attached_token_id;
        IF subject IS NOT NULL THEN
            SELECT level_id INTO NEW.level_id FROM tokens
            WHERE token_id = subject AND scene_id = NEW.scene_id;
        END IF;
    ELSIF TG_TABLE_NAME = 'interactives' THEN
        subject := NEW.subject_ref;
        IF subject IS NOT NULL THEN
            SELECT level_id INTO NEW.level_id FROM walls
            WHERE wall_id = subject AND scene_id = NEW.scene_id;
            IF NEW.level_id IS NULL THEN
                SELECT level_id INTO NEW.level_id FROM tokens
                WHERE token_id = subject AND scene_id = NEW.scene_id;
            END IF;
            IF NEW.level_id IS NULL THEN
                SELECT level_id INTO NEW.level_id FROM shapes
                WHERE shape_id = subject AND scene_id = NEW.scene_id;
            END IF;
        END IF;
    END IF;

    IF NEW.level_id IS NULL THEN
        SELECT level_id INTO NEW.level_id FROM scene_levels
        WHERE scene_id = NEW.scene_id AND is_entry;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- tokens ---------------------------------------------------------------
ALTER TABLE tokens ADD COLUMN level_id UUID;
UPDATE tokens t SET level_id = l.level_id
FROM scene_levels l WHERE l.scene_id = t.scene_id AND l.is_entry;
ALTER TABLE tokens ALTER COLUMN level_id SET NOT NULL;
-- ON DELETE CASCADE on every one of these, tokens included, because a scene's
-- deletion reaches these rows by two paths at once (scene -> row, and
-- scene -> level -> row) and a NO ACTION check on the second fires before the
-- first has finished. Deleting a level out from under a token is refused in
-- `deleteSceneLevel`, where the Game Master can be told why.
ALTER TABLE tokens ADD CONSTRAINT tokens_level_scene_fkey
    FOREIGN KEY (level_id, scene_id) REFERENCES scene_levels (level_id, scene_id)
    ON DELETE CASCADE;
CREATE INDEX idx_tokens_scene_level ON tokens (scene_id, level_id);
CREATE TRIGGER tokens_fill_entry_level_trigger
BEFORE INSERT ON tokens FOR EACH ROW EXECUTE FUNCTION fill_entry_level();

-- walls ----------------------------------------------------------------
ALTER TABLE walls ADD COLUMN level_id UUID;
UPDATE walls t SET level_id = l.level_id
FROM scene_levels l WHERE l.scene_id = t.scene_id AND l.is_entry;
ALTER TABLE walls ALTER COLUMN level_id SET NOT NULL;
ALTER TABLE walls ADD CONSTRAINT walls_level_scene_fkey
    FOREIGN KEY (level_id, scene_id) REFERENCES scene_levels (level_id, scene_id)
    ON DELETE CASCADE;
CREATE INDEX idx_walls_scene_level ON walls (scene_id, level_id);
CREATE TRIGGER walls_fill_entry_level_trigger
BEFORE INSERT ON walls FOR EACH ROW EXECUTE FUNCTION fill_entry_level();

-- light_sources --------------------------------------------------------
ALTER TABLE light_sources ADD COLUMN level_id UUID;
UPDATE light_sources t SET level_id = l.level_id
FROM scene_levels l WHERE l.scene_id = t.scene_id AND l.is_entry;
ALTER TABLE light_sources ALTER COLUMN level_id SET NOT NULL;
ALTER TABLE light_sources ADD CONSTRAINT light_sources_level_scene_fkey
    FOREIGN KEY (level_id, scene_id) REFERENCES scene_levels (level_id, scene_id)
    ON DELETE CASCADE;
CREATE INDEX idx_light_sources_scene_level ON light_sources (scene_id, level_id);
CREATE TRIGGER light_sources_fill_entry_level_trigger
BEFORE INSERT ON light_sources FOR EACH ROW EXECUTE FUNCTION fill_entry_level();

-- shapes ---------------------------------------------------------------
ALTER TABLE shapes ADD COLUMN level_id UUID;
UPDATE shapes t SET level_id = l.level_id
FROM scene_levels l WHERE l.scene_id = t.scene_id AND l.is_entry;
ALTER TABLE shapes ALTER COLUMN level_id SET NOT NULL;
ALTER TABLE shapes ADD CONSTRAINT shapes_level_scene_fkey
    FOREIGN KEY (level_id, scene_id) REFERENCES scene_levels (level_id, scene_id)
    ON DELETE CASCADE;
CREATE INDEX idx_shapes_scene_level ON shapes (scene_id, level_id);
CREATE TRIGGER shapes_fill_entry_level_trigger
BEFORE INSERT ON shapes FOR EACH ROW EXECUTE FUNCTION fill_entry_level();

-- interactives ---------------------------------------------------------
ALTER TABLE interactives ADD COLUMN level_id UUID;
UPDATE interactives t SET level_id = l.level_id
FROM scene_levels l WHERE l.scene_id = t.scene_id AND l.is_entry;
ALTER TABLE interactives ALTER COLUMN level_id SET NOT NULL;
ALTER TABLE interactives ADD CONSTRAINT interactives_level_scene_fkey
    FOREIGN KEY (level_id, scene_id) REFERENCES scene_levels (level_id, scene_id)
    ON DELETE CASCADE;
CREATE INDEX idx_interactives_scene_level ON interactives (scene_id, level_id);
CREATE TRIGGER interactives_fill_entry_level_trigger
BEFORE INSERT ON interactives FOR EACH ROW EXECUTE FUNCTION fill_entry_level();

-- fog_masks ------------------------------------------------------------
ALTER TABLE fog_masks ADD COLUMN level_id UUID;
UPDATE fog_masks t SET level_id = l.level_id
FROM scene_levels l WHERE l.scene_id = t.scene_id AND l.is_entry;
ALTER TABLE fog_masks ALTER COLUMN level_id SET NOT NULL;
ALTER TABLE fog_masks ADD CONSTRAINT fog_masks_level_scene_fkey
    FOREIGN KEY (level_id, scene_id) REFERENCES scene_levels (level_id, scene_id)
    ON DELETE CASCADE;
-- Fog was one mask per scene. It is one per level now: what the table has
-- uncovered downstairs says nothing about upstairs.
ALTER TABLE fog_masks DROP CONSTRAINT fog_masks_scene_id_key;
ALTER TABLE fog_masks ADD CONSTRAINT fog_masks_scene_level_key UNIQUE (scene_id, level_id);
CREATE TRIGGER fog_masks_fill_entry_level_trigger
BEFORE INSERT ON fog_masks FOR EACH ROW EXECUTE FUNCTION fill_entry_level();

-- Which token asked. A request to travel is a request for one token to
-- travel, and the Game Master's approval has to know which: the requester may
-- control several, and by the time the request is approved the token may have
-- walked off the stairs. Null for every request that is not about a token.
ALTER TABLE interaction_requests
    ADD COLUMN token_id UUID REFERENCES tokens(token_id) ON DELETE SET NULL;
