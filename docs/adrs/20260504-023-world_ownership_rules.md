# ADR-023: World Ownership Rules

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT already uses `createdBy` and `updatedBy` to track ownership and authorship. World-level permissions now need explicit read and write rules so that dashboards, lists, and destructive actions behave predictably for owners and administrators.

## Decision

World authorization follows these rules:

- `myWorlds` returns only worlds owned by the authenticated user
- `world(id)` returns the world when the caller is the owner or an admin
- `allWorlds` is admin-only
- only the owning user may rename or delete a world
- admins may inspect worlds they do not own, but may not assume ownership or bypass owner-only writes
- deleting a world cascades to world-scoped tokens, events, and policies

## Consequences

### Positive

1. Collection queries and single-record queries behave consistently with the broader ownership model.
2. Administrators can support and inspect worlds without silently gaining write access to user-owned realms.
3. Cascading world deletion keeps related world-scoped data from orphaning.

### Negative

1. Admin tooling must tolerate read-only access for worlds owned by other users.
2. World-scoped policies require an explicit relation to participate in deletion cascades.

## Alternatives Considered

1. **Let admins modify any world** - rejected because inspection and stewardship are safer than implicit cross-account write authority.
2. **Return `null` instead of forbidden for unowned records** - rejected because explicit authorization failures are easier to reason about and audit.

## Security Implications

- Prevents cross-account mutation through GraphQL even for administrators.
- Keeps destructive operations bound to the owner while still allowing controlled administrative visibility.
- Uses foreign-key cascades to avoid policy, token, or event records lingering after world deletion.
