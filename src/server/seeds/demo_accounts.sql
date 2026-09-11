-- Local/friend-testing seed: admin/admin, user1/user1, user2/user2, a
-- world (user1 as owner/GM, user2 as a Player member), and a default
-- scene ready to launch straight into the play engine.
--
-- NOT for anything beyond your own machine or a throwaway tunnel demo —
-- these passwords are well under the app's own 12-character minimum
-- (src/server/src/auth/registration.rs's validate_registration_input),
-- so they only work because this seeds the users table directly with a
-- real Argon2id hash, bypassing that check entirely (the same way
-- e2e_demo.sql already does for its own demo accounts). The real
-- register/login HTTP flow would reject "admin" as a password outright.
--
-- Idempotent — every insert is keyed on a fixed UUID with
-- ON CONFLICT DO NOTHING, safe to re-run against a DB that already has
-- this seed. Apply with:
--   psql "$DATABASE_URL" -f src/server/seeds/demo_accounts.sql

-- Skip the first-run admin bootstrap wizard — `admin` below already
-- satisfies ensure_admission_allowed (an is_admin=true user must
-- exist), but this also short-circuits the /setup/<code> UI entirely.
INSERT INTO admin_bootstrap_setup (id, setup_completed_at, admin_code_hash, admin_code_generated_at, created_at, updated_at)
VALUES (1, now(), NULL, NULL, now(), now())
ON CONFLICT (id) DO UPDATE SET setup_completed_at = now();

-- admin / admin — platform admin (is_admin = true).
INSERT INTO users (id, username, password_hash, email, created_at, updated_at, two_factor_enabled, two_factor_admin_required, is_admin)
VALUES (
  '00000000-0000-0000-0000-0000000000a1',
  'admin',
  '$argon2id$v=19$m=19456,t=2,p=1$Ao1+TcV6QJ/ArklQkXpqHA$8uAAk/h3YbvC1CHNNp7y+p6KelT/8DKQUXAvsZPFn3k',
  'admin@example.test',
  now(),
  now(),
  false,
  false,
  true
)
ON CONFLICT (id) DO NOTHING;

-- user1 / user1 — regular user, will own the demo world (GM).
INSERT INTO users (id, username, password_hash, email, created_at, updated_at, two_factor_enabled, two_factor_admin_required, is_admin)
VALUES (
  '00000000-0000-0000-0000-0000000000a2',
  'user1',
  '$argon2id$v=19$m=19456,t=2,p=1$ei82mSmHVkqFN9AZiQDcyQ$4zUntAzTzgi00qkfq60ITiylKXNA3MMk+Gq2uMdLSD4',
  'user1@example.test',
  now(),
  now(),
  false,
  false,
  false
)
ON CONFLICT (id) DO NOTHING;

-- user2 / user2 — regular user, joins the demo world as a Player.
INSERT INTO users (id, username, password_hash, email, created_at, updated_at, two_factor_enabled, two_factor_admin_required, is_admin)
VALUES (
  '00000000-0000-0000-0000-0000000000a3',
  'user2',
  '$argon2id$v=19$m=19456,t=2,p=1$cQ1gCYrfpRTW0B+7fOlcEw$FR4RUyE38FjDfnB1OLiRJN8EtKxJxweBGLiriW6d/ME',
  'user2@example.test',
  now(),
  now(),
  false,
  false,
  false
)
ON CONFLICT (id) DO NOTHING;

-- Demo world, owned by user1, on the Genie system (also the server-side
-- default for any world created with no system selected — see
-- prepare_world_input in src/server/src/graphql/helpers.rs).
INSERT INTO worlds (id, name, created_at, updated_at, created_by, updated_by, description, game_system_id, allow_player_created_actors)
VALUES (
  '00000000-0000-0000-0000-0000000000b0',
  'Demo World',
  now(),
  now(),
  '00000000-0000-0000-0000-0000000000a2',
  '00000000-0000-0000-0000-0000000000a2',
  'Seeded demo world for local/tunnel playtesting.',
  'genie',
  false
)
ON CONFLICT (id) DO NOTHING;

-- The Demo World's session, as every world created on this system gets one
-- (the pack's `start_session_for_new_world` hook, which a raw INSERT above
-- never runs). Without it the seeded world was the one place a Game Master
-- was still shown "Start Genie session" (playtest 2026-09-10 P5).
INSERT INTO world_genie_sessions (id, world_id, doom_clock_max, created_by, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-0000000000b3',
  '00000000-0000-0000-0000-0000000000b0',
  6,
  '00000000-0000-0000-0000-0000000000a2',
  now(),
  now()
)
ON CONFLICT (id) DO NOTHING;

-- user2 as an explicit world member — require_world_member only falls
-- back to worlds.created_by for the *creator* (user1); user2 needs a
-- real world_members row to have access at all.
INSERT INTO world_members (id, world_id, user_id, role, joined_at, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-0000000000b2',
  '00000000-0000-0000-0000-0000000000b0',
  '00000000-0000-0000-0000-0000000000a3',
  'Player',
  now(),
  now(),
  now()
)
ON CONFLICT (id) DO NOTHING;

-- Default scene, matching what create_world_impl auto-creates for a new
-- world (battlemap, square grid) so /world/<id>/play has something to
-- render immediately.
INSERT INTO scenes (scene_id, world_id, name, description, type, grid_size, grid_type, width, height, metadata, owner_id, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-0000000000b1',
  '00000000-0000-0000-0000-0000000000b0',
  'Demo World',
  NULL,
  'battlemap',
  5,
  'square',
  100,
  100,
  NULL,
  '00000000-0000-0000-0000-0000000000a2',
  now(),
  now()
)
ON CONFLICT (scene_id) DO NOTHING;

-- A PC for each of user1/user2, so there's something to select and play
-- immediately instead of an empty roster.
INSERT INTO world_actors (id, world_id, scene_id, actor_type, game_system_id, label, created_by, owned_by, is_public, is_npc, created_at, updated_at, description, available_for_claim)
VALUES
  (
    '00000000-0000-0000-0000-0000000000c1',
    '00000000-0000-0000-0000-0000000000b0',
    '00000000-0000-0000-0000-0000000000b1',
    'character',
    'genie',
    'User1''s Character',
    '00000000-0000-0000-0000-0000000000a2',
    '00000000-0000-0000-0000-0000000000a2',
    false,
    false,
    now(),
    now(),
    NULL,
    false
  ),
  (
    '00000000-0000-0000-0000-0000000000c2',
    '00000000-0000-0000-0000-0000000000b0',
    '00000000-0000-0000-0000-0000000000b1',
    'character',
    'genie',
    'User2''s Character',
    '00000000-0000-0000-0000-0000000000a3',
    '00000000-0000-0000-0000-0000000000a3',
    false,
    false,
    now(),
    now(),
    NULL,
    false
  )
ON CONFLICT (id) DO NOTHING;

INSERT INTO world_actor_system_data (id, actor_id, game_system_id, ability_data, resource_data, proficiency_data, trait_data, created_by, updated_by, created_at, updated_at)
VALUES
  (
    '00000000-0000-0000-0000-0000000000d1',
    '00000000-0000-0000-0000-0000000000c1',
    'genie',
    '{"might": 3, "cunning": 3, "spirit": 3}'::jsonb,
    '{"current_wish_points": 3, "max_wish_points": 3, "current_health": 10, "max_health": 10}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"level": 1}'::jsonb,
    '00000000-0000-0000-0000-0000000000a2',
    '00000000-0000-0000-0000-0000000000a2',
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-0000000000d2',
    '00000000-0000-0000-0000-0000000000c2',
    'genie',
    '{"might": 3, "cunning": 3, "spirit": 3}'::jsonb,
    '{"current_wish_points": 3, "max_wish_points": 3, "current_health": 10, "max_health": 10}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"level": 1}'::jsonb,
    '00000000-0000-0000-0000-0000000000a3',
    '00000000-0000-0000-0000-0000000000a3',
    now(),
    now()
  )
ON CONFLICT (actor_id) DO NOTHING;

-- Spec 035 / ADR-072: a fresh install starts `invite_only`, because the
-- migration branches on whether any user exists and a brand-new database has
-- none. That is right for a real instance and wrong for every stack this seed
-- builds: `make dev` and the e2e harness both migrate an empty database and
-- *then* create these accounts, so the instance they hand over refuses the
-- signup that most of the suite — and most of a first demo — begins with.
--
-- Opening it here rather than changing the migration keeps the product's
-- default fail-shut. This file is already refused against a non-local
-- DATABASE_URL, which is what makes it safe to say `open` in it.
UPDATE instance_access_settings SET access_policy = 'open' WHERE id = 1;

-- Spec 040 FR-026: an instance with no contact for copyright notices publishes
-- nothing beyond a world, so every share link is refused until one is set.
--
-- That is right for a real instance and wrong for every stack this seed
-- builds. `make dev` and the e2e harness both migrate an empty database, and a
-- fresh one has no contact by design — so without this, four share mutations
-- refuse on a dev stack and eleven e2e specs that mint links fail with a
-- message about copyright. This is the same shape as the `access_policy`
-- default above: correct product behaviour, wrong starting point for a
-- machine that exists to be tested on.
--
-- `example.org` rather than a `.invalid` address, and the reason is worth
-- knowing: the validator refuses the reserved TLDs `.local`, `.example`,
-- `.invalid` and `.test`, because mail sent to one can never arrive. That rule
-- is right, and it does not catch `example.org` — a second-level
-- documentation domain reserved by the same RFC, equally undeliverable, and
-- far more commonly typed. Used here deliberately; noted because a real
-- operator could paste one past the check.
--
-- This file is already refused against a non-local DATABASE_URL, which is what
-- keeps a fixture contact out of a real instance.
INSERT INTO instance_settings (key, value, created_at, updated_at)
VALUES
    ('notice.contact_name', 'ThunderForge Development Instance', now(), now()),
    ('notice.contact_email', 'notices@example.org', now(), now()),
    ('notice.contact_postal_address', '1 Development Way, Local Testing', now(), now())
ON CONFLICT (key) DO NOTHING;
