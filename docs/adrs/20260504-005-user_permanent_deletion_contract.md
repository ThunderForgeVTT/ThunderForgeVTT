# ADR-004: User Permanent Deletion Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

A permanent deletion feature must remove local identity data, linked auth data, and owned persisted content without leaving orphaned references or silent partial cleanup.

## Decision

Expose `DELETE /user/data` as an authenticated self-service destructive operation. The deletion flow:

1. removes sessions and linked OAuth data
2. deletes or reassigns owned persisted rows based on ownership rules
3. deletes worlds entirely when the user owns them in the current persisted Phase 1 model
4. reserves partial world ownership removal for a future collaborative ownership phase
5. records a non-identifying audit event for the deletion

## Consequences

### Positive

1. Deletion behavior is explicit and testable.
2. The contract is ready to expand when collaborative world ownership is persisted.
3. The system avoids retaining direct personal references unnecessarily after deletion.

### Negative

1. Deletion logic is more complex than pure FK cascade deletion.
2. Future collaborative ownership will require additional persistence and resolver logic.

## Alternatives Considered

1. **Soft delete users only** — rejected because the prompt requires permanent deletion semantics.
2. **Delete every future shared world regardless of other owners** — rejected because it would destroy other users’ data unfairly.

## Migration Implications

- Requires ownership metadata on the persisted content tables in scope.
- Requires deletion-safe foreign key behavior for world-dependent rows.

## Security Implications

- The endpoint must remain self-only and CSRF-protected.
- A non-identifying audit record is retained for destructive-event traceability.
