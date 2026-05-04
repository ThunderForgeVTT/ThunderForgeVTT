# ADR-019: Disk Usage Calculation Strategy

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

The admin dashboard needs a storage metric for the current ThunderForgeVTT deployment. No external storage accounting service exists, and the app already owns the data root where worlds, assets, databases, and related files live.

## Decision

Disk usage will be calculated server-side by traversing the configured data directory and summing file sizes for managed application paths.

- totals are returned in bytes for UI formatting
- the admin stats response may include a safe directory breakdown
- admins can trigger recalculation explicitly when they need a fresh snapshot

## Consequences

### Positive

1. Storage reporting works without extra infrastructure.
2. The same implementation supports both the admin welcome summary and detailed storage view.
3. Byte totals let the frontend format the value for different cards and charts.

### Negative

1. Recalculation cost scales with the size of the data directory.
2. Results are approximate snapshots, not live filesystem watches.

## Alternatives Considered

1. **Rely on OS-level monitoring integration** — rejected because it is outside MVP scope and not portable enough for the current app.
2. **Persist a cached total only** — rejected because admins need a way to refresh when files change outside normal request flow.

## Security Implications

- Traversal must stay inside the configured ThunderForge data root.
- Responses should report aggregate sizes, not arbitrary file contents or unrestricted path metadata.
