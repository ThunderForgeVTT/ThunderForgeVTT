-- Spec 049 (FR-040 to FR-047, FR-050 to FR-057) and spec 050 (FR-001 to
-- FR-006): a book that has been read in, and everything that came out of it.
--
-- Two tables and one Postgres enum. The shape here is the one decision this
-- arc cannot take back cheaply, so it is worth saying plainly what it is:
--
-- **A compendium belongs to an account, never to a world** (049 FR-040,
-- decision 1; research §7). A Game Master with eight worlds and one Monster
-- Manual owns one Monster Manual, and the world's link to it — spec 050's
-- book list — is a separate, world-owned row that arrives in this same arc.
-- There is deliberately no `world_id` column and no "account-owned for now":
-- a world-scoped table would have had to be migrated out from under every
-- entry, every delta and every inherited scene that referenced it.
--
-- **The uploaded PDF is not here, and never will be.** Reading happens in the
-- Game Master's own browser (FR-020) and the file does not leave their
-- machine. What is stored is what was read out of it, plus the SHA-256 that
-- lets a re-import be recognised (FR-047). There is no column for the bytes
-- because there is nothing to put in one.

-- The two ways a piece of content can have come to exist, and the whole of
-- the sharing test (049 FR-050, decision 4).
--
-- An enum rather than a VARCHAR because the set is closed and a third value
-- is not a thing somebody should be able to invent with an INSERT. Content
-- shipped in a system pack is the third thing that is neither of these
-- (FR-050b), and it is not in this table at all — the platform distributes it
-- under the pack's own `legal` block, so it never needs a row here to say so.
--
-- PascalCase values follow the convention "PolicyEffect" and
-- "CanvasImageAssetKind" already set; `diesel-derive-enum` defaults to
-- snake_case and has to be told otherwise, which the Rust side does.
CREATE TYPE "ContentOrigin" AS ENUM ('Authored', 'Uploaded');

