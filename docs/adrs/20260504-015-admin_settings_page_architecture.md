# ADR-015: Admin Settings Page Architecture

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT needs an authenticated administrative surface for analytics, OAuth configuration, manifest editing, and security policy management. The existing stack already exposes authenticated REST and GraphQL entrypoints plus a fantasy-themed React shell, so a separate admin service would add duplication without solving a current scaling problem.

## Decision

The admin control center will be implemented as a role-guarded React page backed by the existing async-graphql endpoint and session middleware.

- frontend admin routes live under `/admin/*`
- admin pages reuse the main layout and fantasy UI primitives
- admin data is fetched through typed GraphQL helpers in the web app
- the primary control surface is a single settings experience with alias routes for overview, analytics, OAuth, and system entrypoints

## Consequences

### Positive

1. Admin features stay aligned with the existing authentication, session, and UI systems.
2. Metrics, configuration, and security settings share a single page model instead of fragmenting into disconnected tools.
3. Alias routes allow focused navigation without duplicating implementation.

### Negative

1. The GraphQL schema grows to include system-level concerns.
2. The settings page becomes a higher-value target and requires careful authorization checks.

## Alternatives Considered

1. **Separate admin-only REST service** — rejected because it would duplicate session handling, authorization, and data access for a small MVP surface.
2. **Static configuration files with no UI** — rejected because OAuth and policy management need an authenticated operator workflow.

## Security Implications

- All `/admin/*` routes must require an authenticated admin session.
- All admin GraphQL fields must reject non-admin callers even if the route guard is bypassed.
- The UI should expose editable provider and manifest fields deliberately, not as an unrestricted config browser.
