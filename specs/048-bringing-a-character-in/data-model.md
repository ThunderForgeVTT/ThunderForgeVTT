# Data Model: Bringing a Character In

The design is in research.md: R6 covers the pack, R8 staging, R9 origin, R10
versions and R13 reports. This file gives the shapes.

## Migrations

There are three host migrations in `crates/thunderforge-server/migrations/`,
each with paired `up.sql` and `down.sql`, plus `diesel print-schema` into
`schema.rs`. New tables carry `created_by` and `updated_by`
(Constitution III).

### 1. `2026-10-09-100000-0000_content_origin_columns`

```sql
ALTER TABLE world_actors    ADD COLUMN origin "ContentOrigin" NOT NULL DEFAULT 'Authored';
ALTER TABLE world_items     ADD COLUMN origin "ContentOrigin" NOT NULL DEFAULT 'Authored';
ALTER TABLE world_abilities ADD COLUMN origin "ContentOrigin" NOT NULL DEFAULT 'Authored';
-- FR-033d: no permissive default once the backfill is done.
ALTER TABLE world_actors    ALTER COLUMN origin DROP DEFAULT;
ALTER TABLE world_items     ALTER COLUMN origin DROP DEFAULT;
ALTER TABLE world_abilities ALTER COLUMN origin DROP DEFAULT;

-- Items and abilities: immutable once written.
CREATE FUNCTION forbid_origin_change() RETURNS trigger AS $$
BEGIN
  IF NEW.origin IS DISTINCT FROM OLD.origin THEN
    RAISE EXCEPTION 'origin is immutable' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$ LANGUAGE plpgsql;
-- Actors: Authored -> Uploaded only (an applied import); never back.
CREATE FUNCTION actor_origin_only_uploads() RETURNS trigger AS $$
BEGIN
  IF NEW.origin IS DISTINCT FROM OLD.origin
     AND NOT (OLD.origin = 'Authored' AND NEW.origin = 'Uploaded') THEN
    RAISE EXCEPTION 'actor origin may only become Uploaded' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$ LANGUAGE plpgsql;
-- triggers on each of the three tables (BEFORE UPDATE OF origin)

-- content_origin(kind, id): actor/item/ability read the column.
CREATE OR REPLACE FUNCTION content_origin(...) ... ;  -- redefines 2026-09-13-140000 :92-125
```

`down.sql` restores the earlier `content_origin()`, then drops the triggers,
the functions and the columns.

### 2. `2026-10-09-110000-0000_sheet_import`