CREATE TABLE compendiums (
    id UUID PRIMARY KEY,

    -- The shelf this sits on (049 FR-040, 050 FR-001).
    --
    -- ON DELETE CASCADE is decision 3 made literal: when an account goes, its
    -- compendiums go with it. That is what lets "delete my data" be said
    -- without a caveat about a shared base kept behind the scenes — the
    -- reason cross-account deduplication was rejected rather than deferred.
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- What the Game Master calls the book (FR-041). Not the filename: the
    -- file is not kept, and a person names their shelf after the book.
    book_title VARCHAR(300) NOT NULL,

    -- SHA-256 of the file, hex, as spec 047 FR-070 to FR-075 already hash it
    -- in the browser before any upload.
    --
    -- The UNIQUE below is per owner and not global, and the distinction is
    -- the whole of decision 1: two accounts that upload the identical file
    -- get two compendiums, deliberately. A global unique constraint would be
    -- the first half of a cross-account store, which needs a possession
    -- protocol, a DMCA determination and an accountable owner's signature
    -- before anybody writes a line of it.
    --
    -- Within one account it is an identity: FR-047's re-import check is
    -- against the account's library, and "you already have this book" is only
    -- answerable if the answer cannot be two rows.
    source_hash VARCHAR(64) NOT NULL,

    -- Which game system the book was read as (FR-041). Decides which worlds
    -- may ever switch it on (050 FR-041) — a Pathfinder book read into a
    -- Pathfinder world says nothing intelligible to a 5e one.
    --
    -- A column, never a value: no system's identifier appears anywhere in
    -- shared code, and `scripts/check-system-registry.mjs` fails the build if
    -- one does.
    system_id VARCHAR(64) NOT NULL,

    -- Where this content came from, and therefore whether it may ever leave
    -- the account (FR-051).
    --
    -- Everything this spec produces is 'Uploaded', with nobody asked and no
    -- answer to get wrong. There is no DEFAULT: a default is a value the
    -- write path can forget to think about, and this is the one column every
    -- sharing rule is enforced against. The insert states it, the trigger
    -- below stops it ever being anything else, and the Rust side has no field
    -- a caller could set (see `compendium::store::NewCompendium`).
    origin "ContentOrigin" NOT NULL,

    -- Which build of the reader produced this.
    --
    -- So a second read of the same file can tell "the book changed" from "the
    -- reader improved" — two situations that look identical from the entry
    -- counts alone and call for opposite responses.
    parser_version VARCHAR(64) NOT NULL,

    -- FR-005. 51 of 246 measured books are image scans with no text layer at
    -- all, and some books are part scans. A book that was a third silent must
    -- be able to say so months later, on the shelf, and not only in the
    -- review window that is long since closed.
    page_count INTEGER NOT NULL,
    silent_page_count INTEGER NOT NULL,

    -- What the library lists without opening every entry (FR-041, 050 FR-002).
    --
    -- A JSON object of kind to count rather than columns, because the set of
    -- kinds is open: a system declares its own, and a column per kind would
    -- mean a migration every time a pack learned a new word. Read-only
    -- bookkeeping — the entries themselves are the truth, and this is what
    -- keeps a shelf of thirty books from being thirty count(*) queries.
    entry_counts JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Who imported it and when (FR-041).
    --
    -- `created_by`/`created_at` ARE the import's author and moment; there is
    -- deliberately no second `imported_by`/`imported_at` pair beside them.
    -- Two pairs meaning the same thing is two pairs that can disagree, and
    -- the one that gets updated is never the one being read.
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT compendiums_owner_source_hash_unique UNIQUE (owner_user_id, source_hash),

    -- A book cannot have been silent on more pages than it has, and cannot
    -- have a negative number of either. Arithmetic rather than policy, and
    -- cheaper to state here than to discover in a library view showing
    -- "-3 of 0 pages unreadable".
    CONSTRAINT compendiums_page_counts_sane CHECK (
        page_count >= 0
        AND silent_page_count >= 0
        AND silent_page_count <= page_count
    )
);

-- Serves the library: one account's own shelf (050 FR-002), which is the only
-- listing this table has. Nothing here lists compendiums across accounts, and
-- nothing should be added that does — FR-055a says an uploaded book must not
-- become reachable by any other account, and a query shape that spans owners
-- is the first step towards one that forgets to filter.
CREATE INDEX compendiums_owner_user_id_idx ON compendiums(owner_user_id);

-- FR-057: origin is not editable. Not by a Game Master, not by an admin, not
-- by a migration written in a hurry eighteen months from now.
--
-- The Rust store offers no update path to this column, which stops the
-- mistake being made through the application. This trigger is what stops it
-- being made around it — by a repair script, a psql session, or a future
-- resolver that builds its UPDATE from a struct somebody added a field to.
-- The spec's own words: a value that can be flipped is a value that will be
-- flipped, and "uploaded becomes authored" is precisely the flip that turns a
-- publisher's book into something the platform offers to strangers.
--
-- Deliberately a hard error rather than a silent restore of the old value. A
-- write that was refused must look like a refusal from the caller's side;
-- quietly keeping OLD.origin would let a broken write path report success
-- forever.
CREATE OR REPLACE FUNCTION compendiums_origin_is_immutable() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.origin IS DISTINCT FROM OLD.origin THEN
        RAISE EXCEPTION
            'compendium origin is not editable (spec 049 FR-057): % cannot become %',
            OLD.origin, NEW.origin;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER compendiums_origin_immutable_trigger
BEFORE UPDATE ON compendiums
FOR EACH ROW EXECUTE PROCEDURE compendiums_origin_is_immutable();

