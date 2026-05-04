# ADR-007: No Auto-Provisioning Policy

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

OAuth-first auto-provisioning is convenient, but it can create duplicate accounts, ambiguous ownership, and accidental onboarding before local policies are defined.

## Decision

Outside bootstrap, OAuth does not auto-create a new user record. If an OAuth identity is unlinked and does not match an existing local account eligible for linking, the flow returns `no_matching_user`.

## Consequences

### Positive

1. Account creation remains explicit.
2. Local registration policy stays authoritative.
3. Ownership, export, and deletion semantics start from a deliberate user record.

### Negative

1. New OAuth-only users cannot onboard in one click yet.
2. Product friction is higher until a future invite/onboarding model is added.

## Alternatives Considered

1. **Always auto-provision on first OAuth login** — rejected because it conflicts with explicit linking and future invite policy.
2. **Provider-specific auto-provisioning rules** — deferred until a richer account lifecycle exists.

## Migration Implications

- No extra onboarding shadow state is required in Phase 1.

## Security Implications

- Prevents accidental or hostile identity proliferation through provider-only assertions.