```sql
CREATE TYPE "StagedState" AS ENUM ('pending', 'adopted', 'declined');
CREATE TYPE "ActorImportKind" AS ENUM ('import', 'rollback');

CREATE TABLE brought_characters (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  owner_user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  system_id     varchar(64) NOT NULL,
  name          varchar(200) NOT NULL,
  created_at    timestamp NOT NULL DEFAULT now(),
  updated_at    timestamp NOT NULL DEFAULT now(),
  created_by    uuid NOT NULL REFERENCES users(id),
  updated_by    uuid NOT NULL REFERENCES users(id)
);

CREATE TABLE sheet_import_versions (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  character_id        uuid NOT NULL REFERENCES brought_characters(id) ON DELETE CASCADE,
  version_no          integer NOT NULL CHECK (version_no >= 1),
  file_key            text NOT NULL UNIQUE,          -- sheets/{owner}/{character}/{version}.pdf
  file_sha256         char(64) NOT NULL,
  file_bytes          integer NOT NULL CHECK (file_bytes > 0),
  file_pages          smallint NOT NULL CHECK (file_pages > 0),
  reader_id           varchar(64) NOT NULL,          -- e.g. ddb-pdf
  reader_version      varchar(32) NOT NULL,
  reading             jsonb NOT NULL,                -- the server's ImportedCharacter
  corrections         jsonb NOT NULL DEFAULT '{}',   -- the player's, kept apart
  created_at          timestamp NOT NULL DEFAULT now(),
  created_by          uuid NOT NULL REFERENCES users(id),
  updated_by          uuid NOT NULL REFERENCES users(id),
  UNIQUE (character_id, version_no)
);

CREATE TABLE actor_imports (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  world_id         uuid NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  actor_id         uuid NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
  version_id       uuid REFERENCES sheet_import_versions(id) ON DELETE SET NULL,
  kind             "ActorImportKind" NOT NULL,
  restored_from    uuid REFERENCES actor_imports(id),
  before_snapshot  jsonb NOT NULL,   -- sheet fields + links as they were
  written          jsonb NOT NULL,   -- the plan's changes as applied
  plan_hash        char(64),
  applied_at       timestamp NOT NULL DEFAULT now(),
  created_by       uuid NOT NULL REFERENCES users(id),
  updated_by       uuid NOT NULL REFERENCES users(id),
  -- An import names its version when written; the version goes to NULL when
  -- the uploader's account is deleted ("file no longer kept"), so the check
  -- does not demand it (T085, migration 2026-10-10-204800).
  CHECK ((kind = 'import'   AND restored_from IS NULL)
      OR (kind = 'rollback' AND restored_from IS NOT NULL))
);
CREATE INDEX actor_imports_actor ON actor_imports (actor_id, applied_at DESC);

CREATE TABLE world_staged_content (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  world_id            uuid NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  player_user_id      uuid NOT NULL REFERENCES users(id),
  kind                varchar(32) NOT NULL,    -- a vocabulary type or 'item'
  name                varchar(200) NOT NULL,
  normalized_name     varchar(200) NOT NULL,
  content_hash        char(64) NOT NULL,
  field_values        jsonb NOT NULL,
  origin              "ContentOrigin" NOT NULL CHECK (origin = 'Uploaded'),
  state               "StagedState" NOT NULL DEFAULT 'pending',
  differs_from        uuid REFERENCES world_staged_content(id),
  first_actor_id      uuid REFERENCES world_actors(id) ON DELETE SET NULL,
  adopted_ability_id  uuid REFERENCES world_abilities(id),
  adopted_item_id     uuid REFERENCES world_items(id),
  decided_by          uuid REFERENCES users(id),
  decided_at          timestamp,
  created_at          timestamp NOT NULL DEFAULT now(),
  created_by          uuid NOT NULL REFERENCES users(id),
  updated_by          uuid NOT NULL REFERENCES users(id),
  UNIQUE (world_id, kind, normalized_name, content_hash),
  CHECK ((state = 'adopted') = (adopted_ability_id IS NOT NULL OR adopted_item_id IS NOT NULL))
);
CREATE INDEX world_staged_content_queue ON world_staged_content (world_id, state, player_user_id);

ALTER TABLE world_actor_abilities
  ADD COLUMN staged_id  uuid REFERENCES world_staged_content(id),
  ADD COLUMN prepared   boolean,
  ADD COLUMN granted_by varchar(200),   -- "Fighter 2", "Warforged", "Lucky"
  ADD COLUMN uses_max   smallint CHECK (uses_max >= 0),
  ADD COLUMN uses_used  smallint CHECK (uses_used >= 0),
  ADD COLUMN recharge   varchar(16),    -- short_rest | long_rest | dawn | none
  ADD CONSTRAINT one_target CHECK (NOT (ability_id IS NOT NULL AND staged_id IS NOT NULL));
ALTER TABLE world_actor_inventory
  ADD COLUMN staged_id  uuid REFERENCES world_staged_content(id),
  ADD COLUMN equipped   boolean NOT NULL DEFAULT false,
  ADD COLUMN attuned    boolean NOT NULL DEFAULT false,
  ADD CONSTRAINT one_target CHECK (NOT (item_id IS NOT NULL AND staged_id IS NOT NULL));
ALTER TABLE world_items ADD COLUMN weight double precision CHECK (weight >= 0);
-- Implemented as double precision, not numeric(8,2): Diesel's numeric
-- support needs a new dependency, and a sheet's weight is a display value.
```

`world_actor_inventory.item_id` and `world_actor_abilities.ability_id` are
already nullable, so a link to a staged piece needs no change to them. The
`*_name_snapshot` columns keep the name either way. Existing timestamps are
`timestamp` (no zone), and the new tables follow them.

### 3. `2026-10-09-120000-0000_unadopted_use_attempts`

