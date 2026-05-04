# ADR-012: Delete-My-Data Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT needs an irreversible self-service deletion flow that removes identity records and currently persisted owned content without leaving orphaned rows behind.

## Decision

Expose a self-only destructive contract through:

- GraphQL `deleteMyData`
- Axum `DELETE /user/data`

The deletion flow removes:

1. sessions
2. OAuth links and outstanding auth challenges
3. owned `world_tokens`
4. owned `world_events`
5. owned `policies`
6. owned `worlds`
7. the `users` row

Deletion emits a non-identifying log event through structured application logging rather than a dedicated audit table.

Future collaborative world ownership rules remain a contract concern, but the current persisted implementation only deletes worlds directly owned by the requesting user because shared-world memberships are not yet persisted in this repo state.

## Consequences

### Positive

1. Deletion behavior is explicit and testable.
2. The implementation stays aligned with the tables that actually exist today.
3. The contract can later expand to partial world ownership removal when collaborative ownership is implemented.

### Negative

1. Future shared-world retention is documented rather than fully implemented today.
2. Destructive ordering must stay coordinated with foreign keys and explicit deletes.

## Alternatives Considered

1. **Soft-delete the user only** — rejected because the contract is for permanent deletion.
2. **Rely only on broad FK cascades** — rejected because the system needs explicit, reviewable deletion behavior.

## Migration Implications

- Depends on ownership metadata for the four persisted content tables.
- Does not introduce a new membership or audit-log table in this scoped implementation.

## Security Implications

- The endpoint must remain self-only, authenticated, and CSRF-protected.
- Logging must avoid retaining direct personal identifiers after deletion.
