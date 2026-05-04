# ADR-020: World Creation Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Phase 3 introduces the world as the primary durable container for tabletop play. ThunderForgeVTT already has authenticated session middleware, ownership fields on persisted tables, and a GraphQL entrypoint, so world creation must fit those existing contracts instead of introducing a side-channel API.

## Decision

World creation is handled by the authenticated GraphQL mutation `createWorld(input)` and persists a single world record with server-assigned ownership metadata.

- required input: `name`
- optional input: `description`, `gameSystemId`, `interfacePackId`
- the server normalizes and validates the world name before persistence
- `createdBy` and `updatedBy` always come from the authenticated user, never from client input
- duplicate world names are rejected per owner, not globally

## Consequences

### Positive

1. World creation follows the same authenticated GraphQL flow as the rest of the application.
2. Ownership metadata is guaranteed at insert time, which keeps later authorization checks simple.
3. Placeholder system and interface identifiers can be persisted now without blocking later richer contracts.

### Negative

1. Validation rules must be kept in sync between UX hints and backend enforcement.
2. Case-insensitive duplicate detection adds an owner-scoped uniqueness constraint to the data model.

## Alternatives Considered

1. **Create worlds through a REST endpoint** - rejected because the application already uses GraphQL for ownership-aware domain operations.
2. **Trust client-supplied ownership metadata** - rejected because ownership must be derived from the authenticated session.

## Security Implications

- Prevents clients from forging `createdBy` or `updatedBy`.
- Rejects duplicate names inside the same account boundary, reducing ambiguous ownership outcomes.
- Keeps world creation behind the same session and CSRF protections as other authenticated writes.
