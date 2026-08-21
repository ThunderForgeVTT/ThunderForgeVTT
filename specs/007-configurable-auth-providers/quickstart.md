# Quickstart: Configurable Multi-Provider Authentication

Validation scenarios for this feature, run against a live local dev stack (`pnpm dev`, per this repo's usual flow). Each scenario maps to an acceptance scenario in spec.md — see there for the full Given/When/Then; this is the runnable version.

## Prerequisites

- Local dev stack running (`pnpm dev`) with a fresh or existing database, migrations applied (including this feature's `config_source` column and any new preset seed rows).
- A `.env` (or equivalent process-env mechanism) you can edit and restart the server against.

## Scenario 1 — Zero-touch provider enablement (US1)

1. With no `OAUTH_*` env vars set, load the sign-in screen. **Expect**: only username/password sign-in/sign-up, no OAuth buttons.
2. Set `OAUTH_DISCORD_CLIENT_ID` and `OAUTH_DISCORD_CLIENT_SECRET` (real or throwaway test app credentials), restart the server.
3. Reload the sign-in screen. **Expect**: a "Log in with Discord" button now renders. Query `oauthProviders` as an admin — the `discord` row shows `configSource: ENV`, `configured: true`.
4. Unset the env vars, restart. **Expect**: the button disappears again; the admin query still shows the `discord` row (now `configSource: ADMIN`), with its previously-env-sourced credentials still present but now admin-editable (research.md §6).

## Scenario 2 — Multi-instance (US1, FR-012)

1. Set `OAUTH_KEYCLOAK_ISSUER_URL`, `OAUTH_KEYCLOAK_CLIENT_ID`, `OAUTH_KEYCLOAK_CLIENT_SECRET` for a default Keycloak instance, **and** `OAUTH_KEYCLOAK_WORK_ISSUER_URL`, `OAUTH_KEYCLOAK_WORK_CLIENT_ID`, `OAUTH_KEYCLOAK_WORK_CLIENT_SECRET`, `OAUTH_KEYCLOAK_WORK_LABEL="Work SSO"` for a second, named instance. Restart.
2. Reload the sign-in screen. **Expect**: two distinct Keycloak login buttons — one "Log in with Keycloak", one "Log in with Work SSO" — each completing a real login independently against its own realm.

## Scenario 3 — Generic/unlisted provider (US1, FR-002)

1. Pick any OAuth2-compliant provider with no built-in preset. Set `OAUTH_MYSERVICE_AUTHORIZATION_URL`, `OAUTH_MYSERVICE_TOKEN_URL`, `OAUTH_MYSERVICE_CLIENT_ID`, `OAUTH_MYSERVICE_CLIENT_SECRET` (and `_USERINFO_URL`/`_SCOPES` if needed). Restart.
2. Reload the sign-in screen. **Expect**: a "Log in with Myservice" button renders and completes a real login, exactly like a built-in preset would.

## Scenario 4 — Username/password never displaced (US2)

1. With several OAuth providers configured (from Scenarios 1-3), reload the sign-in screen. **Expect**: username/password sign-up and sign-in are still presented, not buried below or hidden behind the provider buttons.
2. Sign up with username/password. Then, as the same real-world person, log in via one of the configured OAuth providers using an account you can link. **Expect**: existing account-linking behavior applies — no duplicate user record.

## Scenario 5 — Admin panel runtime configuration (US3)

1. As an admin, open the OAuth provider admin screen. Add credentials for a provider with no env vars set (e.g. GitHub, if not already env-configured). Save.
2. Reload the sign-in screen (no server restart). **Expect**: the GitHub button now renders.
3. On the admin screen, view the `discord` row from Scenario 1 (still `configSource: ENV`, assuming its env vars are still set). **Expect**: its credential/URL fields render read-only/masked; the `enabled` toggle is still interactive.
4. Toggle that row's `enabled` off, save. Reload the sign-in screen. **Expect**: the Discord button disappears immediately, no restart. Restart the server (env vars unchanged). **Expect**: the Discord button stays gone — the admin's `enabled = false` was not reset by the startup env-var scan (research.md §3).

## Scenario 6 — Custom label (US4)

1. Set `OAUTH_KEYCLOAK_LABEL="Thicc Dungeon"` alongside valid default-instance Keycloak credentials. Restart.
2. Reload the sign-in screen. **Expect**: the button reads "Log in with Thicc Dungeon" instead of "Log in with Keycloak".
3. Repeat via the admin panel's label field on an admin-sourced (non-env) provider row instead. **Expect**: same result, no env var involved.

## Scenario 7 — Secret never leaks (SC-004)

1. With any provider configured, open browser devtools network tab, load the sign-in screen and the admin panel (as admin).
2. Inspect every response body. **Expect**: no `oauthClientSecret` value ever appears in any response — only `hasClientSecret: true/false`.

## Scenario 8 — Pre-existing admin-configured provider survives upgrade (US3, FR-014, SC-007)

1. On a database that predates this feature (or a fresh DB with this feature's migrations rolled back to just before `config_source` is added), configure a provider through the admin panel exactly as it worked pre-feature (e.g. fill in real GitHub credentials, enable it). Confirm it authenticates successfully.
2. Apply this feature's `config_source` migration (and any later ones) without changing that row by hand.
3. Reload the sign-in screen and admin panel. **Expect**: the GitHub button still renders and still completes a real login, with zero re-entry of credentials. The admin panel shows that row's `configSource: ADMIN` (the migration's default), not `ENV`.
