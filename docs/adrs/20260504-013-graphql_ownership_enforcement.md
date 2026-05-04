# ADR-013: GraphQL Ownership Enforcement

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Ownership metadata only improves safety if the GraphQL API enforces it consistently. ThunderForgeVTT already injects authenticated identity into GraphQL requests through middleware-backed session resolution.

## Decision

GraphQL resolvers must:

- require an authenticated session for ownership-sensitive queries and mutations
- filter collection queries by `created_by == authenticated_user_id`
- reject single-object access when a row exists but is not owned by the requester
- reuse the same export/delete service functions as the Axum endpoints

The GraphQL API exposes ownership metadata fields directly on `World`, `WorldToken`, `WorldEvent`, and `Policy`.

## Consequences

### Positive

1. GraphQL and REST stay consistent because they share the same authenticated identity and export/delete services.
2. Resolver behavior is predictable: list queries filter, single-row queries forbid.
3. Ownership metadata is visible to clients that need to reason about authorship.

### Negative

1. Schema surface grows because ownership fields are first-class API fields.
2. Resolver tests must cover both filtered and forbidden paths.

## Alternatives Considered

1. **Hide ownership fields and enforce only server-side** — rejected because clients also need authorship metadata for account tooling and future collaboration UX.
2. **Trust client-provided owner IDs in GraphQL arguments** — rejected because ownership must come from middleware-authenticated identity, not caller input.

## Migration Implications

- Requires the ownership columns to exist before the schema is safe to expose.
- Does not require separate authorization tables in this phase.

## Security Implications

- Prevents cross-account export and read access through GraphQL.
- Keeps destructive operations bound to the session-authenticated user only.
