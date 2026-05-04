# ADR-024: World Placeholder Domain Objects

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Phase 3 needs a complete-looking world dashboard before scenes, actors, tokens, events, game systems, and interface packs have their final persistence models. The dashboard and API still need stable placeholders so later phases can expand the contract without reworking the foundational world UI.

## Decision

World dashboards and GraphQL world payloads expose placeholder sections for:

- scenes
- actors
- tokens
- events
- game system
- interface pack

In Phase 3 these sections are intentionally non-authoritative:

- collection placeholders return empty arrays
- object placeholders return `null`
- the dashboard presents explanatory copy instead of editable data

## Consequences

### Positive

1. The dashboard can ship now with a stable information architecture.
2. Later phases can populate existing sections instead of introducing new dashboard categories.
3. Frontend and backend contracts stay aligned on which domains exist conceptually, even before they are implemented.

### Negative

1. Some panels will be visibly empty until later phase work lands.
2. Clients must not mistake placeholder sections for real persistence support.

## Alternatives Considered

1. **Hide future sections until each subsystem is fully implemented** - rejected because the world dashboard should establish the eventual domain model early.
2. **Return mocked sample content** - rejected because fake data could mislead consumers and obscure unfinished persistence boundaries.

## Security Implications

- Returning empty arrays and nulls is safer than exposing speculative or mocked domain data.
- Placeholder contracts reduce pressure to overgrant access to unfinished subsystems just to fill UI panels.
