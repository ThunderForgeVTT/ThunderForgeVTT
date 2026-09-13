-- Spec 049 (FR-050 to FR-057, FR-054a especially), spec 050 (FR-050 to
-- FR-053), ADR-097: uploaded content cannot leave the account that uploaded
-- it, and a collection cannot contain it — by construction.
--
-- Two functions and four triggers. What they replace is worth saying plainly:
-- the alternative was a check in each route that leads outward, and the ADR's
-- own words are that share, publish, export and collection adoption are
-- today's list of routes, not the list. A check in each is lost the first time
-- somebody adds a fifth without reading spec 049. A trigger on the table a
-- route has to write to cannot be forgotten by the route, because the route
-- does not have to remember it.
--
-- **Where the line is drawn.** Everything that carries content out of an
-- account does so by writing one of four kinds of row:
--
-- * `world_collection_members` — a collection is the unit spec 026 shares,
--   publishes, downloads, adopts and rescues, and every one of those reads
--   its content from here;
-- * `world_actor_shares`, `world_item_shares`, `world_ability_shares` — the
--   three single-artifact links that predate collections and publish one
--   thing without one.
--
-- `world_collection_shares` is deliberately not guarded. It shares a
-- collection, and what a collection holds is guarded at the door it came in
-- by; asking again on the way out would be a second answer to the same
-- question, and a second answer is a place for the two to disagree.

-- Where one piece of content came from, asked of a single entry.
--
-- **The meeting point.** Every sharing rule in this arc asks this function,
-- and nothing asks a table's `origin` column directly — so a new kind of
-- content becomes subject to the invariant by gaining an arm here, and not by
-- every guard learning its table. The Rust side calls this same function
-- (`compendium::origin::origin_of`) rather than restating it, so the
-- application's refusal and the database's cannot give different answers.
--
-- **Per entry, never per book** (050 FR-052, FR-052a). An entry is asked
-- about by its own id. For a base entry the answer is its book's, because a
-- book is read in whole and every entry in it arrived by the same path; a
-- world's change to an uploaded entry is uploaded and a world-only addition
-- beside it is authored, and those are rows of their own that answer for
-- themselves.
--
-- **NULL means "not established", never "fine".** Content that does not exist
-- and a content type this function has never heard of both answer NULL, and
-- the guard below refuses NULL. A typo in a member type must not become
-- "nothing restricts this", and a type added next year must be refused until
-- somebody has said where its content comes from.
--
-- **The world's own artifacts are authored**, and the arm says so rather than
-- reading a column, because every write path into those five tables today is
-- an authoring tool or a copy of one (spec 026's adoption copies authored
-- content, and copies of authored content are authored). The first path that
-- writes uploaded content into one of them — spec 048's character-sheet
-- reader, or placing a creature from a book onto a scene as its own actor
-- (049 FR-055) — must give that table an `origin` column and change its arm
-- here to read it, in the same change. Until it does, the arm is true.
--
-- STABLE: it reads, it does not write, and within one statement the answer
-- for one id does not change — origin is immutable wherever it is stored.
CREATE OR REPLACE FUNCTION content_origin(content_type TEXT, content_id UUID)
RETURNS "ContentOrigin" AS $$
BEGIN
    CASE content_type
        WHEN 'compendium' THEN
            RETURN (SELECT origin FROM compendiums WHERE id = content_id);
        WHEN 'compendium_entry' THEN
            RETURN (
                SELECT c.origin
                FROM compendium_entries e
                JOIN compendiums c ON c.id = e.compendium_id
                WHERE e.id = content_id
            );
        -- EXTENSION POINT for spec 050's deltas (world_entry_deltas, which
        -- stores its own per-entry origin): a migration after this one
        -- replaces the function with this arm added —
        --   WHEN 'world_entry_delta' THEN
        --       RETURN (SELECT origin FROM world_entry_deltas WHERE id = content_id);
        -- and `origin::tests` gains the matching case.
        WHEN 'actor' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_actors WHERE id = content_id);
        WHEN 'item' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_items WHERE id = content_id);
        WHEN 'ability' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_abilities WHERE id = content_id);
        WHEN 'lore' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_lore_entries WHERE id = content_id);
        WHEN 'scene' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM scenes WHERE scene_id = content_id);
        ELSE
            RETURN NULL;
    END CASE;
