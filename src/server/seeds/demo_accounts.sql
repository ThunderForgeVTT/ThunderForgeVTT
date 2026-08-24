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
-- satisfies ensure_registration_allowed (an is_admin=true user must
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