-- One spell, item, creature, feat or piece of prose.
CREATE TABLE compendium_entries (
    id UUID PRIMARY KEY,

    -- FR-043: every entry names its bucket. CASCADE because FR-044's removal
    -- takes the import's whole contribution and nothing else — an entry
    -- without its compendium is not a lesser entry, it is a dangling one.
    compendium_id UUID NOT NULL REFERENCES compendiums(id) ON DELETE CASCADE,

    -- The system's own word for what this is — spell, item, creature, feat.
    -- Carried, never switched on by shared code (FR-012).
    kind VARCHAR(64) NOT NULL,

    name TEXT NOT NULL,

    -- Whether the reader trusts the name it found (`NameState` in
    -- `content_entry.rs`). A boolean because the state has exactly two values
    -- and a boolean is exactly isomorphic to them; a VARCHAR would admit a
    -- third spelling that nothing knows how to render.
    --
    -- A block whose name could not be read cleanly is still worth keeping
    -- when its declared fields read well, so this marks rather than excludes.
    name_uncertain BOOLEAN NOT NULL DEFAULT FALSE,

    -- One-based, as a person would cite it (FR-043): the point is that a Game
    -- Master can go and look it up in the physical book.
    page INTEGER NOT NULL,

    -- Anchored entries only: the declared fields, each with its certainty.
    --
    -- The serialised form of `BTreeMap<String, ReadValue>`, and the tagged
    -- union survives into storage exactly as it is in memory: a field that
    -- was read stores {"state":"clear","value":"15"}, one the reader doubts
    -- stores "uncertain" with the text exactly as read, and one that was
    -- looked for and not found stores {"state":"unread"} with **no value key
    -- at all**. That last one is FR-002 — absence recorded as absence — and
    -- it is a property of the shape rather than a discipline the write path
    -- has to keep: `ReadValue::Unread` has nowhere to carry a value, so no
    -- serialisation of it can invent one. Not an empty string. Not a zero.
    --
    -- Named `field_values` and not `values`: VALUES is a reserved word in
    -- SQL, and a column that must be quoted at every site is a column that
    -- will eventually not be.
    field_values JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Prose entries only: what it says (FR-001b). NULL for anchored kinds.
    prose_text TEXT,

    -- FR-004: built from lines the layout pass did not trust. A subsetted
    -- font with no usable encoding produces readable-looking nonsense, and
    -- the only reliable detector of that is a person looking at it — so the
    -- distrust has to reach them, at review time and afterwards.
    suspect BOOLEAN NOT NULL DEFAULT FALSE,

    -- Whatever the system's own pack read that a declaration could not
    -- express — a 5e creature's per-attack reach, for instance.
    --
    -- Opaque here exactly as it is in shared Rust: stored, handed back, never
    -- looked inside. It is what lets a pack own a reading only it understands
    -- without shared code learning that system's vocabulary, and FR-027 says
    -- what is committed must be what the review showed, which means the
    -- pack's contribution has to survive the journey too.
    extras JSONB,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT compendium_entries_page_is_one_based CHECK (page >= 1),

    -- FR-001b made structural: a prose entry has no mechanical fields, and
    -- this is the model giving it nowhere to put one. The importer must not
    -- invent fields for prose, and the row cannot hold them even if some
    -- future write path tries.
    --
    -- Stated as an implication rather than an exclusive-or on purpose: an
    -- anchored entry whose every declared field came back unread still has a
    -- populated `field_values` (every declared field appears, including the
    -- ones not found), and an entry that is genuinely both empty and
    -- text-less is a defect for the review to show, not a row to refuse.
    CONSTRAINT compendium_entries_prose_has_no_fields CHECK (
        prose_text IS NULL OR field_values = '{}'::jsonb
    )
);

-- The two reads this table has: everything in one compendium, and everything
-- of one kind in one compendium (FR-042's browse-by-compendium, grouped by
-- kind). The composite serves both, since `compendium_id` leads it.
CREATE INDEX compendium_entries_compendium_id_kind_idx
    ON compendium_entries(compendium_id, kind);