```sql
CREATE TABLE world_unadopted_use_attempts (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  world_id       uuid NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  actor_id       uuid NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
  staged_id      uuid NOT NULL REFERENCES world_staged_content(id) ON DELETE CASCADE,
  user_id        uuid NOT NULL REFERENCES users(id),
  operation      varchar(64) NOT NULL,     -- the GraphQL root field
  first_at       timestamp NOT NULL DEFAULT now(),
  last_at        timestamp NOT NULL DEFAULT now(),
  attempts       integer NOT NULL DEFAULT 1,
  reported       boolean NOT NULL,
  chat_message_id uuid REFERENCES world_chat_messages(id) ON DELETE SET NULL,
  created_by     uuid NOT NULL REFERENCES users(id),
  updated_by     uuid NOT NULL REFERENCES users(id)
);
CREATE INDEX unadopted_attempts_window ON world_unadopted_use_attempts (actor_id, staged_id, last_at DESC);
```

Suppressed-stale attempts are not stored. They count only in telemetry, and
they are not anyone's record.

## State: a staged piece

```
             adopt / adopt all (GM, Trusted Player)
  pending ───────────────────────────────────────► adopted ──► world_abilities / world_items row
     │  ▲                                             (origin Uploaded; links repointed)
     │  │ revisit
     ▼  │
  declined  ─── revisit → pending or adopted
```

- `adopted` is final in this spec. Withdrawing adopted content is ordinary
  compendium deletion, and the existing guard triggers already govern it.
- A Player has no transitions (FR-033b).

## 5e pack: fields added (Q1=C)

All fields are optional unless marked. The validators in
`packs/systems/dnd5e/server/src/validators.rs` gain the rules shown.

| Data type | Field | Shape | Validator rule |
| --- | --- | --- | --- |
| `trait_data` | `classes` | `[{name, subclass?, level, hit_die}]` | 1–20 per class; `hit_die` ∈ d6,d8,d10,d12; if present, `level` = Σ levels and `class` = first name |
| | `age`, `height`, `weight`, `eyes`, `skin`, `hair`, `gender`, `faith` | string ≤ 100 | — |
| | `personality_traits`, `ideals`, `bonds`, `flaws`, `backstory`, `allies_and_organizations` | string ≤ 8000 | — |
| | `resistances`, `immunities`, `vulnerabilities` | string[] of damage types | each ∈ a new `damageTypes` list in `system.json` (the thirteen SRD types); none exists today |
| | `condition_immunities` | string[] of condition ids | each ∈ `conditions` |
| `resource_data` | `hit_dice_pools` | `[{die, total, used}]` | `used ≤ total`; `hit_dice` and `hit_dice_used` derived by refine |
| | `coins` | `{cp, sp, ep, gp, pp}` | each ≥ 0 integer |
| `spell_data` | `spellcasting_classes` | `[{class, ability, save_dc, attack_bonus}]` | `ability` ∈ six abilities |
| | `pact_slots` | `{level, total, used}` | level 1–5; `used ≤ total` |

`abilityVocabulary` gains `feature` and `species_trait`. Both are ungraded
and described in the pack's own words.

`packs/systems/dnd5e/server/src/models.rs:13-48` is deleted. Nothing reads
it (R6).

The 5e sheet (`packs/systems/dnd5e/web/src/ActorSheet.tsx`, 1853 lines)
cannot take these sections inline. They go in new files under
`web/src/sheet/`: `ClassesSection.tsx`, `DefencesSection.tsx`,
`PersonaSection.tsx`, `CoinsSection.tsx` and `LinkedContent.tsx`. The last
one lists linked spells, features and items, and marks staged ones "awaiting
the GM" or "declined by the GM".

## The ImportPlan (server, not stored except as `written`)

| Part | Shape |
| --- | --- |
| `fields` | `[{path, target, old, new, certainty, reason?, source, playState: bool}]` |
| `content` | `[{kind, name, resolution: world{id} \| staged_existing{id} \| staged_new \| differs{id}, link: {prepared?, granted_by?, uses?, equipped?, attuned?}}]` |
| `unmapped` | `[{path, value, goesTo: "notes"}]` |
| `crossChecks` | `[{path, sheet, derived}]` (mismatches only) |
| `keptInPlay` | `[{target, current, sheet}]` (re-import: play-state the plan will not touch unless ticked) |
| `planHash` | sha256 over the canonical JSON of the above |
