# Feature Specification: Configurable Multi-Provider Authentication

**Feature Branch**: `007-configurable-auth-providers`

**Created**: 2026-08-21

**Status**: Draft

**Input**: User description: "a auth module that supports username and password with sign up OR different auth providers configurable by env vars or owner configuration on a admin panel screen. thing OAUTH_* so OAUTH_DISCORD_* OAUTH_KEYCLOAK_* OAUTH_GITHUB_* and more providers you can think of. if those are defined we present different login buttons so like if i have keycloak we see a login with keycloak but each provider should support a _LABEL= so if i want keycloak to be my thicc dugneon it could be haha"

## Clarifications

### Session 2026-08-21

- Q: How does each provider instance's OAuth redirect/callback URI get determined? → A: Auto-derived, single shared path — each provider instance's callback URL is derived automatically from the app's own base URL plus the instance's key (e.g. `/auth/oauth/callback/<provider-instance-key>`), so an operator only ever has to register one predictable URL per instance with the provider, with no separate redirect-URI setting to configure.
- Q: What happens to already-existing admin-configured provider rows (from before this feature) when it ships? → A: They are treated as that provider's default/unnamed instance under the new multi-instance model automatically — no data migration step, no re-entry required, and no disruption to already-working logins.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Operator enables a new login provider with zero UI setup (Priority: P1)

An operator standing up (or already running) a ThunderForgeVTT instance wants to let their players sign in with an identity provider they already run or trust (Discord, GitHub, their own Keycloak/OIDC server, etc.). They set that provider's credentials as environment variables when deploying the server — nothing else. The next time the sign-in screen loads, a login button for that provider appears automatically, with no admin-panel step required.

**Why this priority**: This is the core value of the feature — an operator's first experience of "configurable by env var" must work with zero clicks in the admin UI, matching how the rest of this app's deployment config already works (12-factor-style env configuration).

**Independent Test**: Set `OAUTH_DISCORD_CLIENT_ID` and `OAUTH_DISCORD_CLIENT_SECRET` (and any other required Discord settings) on a running instance, restart it, and confirm a "Log in with Discord" button now renders on the sign-in screen and completes a real login. Unset them, restart, and confirm the button disappears. Separately, confirm a provider with no built-in preset (an arbitrary OAuth2/OIDC service) can still be enabled by supplying its full endpoint set generically.

**Acceptance Scenarios**:

1. **Given** a fresh instance with no OAuth environment variables set, **When** the sign-in screen loads, **Then** no OAuth provider buttons render — only username/password sign-in and sign-up are available.
2. **Given** `OAUTH_KEYCLOAK_CLIENT_ID`, `OAUTH_KEYCLOAK_CLIENT_SECRET`, and Keycloak's required connection setting are set and the server has been (re)started, **When** the sign-in screen loads, **Then** a "Log in with Keycloak" button renders and clicking it completes a real OAuth login for a new or returning user.
3. **Given** a provider's environment variables are only partially set (e.g. client ID present, client secret missing), **When** the sign-in screen loads, **Then** that provider's button does not render, and the operator can find a clear diagnostic (e.g. in server logs or an admin-panel status view) explaining which required setting is missing.
4. **Given** multiple providers' environment variables are all set at once (e.g. Discord and GitHub), **When** the sign-in screen loads, **Then** a distinct login button renders for each configured provider, alongside username/password.
5. **Given** an operator sets a second, named Keycloak instance's environment variables (e.g. `OAUTH_KEYCLOAK_WORK_CLIENT_ID`/`_CLIENT_SECRET`/`_LABEL`) alongside the default Keycloak instance's variables, **When** the sign-in screen loads, **Then** two distinct Keycloak login buttons render, each labeled and behaving independently.
6. **Given** an operator wants to enable a provider with no built-in preset, **When** they supply the generic template's full endpoint set (authorization URL, token URL, userinfo URL, scopes, client ID/secret) via environment variables, **Then** a working login button renders for it exactly as it would for a built-in preset.

---

### User Story 2 - Username/password sign-in stays a first-class option (Priority: P1)

A player wants to create an account and sign in using just a username and password, regardless of how many (or how few) OAuth providers the operator has configured. Username/password must never be hidden, demoted, or made harder to find just because OAuth providers exist.

**Why this priority**: This is the existing, already-implemented baseline auth path (unified identity model). This feature must not regress it while adding provider flexibility — it's the fallback every instance can rely on even with zero providers configured.

**Independent Test**: On an instance with several OAuth providers configured and visible, confirm a brand-new user can still sign up and log in with only a username and password, and that this path resolves to the same unified user identity as every other auth path.

