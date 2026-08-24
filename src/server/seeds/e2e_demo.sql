-- Idempotent e2e/dev seed: a demo user + demo world, ready to launch
-- straight into the play engine, and a completed admin bootstrap so a
-- freshly-migrated (or docker-compose-down--v'd) database never blocks on
-- the one-time /setup/<code> wizard.
--
-- Safe to re-run: every insert is keyed on a fixed UUID with
-- ON CONFLICT DO NOTHING, so running this against a DB that already has
-- the seed is a no-op.
--
-- Applied by apps/web/e2e/fixtures/global-setup.ts before the e2e suite
-- runs. To apply by hand:
--   psql "$DATABASE_URL" -f src/server/seeds/e2e_demo.sql

-- Reset any Genie session-loop state left over from a previous e2e run
-- against this same fixed-UUID demo world (spend_wish/advance_doom_clock/
-- etc. mutate real rows, so re-running the suite against a not-yet-wiped
-- DB would otherwise start from wherever the last run left off instead of
-- a clean session).
DELETE FROM world_genie_puzzle_clocks
WHERE session_id IN (SELECT id FROM world_genie_sessions WHERE world_id = '00000000-0000-0000-0000-0000000000f0');
DELETE FROM world_genie_resource_holdings
WHERE session_id IN (SELECT id FROM world_genie_sessions WHERE world_id = '00000000-0000-0000-0000-0000000000f0');
DELETE FROM world_genie_trade_proposals
WHERE session_id IN (SELECT id FROM world_genie_sessions WHERE world_id = '00000000-0000-0000-0000-0000000000f0');
DELETE FROM world_genie_sessions WHERE world_id = '00000000-0000-0000-0000-0000000000f0';

-- Skip the first-run admin bootstrap wizard entirely.
INSERT INTO admin_bootstrap_setup (id, setup_completed_at, admin_code_hash, admin_code_generated_at, created_at, updated_at)
VALUES (1, now(), NULL, NULL, now(), now())
ON CONFLICT (id) DO UPDATE SET setup_completed_at = now();

-- Demo user. Password is "Sup3r-Secret-Passphrase!" (matches the
-- convention used by apps/web/e2e/fixtures/helpers.ts's freshCredentials),
-- hashed with the same Argon2 params as auth::hash_password.
INSERT INTO users (id, username, password_hash, email, created_at, updated_at, two_factor_enabled, two_factor_admin_required, is_admin)
VALUES (
  '00000000-0000-0000-0000-0000000000e2',
  'e2edemo',
  '$argon2id$v=19$m=19456,t=2,p=1$niEwA63DF+T39rY601qniQ$r0q7cdblJI4nH9jsOohucWwiYaWLtXKAqDxvq62Bj+s',
  'e2edemo@example.test',
  now(),
  now(),
  false,
  false,
  false
)
ON CONFLICT (id) DO NOTHING;

-- Demo world, explicitly on the Genie game system (also now the
-- server-side default for any world created with no system selected —
-- see prepare_world_input in src/server/src/graphql/helpers.rs).
INSERT INTO worlds (id, name, created_at, updated_at, created_by, updated_by, description, game_system_id, allow_player_created_actors)
VALUES (
  '00000000-0000-0000-0000-0000000000f0',
  'Genie Demo World',
  now(),
  now(),
  '00000000-0000-0000-0000-0000000000e2',
  '00000000-0000-0000-0000-0000000000e2',
  NULL,
  'genie',
  false
)
ON CONFLICT (id) DO NOTHING;

-- Default scene, matching what create_world_impl auto-creates for a new
-- world (battlemap, square grid) so /world/<id>/play has something to
-- render immediately.
INSERT INTO scenes (scene_id, world_id, name, description, type, grid_size, grid_type, width, height, metadata, owner_id, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-0000000000f1',
  '00000000-0000-0000-0000-0000000000f0',
  'Genie Demo World',
  NULL,
  'battlemap',
  5,
  'square',
  100,
  100,
  NULL,
  '00000000-0000-0000-0000-0000000000e2',
  now(),
  now()
)
ON CONFLICT (scene_id) DO NOTHING;

-- Spec 019: a richer demo world — a second player, a few NPCs spanning
-- size categories, two PCs (one leveled), items, and an inventory grant —
-- so movement, trading, and inventory can be exercised by hand instead of
-- starting from an empty world.
--
-- world_actor_system_data JSONB columns (resource_data/trait_data) are
-- only validated on the GraphQL mutation path (packs/systems/genie/server/src/validators.rs),
-- not by this raw INSERT — the values below are hand-checked against
-- that file's allowed-value lists so they don't silently plant data the
-- app's own mutation path would have rejected:
--   trait_data.size_category: diminutive|small|medium|large|huge|colossal
--   trait_data.level: integer 1-10
--   resource_data: max_health >= 1, max_wish_points >= 0 (both required)

-- Second demo user + world membership (require_world_member only falls
-- back to worlds.created_by for the *creator* — a second player needs an
-- explicit world_members row to have access at all).
INSERT INTO users (id, username, password_hash, email, created_at, updated_at, two_factor_enabled, two_factor_admin_required, is_admin)
VALUES (
  '00000000-0000-0000-0000-0000000000e3',
  'e2edemo2',
  '$argon2id$v=19$m=19456,t=2,p=1$niEwA63DF+T39rY601qniQ$r0q7cdblJI4nH9jsOohucWwiYaWLtXKAqDxvq62Bj+s',
  'e2edemo2@example.test',
  now(),
  now(),
  false,
  false,
  false
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO world_members (id, world_id, user_id, role, joined_at, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-000000000600',
  '00000000-0000-0000-0000-0000000000f0',
  '00000000-0000-0000-0000-0000000000e3',
  'Player',
  now(),
  now(),
  now()
)
ON CONFLICT (id) DO NOTHING;

-- Items.
INSERT INTO world_items (id, world_id, name, description, created_by, created_at, updated_at)
VALUES
  (
    '00000000-0000-0000-0000-000000001001',
    '00000000-0000-0000-0000-0000000000f0',
    'Lamp of Minor Binding',
    'A tarnished brass lamp that suppresses a bound Genie''s power while held.',
    '00000000-0000-0000-0000-0000000000e2',
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-000000001002',
    '00000000-0000-0000-0000-0000000000f0',
    'Cloak of the Wish-Warped',
    'A shifting cloak that hums faintly near gridless, reality-thin places.',
    '00000000-0000-0000-0000-0000000000e2',
    now(),
    now()
  )
ON CONFLICT (id) DO NOTHING;

-- NPCs spanning size categories, to exercise TokenPanel's scale hint.
INSERT INTO world_actors (id, world_id, scene_id, actor_type, game_system_id, label, created_by, owned_by, is_public, is_npc, created_at, updated_at, description, available_for_claim)
VALUES
  (
    '00000000-0000-0000-0000-000000002001',
    '00000000-0000-0000-0000-0000000000f0',
    '00000000-0000-0000-0000-0000000000f1',
    'character',
    'genie',
    'Flickering Sprite',
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    true,
    true,
    now(),
    now(),
    'A diminutive, mischievous minor spirit.',
    false
  ),
  (
    '00000000-0000-0000-0000-000000002002',
    '00000000-0000-0000-0000-0000000000f0',
    '00000000-0000-0000-0000-0000000000f1',
    'character',
    'genie',
    'Vault Warden',
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    true,
    true,
    now(),
    now(),
    'A medium-sized construct guarding the escape room''s exit.',
    false
  ),
  (
    '00000000-0000-0000-0000-000000002003',
    '00000000-0000-0000-0000-0000000000f0',
    '00000000-0000-0000-0000-0000000000f1',
    'character',
    'genie',
    'Towering Elemental Servant',
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    true,
    true,
    now(),
    now(),
    'A colossal bound elemental, the encounter''s centerpiece threat.',
    false
  )
ON CONFLICT (id) DO NOTHING;

INSERT INTO world_actor_system_data (id, actor_id, game_system_id, ability_data, resource_data, proficiency_data, trait_data, created_by, updated_by, created_at, updated_at)
VALUES
  (
    '00000000-0000-0000-0000-000000005001',
    '00000000-0000-0000-0000-000000002001',
    'genie',
    '{"might": 1, "cunning": 4, "spirit": 3}'::jsonb,
    '{"current_wish_points": 0, "max_wish_points": 0, "current_health": 3, "max_health": 3}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"size_category": "diminutive"}'::jsonb,
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-000000005002',
    '00000000-0000-0000-0000-000000002002',
    'genie',
    '{"might": 4, "cunning": 2, "spirit": 2}'::jsonb,
    '{"current_wish_points": 0, "max_wish_points": 0, "current_health": 12, "max_health": 12}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"size_category": "medium"}'::jsonb,
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-000000005003',
    '00000000-0000-0000-0000-000000002003',
    'genie',
    '{"might": 6, "cunning": 1, "spirit": 3}'::jsonb,
    '{"current_wish_points": 0, "max_wish_points": 0, "current_health": 30, "max_health": 30}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"size_category": "colossal", "active_conditions": ["bound"]}'::jsonb,
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    now(),
    now()
  )
ON CONFLICT (actor_id) DO NOTHING;

-- Two more PCs — one per demo user, one leveled up to exercise the
-- leveling UI (spec 019).
INSERT INTO world_actors (id, world_id, scene_id, actor_type, game_system_id, label, created_by, owned_by, is_public, is_npc, created_at, updated_at, description, available_for_claim)
VALUES
  (
    '00000000-0000-0000-0000-000000003001',
    '00000000-0000-0000-0000-0000000000f0',
    '00000000-0000-0000-0000-0000000000f1',
    'character',
    'genie',
    'Rin the Unbound',
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    false,
    false,
    now(),
    now(),
    'A level 3 Genie, still learning the shape of their own wishes.',
    false
  ),
  (
    '00000000-0000-0000-0000-000000003002',
    '00000000-0000-0000-0000-0000000000f0',
    '00000000-0000-0000-0000-0000000000f1',
    'character',
    'genie',
    'Cass Emberlight',
    '00000000-0000-0000-0000-0000000000e3',
    '00000000-0000-0000-0000-0000000000e3',
    false,
    false,
    now(),
    now(),
    'A freshly-bound Genie, level 1.',
    false
  )
ON CONFLICT (id) DO NOTHING;

INSERT INTO world_actor_system_data (id, actor_id, game_system_id, ability_data, resource_data, proficiency_data, trait_data, created_by, updated_by, created_at, updated_at)
VALUES
  (
    '00000000-0000-0000-0000-000000005004',
    '00000000-0000-0000-0000-000000003001',
    'genie',
    '{"might": 2, "cunning": 5, "spirit": 4}'::jsonb,
    '{"current_wish_points": 3, "max_wish_points": 4, "current_health": 10, "max_health": 10}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"level": 3}'::jsonb,
    '00000000-0000-0000-0000-0000000000e2',
    '00000000-0000-0000-0000-0000000000e2',
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-000000005005',
    '00000000-0000-0000-0000-000000003002',
    'genie',
    '{"might": 3, "cunning": 3, "spirit": 3}'::jsonb,
    '{"current_wish_points": 2, "max_wish_points": 2, "current_health": 8, "max_health": 8}'::jsonb,
    '{"trained_skills": []}'::jsonb,
    '{"level": 1}'::jsonb,
    '00000000-0000-0000-0000-0000000000e3',
    '00000000-0000-0000-0000-0000000000e3',
    now(),
    now()
  )
ON CONFLICT (actor_id) DO NOTHING;

-- Give Rin one of the seeded items, so inventory/trading UI has
-- something to show immediately.
INSERT INTO world_actor_inventory (id, actor_id, item_id, item_name_snapshot, quantity, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-000000004001',
  '00000000-0000-0000-0000-000000003001',
  '00000000-0000-0000-0000-000000001001',
  'Lamp of Minor Binding',
  1,
  now(),
  now()
)
ON CONFLICT (id) DO NOTHING;
