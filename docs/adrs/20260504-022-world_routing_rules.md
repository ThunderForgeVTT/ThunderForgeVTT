# ADR-022: World Routing Rules

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Before Phase 3, `/world/:id` was used as a direct workspace entry. The new product flow needs a world list, a creation surface, a metadata dashboard, and a separate route for entering the live world workspace.

## Decision

ThunderForgeVTT routes world surfaces as follows:

- `/worlds` -> world list
- `/worlds/create` -> world creation form
- `/world/:id` -> world dashboard
- `/world/:id/play` -> live world workspace

All world routes require an authenticated session. The dashboard route is the canonical entrypoint for inspecting a world before entering play.

## Consequences

### Positive

1. World discovery, creation, inspection, and play are separated into explicit route responsibilities.
2. The dashboard becomes a stable control room for future settings, invitations, and content management.
3. The previous workspace route remains available under a dedicated play path, avoiding feature loss.

### Negative

1. Existing deep links to `/world/:id` must be updated if they intended to open the live workspace directly.
2. Navigation surfaces need to differentiate between "open dashboard" and "enter world".

## Alternatives Considered

1. **Keep `/world/:id` as the live workspace and put the dashboard elsewhere** - rejected because the dashboard is the more durable, ownership-aware entrypoint for the expanding domain.
2. **Embed world creation in a modal only** - rejected because a dedicated route is easier to prefetch, deep-link, and revisit after validation errors.

## Security Implications

- Auth guards apply consistently to the archive, creation, dashboard, and play routes.
- Ownership-sensitive dashboard data is loaded through guarded GraphQL queries rather than unauthenticated route parameters.
