# ADR-008: Bootstrap Admin Exception

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT must support first-run administrator creation before any local account exists, while still enforcing the stricter non-auto-provisioning rules for normal operation.

## Decision

Allow a one-time bootstrap exception:

- `setup/basic` may create the initial admin locally
- `setup/oauth/{provider}` may create the initial admin from OAuth

After bootstrap completes, normal explicit-linking and no-auto-provisioning rules apply again.

## Consequences

### Positive

1. First-run setup stays flexible.
2. The repo can support secure admin creation without a pre-existing account.

### Negative

1. Bootstrap requires special-case routing and state.
2. Bootstrap logic must remain clearly separate from normal auth flows.

## Alternatives Considered

1. **Local-only bootstrap** — rejected because provider-based admin setup is useful and already scaffolded.
2. **Permanent bootstrap-style auto-provisioning** — rejected because the exception should not become the standard account model.

## Migration Implications

- Depends on admin bootstrap state tables and bootstrap OAuth session state.

## Security Implications

- Bootstrap is guarded by setup state and admin bootstrap codes.
- The exception is intentionally narrow and one-time.
