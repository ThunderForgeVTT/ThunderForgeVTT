# ADR-016: Analytics Data Sources

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

The Phase 2 admin dashboard needs system-wide metrics without inventing synthetic telemetry. ThunderForgeVTT already persists users, worlds, tokens, events, and policies in PostgreSQL, while storage footprint exists on disk beneath the configured data root.

## Decision

Admin analytics will be derived from durable application state that already exists today.

- user, world, token, event, and policy counts come from existing persisted tables
- disk usage is calculated from the configured data directory rather than external monitoring
- admin welcome and admin settings read from the same server-side aggregation helpers

## Consequences

### Positive

1. Metrics reflect real system state and remain available without introducing a telemetry pipeline.
2. The dashboard can be implemented with minimal new infrastructure.
3. Counts stay consistent across admin views because they come from shared helper functions.

### Negative

1. Metrics are coarse snapshots rather than time-series analytics.
2. Disk usage calculation can be slower than reading a precomputed cache on large deployments.

## Alternatives Considered

1. **Add a metrics database or observability stack** — rejected for MVP because it would add operational weight and duplicate persisted facts already stored by the app.
2. **Show placeholder counters in the UI** — rejected because admin pages must report real state, not demos.

## Security Implications

- Analytics queries are admin-only because aggregate counts can still reveal sensitive system posture.
- Disk usage responses should expose totals and safe breakdowns, not raw filesystem listings outside managed paths.