END;
$$ LANGUAGE plpgsql STABLE;

-- The guard: a row that would carry content outward is refused unless that
-- content is known to be authored (049 FR-054a, ADR-097 property 3).
--
-- One function for four tables, told by its trigger arguments where to look:
--
--   ('column',  <type column>, <id column>)   the type is stored on the row
--   ('literal', <content type>, <id column>)  the table only ever holds one
--
-- Read through `to_jsonb(NEW)` because a trigger function cannot name a
-- column that only some of its tables have. The cost is one conversion per
-- inserted row, on tables written a handful of times per share.
--
-- Fires on INSERT, and on UPDATE only of the columns that say *what* the row
-- carries. Reordering a collection must keep working after one of its
-- members has been deleted from its world — spec 026 keeps such rows so a
-- collection stays openable — and a guard on every column would refuse that
-- reorder for content that is merely gone.
--
-- Hard errors, as the compendium trigger's are: a write that was refused must
-- look refused from the caller's side. The message carries a fixed marker,
-- `(spec 049 FR-054a)`, that `compendium::origin::refusal_from_database`
-- recognises, so a refusal that reaches a person through a route nobody
-- thought to phrase still arrives in the words FR-053 and FR-056a require
-- rather than as a database error.
--
-- No role is consulted. There is no session variable, no superuser bypass and
-- no "operator override" argument, because ADR-097 was accepted on exactly
-- that condition: nobody, instance operators included, may reclassify
-- uploaded content as shareable.
CREATE OR REPLACE FUNCTION refuse_content_that_may_not_leave() RETURNS TRIGGER AS $$
DECLARE
    fields JSONB := to_jsonb(NEW);
    kind TEXT;
    target UUID;
    origin "ContentOrigin";
BEGIN
    IF TG_NARGS <> 3 OR TG_ARGV[0] NOT IN ('column', 'literal') THEN
        RAISE EXCEPTION
            'refuse_content_that_may_not_leave on % is misconfigured: expected (column|literal, type, id column)',
            TG_TABLE_NAME;
    END IF;

    kind := CASE TG_ARGV[0]
        WHEN 'column' THEN fields ->> TG_ARGV[1]
        ELSE TG_ARGV[1]
    END;
    target := (fields ->> TG_ARGV[2])::UUID;
    origin := content_origin(kind, target);

    IF origin = 'Uploaded' THEN
        RAISE EXCEPTION USING
            ERRCODE = 'check_violation',
            MESSAGE = format(
                'uploaded content cannot leave the account that uploaded it (spec 049 FR-054a): %s %s is Uploaded',
                kind, target
            );
    END IF;

    IF origin IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = 'check_violation',
            MESSAGE = format(
                'content whose origin cannot be established cannot leave its account (spec 049 FR-054a): no %s %s',
                kind, target
            );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_collection_members_origin_trigger
BEFORE INSERT OR UPDATE OF member_type, member_id ON world_collection_members
FOR EACH ROW EXECUTE PROCEDURE refuse_content_that_may_not_leave('column', 'member_type', 'member_id');

CREATE TRIGGER world_actor_shares_origin_trigger
BEFORE INSERT OR UPDATE OF actor_id ON world_actor_shares
FOR EACH ROW EXECUTE PROCEDURE refuse_content_that_may_not_leave('literal', 'actor', 'actor_id');

CREATE TRIGGER world_item_shares_origin_trigger
BEFORE INSERT OR UPDATE OF item_id ON world_item_shares
FOR EACH ROW EXECUTE PROCEDURE refuse_content_that_may_not_leave('literal', 'item', 'item_id');

CREATE TRIGGER world_ability_shares_origin_trigger
BEFORE INSERT OR UPDATE OF ability_id ON world_ability_shares
FOR EACH ROW EXECUTE PROCEDURE refuse_content_that_may_not_leave('literal', 'ability', 'ability_id');