**Acceptance Scenarios**:

1. **Given** any number of configured OAuth providers (zero or more), **When** the sign-in screen loads, **Then** username/password sign-in and sign-up are always presented, never hidden behind a provider list.
2. **Given** a user who signed up with username/password, **When** they later also log in via a configured OAuth provider using the same identity, **Then** the account linking behavior follows the existing unified-identity rules already established for this app (no new parallel account is created).

---

### User Story 3 - Owner configures and overrides providers from the admin panel (Priority: P2)

A world/instance owner without direct access to the server's deployment environment (or who wants to change something after deploy without a restart) wants to configure an OAuth provider — or adjust one already set via environment variables — from the existing admin panel screen.

**Why this priority**: Extends the existing admin-panel provider management (already built for this app) to work alongside the new env-var path, rather than replacing it — env vars serve operators with deploy-time control, the admin panel serves owners who need runtime control without a redeploy.

**Independent Test**: As an admin, open the provider management screen, add credentials for a provider that has no environment variables set, save, and confirm its login button appears on the sign-in screen without a server restart. Separately, disable a provider that *does* have valid environment variables set, and confirm its button disappears from the sign-in screen while the instance keeps running.

**Acceptance Scenarios**:

1. **Given** an admin viewing the provider management screen, **When** they view a provider instance whose credentials came from environment variables, **Then** the screen clearly indicates that instance's configuration source and shows its values as read-only/masked, since environment variables always take precedence over admin-panel edits for that instance.
2. **Given** an admin adds valid credentials for a provider with no environment variables set, **When** they save, **Then** that provider's login button appears on the sign-in screen without requiring a server restart.
3. **Given** an admin disables a configured provider (regardless of its configuration source), **When** they save, **Then** that provider's login button no longer appears on the sign-in screen until re-enabled.

---

### User Story 4 - Custom branding for a provider's login button (Priority: P3)

An owner wants their login button for a given provider to say something other than the provider's default name — for example, relabeling their self-hosted Keycloak login as "Log in with Thicc Dungeon" instead of the generic "Log in with Keycloak."

**Why this priority**: A nice-to-have personalization layer on top of the core provider-enablement work in User Stories 1 and 3 — valuable but not required for the feature to deliver its main benefit.

**Independent Test**: Set `OAUTH_KEYCLOAK_LABEL` to a custom string alongside valid Keycloak credentials, and confirm the sign-in screen's button displays that custom text instead of the default "Keycloak" label. Separately, set the same custom label via the admin panel's equivalent field and confirm the same result without using the environment variable.

**Acceptance Scenarios**:

1. **Given** `OAUTH_KEYCLOAK_CLIENT_ID`/`OAUTH_KEYCLOAK_CLIENT_SECRET` are set with no `OAUTH_KEYCLOAK_LABEL`, **When** the sign-in screen loads, **Then** the button shows a sensible default label (e.g. "Log in with Keycloak").
2. **Given** `OAUTH_KEYCLOAK_LABEL=Thicc Dungeon` is set alongside valid credentials, **When** the sign-in screen loads, **Then** the button reads "Log in with Thicc Dungeon".
3. **Given** an admin sets a custom label for a provider through the admin panel, **When** the sign-in screen loads, **Then** the button reflects that custom label.

---

### Edge Cases

