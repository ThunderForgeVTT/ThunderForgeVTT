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