- What happens when a provider instance is configured via environment variables, an admin also entered admin-panel values for the same instance (which are masked/inert per FR-008), and the operator then removes the environment variables at the next deploy? Does the previously-inert admin-panel configuration now take effect, or does the instance disappear until an admin re-enters it? (Reasonable default: the previously-entered admin-panel values are preserved and take effect immediately once the env vars are gone — nothing the admin typed is silently lost.)
- What happens when required *non-secret* connection settings for a self-hosted/generic-OIDC-style provider (e.g. Keycloak's issuer or base URL) are missing even though client ID/secret are present?
- What happens when a provider's OAuth credentials are valid at server startup but become invalid later (revoked/expired at the provider's end)? The login button should still render; the failure surfaces at login attempt, not at button-render time.
- What happens when an operator sets environment variables using a preset name this app doesn't recognize (a typo, or a not-yet-supported preset)? The system should ignore it safely rather than error the whole sign-in screen, and should surface a diagnostic for the operator — falling back to the generic OAuth2/OIDC template only when the operator has actually supplied a full generic endpoint set, not merely guessing at one from an unrecognized preset name.
- What happens when a returning user's *only* linked identity is a provider instance that has since been disabled or unconfigured? They must have some other way to reach their account (this app's existing account-recovery/admin-assistance rules apply; this feature does not need to invent new ones).
- What happens when two named instances of the same provider type (e.g. `OAUTH_KEYCLOAK_*` and `OAUTH_KEYCLOAK_WORK_*`) each successfully authenticate the same real-world user? They are treated as two independent linked-provider identities (per-instance, not per-provider-type) unless and until the user links them together under this app's existing account-linking rules — this feature does not add automatic cross-instance identity merging.
- ~~What happens when the app can't reliably auto-detect its own public base URL...~~ **Resolved during planning (research.md §4)**: the redirect URI is derived client-side, in the browser, from `window.location.origin` — the address the user's browser actually loaded the app from — not by the server guessing its own public URL. This is immune to reverse-proxy host-forwarding issues by construction; no app-wide base-URL setting is needed.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST allow an operator to enable an OAuth provider entirely through environment variables, with no admin-panel action required, following a per-provider naming convention (`OAUTH_<PROVIDER>_CLIENT_ID`, `OAUTH_<PROVIDER>_CLIENT_SECRET`, plus any other setting that provider requires to connect).
- **FR-002**: System MUST treat OAuth2/OIDC provider support generically — since every provider exchanges credentials through the same shape of API calls (authorization URL, token URL, userinfo URL, scopes) — rather than hard-coding a fixed, closed list. This is expressed as a **fully generic template** (operator supplies every endpoint/scope by hand) that works for any OAuth2/OIDC-compliant provider, **plus a library of built-in presets** (at minimum Discord, GitHub, and Keycloak/generic-OIDC, with Google, GitLab, and other common providers added over time) that pre-fill the well-known endpoints so an operator only has to supply a client ID/secret (and, for self-hosted presets like Keycloak, an issuer/base URL) rather than every endpoint by hand. Adding a new preset to the library is expected to be a low-effort, config-only addition (not a structural change) because all presets share the same underlying generic model.
- **FR-003**: System MUST render a distinct, clearly labeled login button on the sign-in screen for every provider instance that has valid, complete configuration (via environment variables and/or the admin panel), and MUST NOT render a button for a provider instance with missing or incomplete required configuration.
- **FR-004**: System MUST always present username/password sign-in and sign-up on the sign-in screen, independent of how many OAuth providers are configured, and this path MUST resolve to the same unified user identity model already used by every other auth path in this app.
- **FR-005**: System MUST allow an admin/owner to configure a provider instance's credentials and settings through the existing admin panel provider-management screen, for provider instances that have no environment-variable configuration.
- **FR-006**: System MUST allow an admin/owner to enable or disable any configured provider instance (regardless of whether its credentials came from environment variables or the admin panel) from the admin panel, without requiring a server restart for the enable/disable toggle to take effect.
- **FR-007**: System MUST support a per-provider-instance display-label override, settable via an `OAUTH_<PROVIDER>_LABEL` (or `OAUTH_<PROVIDER>_<INSTANCE>_LABEL` for a named additional instance) environment variable and/or an equivalent admin-panel field, that replaces the provider's default name in its sign-in button text.
- **FR-008**: When the same provider instance has settings defined both via environment variables and via the admin panel, **environment variables always win** for any field they set — this matches this app's existing deploy-time-config-is-authoritative convention. The admin panel MAY be used to configure a provider instance that has no environment variables set at all, and for an env-var-configured instance the admin panel MUST display that instance's values as read-only/masked rather than silently accepting edits that would have no effect.
- **FR-009**: System MUST NOT expose configured client secrets (from either configuration source) back to the browser or to any non-admin API response; the admin panel MUST only ever display a masked/redacted representation of an already-set secret.
- **FR-010**: System MUST log or otherwise surface a diagnostic when a provider instance is partially configured (some but not all required settings present) so an operator can identify and fix the gap, without that partial configuration crashing or degrading the rest of the sign-in screen.
- **FR-011**: System MUST continue to enforce this app's existing account-linking and unified-identity rules (one `users` record per person, regardless of auth path) for every provider instance enabled through this feature — this feature does not introduce a second identity model.
- **FR-012**: System MUST support **multiple named instances of the same provider type** configured side-by-side (e.g. two separate self-hosted Keycloak realms), each independently enabled, labeled, and credentialed. The default/unnamed instance of a provider type uses the base naming convention (`OAUTH_KEYCLOAK_*`); each additional named instance uses an operator-chosen instance key inserted into the same convention (e.g. `OAUTH_KEYCLOAK_WORK_CLIENT_ID`, `OAUTH_KEYCLOAK_WORK_CLIENT_SECRET`, `OAUTH_KEYCLOAK_WORK_LABEL`), and each renders as its own distinct login button.
- **FR-013**: System MUST auto-derive each provider instance's OAuth redirect/callback URI from the app's own base URL plus that instance's key (e.g. `/auth/oauth/callback/<provider-instance-key>`) — an operator MUST NOT need to configure a separate redirect-URI setting per instance; they only need to register that one predictable, auto-derived URL with the provider itself.
- **FR-014**: System MUST treat any provider row already configured through the existing admin-panel provider-management screen (prior to this feature) as that provider's default/unnamed instance under the new multi-instance model automatically, with no data migration step and no disruption to logins that already work through it.

### Key Entities *(include if feature involves data)*

- **Auth Provider Instance**: Represents one configured login provider instance (an OAuth/OIDC provider instance, or the built-in username/password path). Key attributes: provider template (a built-in preset like Discord/GitHub/Keycloak, or the fully generic OAuth2/OIDC template), instance key (default/unnamed, or an operator-chosen name distinguishing a second instance of the same template), configuration source (environment variable vs. admin panel — env vars always win per instance), enabled/disabled state, display label, and the connection settings the template requires (client ID, client secret, and template-specific settings such as an issuer/base URL, or the full endpoint set for the generic template). Builds on the existing OAuth provider record already used by this app's admin panel.
- **User Identity**: The existing unified account record every auth path (username/password or any OAuth provider) resolves to. This feature does not change its shape — it only changes how many ways a user can reach it.
- **Linked Provider Identity**: The existing per-user, per-provider link record (e.g. "this user's GitHub account is linked to this user record"). This feature does not change its shape.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator can go from "provider not available" to "working login button" for a supported provider by setting environment variables and restarting the instance alone — no code change and no admin-panel step required.
- **SC-002**: An admin can enable, disable, or relabel a provider from the admin panel and see the sign-in screen reflect that change within one page reload, with no server restart.
- **SC-003**: 100% of sign-in screen loads show username/password as an available option, across every tested combination of zero, one, and multiple configured OAuth providers.
- **SC-004**: Zero configured provider secrets are ever observable in a browser network request, page source, or non-admin API response during testing.
- **SC-005**: An operator or admin who misconfigures a provider (partial settings) can identify what's missing from a diagnostic (log line or admin-panel status indicator) without needing to read source code.
- **SC-006**: An operator registering a new provider instance with the external identity provider needs to supply exactly one predictable callback/redirect URL — no trial-and-error or app-code reading required to determine it.
- **SC-007**: Every provider instance configured through the admin panel before this feature shipped continues to authenticate users successfully immediately after upgrade, with zero admin re-entry of credentials required.

## Assumptions

- The existing `oauth_providers` persistence model, unified-identity model (ADR-001), and admin GraphQL provider-management surface (ADR-017) are the foundation this feature extends, not replaces — this feature adds an environment-variable configuration source, per-provider built-in templates, and a multi-instance model on top of them. Any provider row already configured before this feature ships becomes that provider's default instance automatically, with no migration step.
- Environment variables are deploy-time configuration, consistent with how the rest of this app's server configuration already works — changing them requires restarting the server process; this feature does not need to add live-reload of the process environment itself. The admin panel is the path for changes that must take effect without a restart.
- "Provider" in this feature means an OAuth2/OIDC-based external identity provider. Non-OAuth auth methods (e.g. SAML, magic links) are out of scope.
- Provider template definitions (which URLs/settings each named provider type needs) are maintained by this app's own codebase, not fetched dynamically from each provider at runtime. Since every OAuth2/OIDC provider shares the same underlying request shape, built-in presets (Discord, GitHub, Keycloak/generic-OIDC, and others added over time) are a convenience layer over one fully generic template — adding a new preset is expected to be a low-effort, config-only addition, not a structural change. An operator can always fall back to the fully generic template for any provider without a built-in preset.
- Multiple named instances of the same provider template are supported from day one (e.g. two separate Keycloak realms), each independently configured, labeled, and rendered as its own login button — this is a first-class part of the environment-variable naming convention and the admin panel, not a later addition.
- Environment-variable configuration always takes precedence over admin-panel configuration for the same provider instance's fields; the admin panel is for instances with no environment-variable configuration, or for viewing (read-only/masked) an env-var-configured instance's values.
- Existing bootstrap-admin and OAuth-linking-safety rules (ADR-006, ADR-008) continue to apply unchanged; this feature does not alter who is allowed to become an admin or how account linking is authorized.
