# Specification Quality Checklist: Optional Lore Repository Synchronisation

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-01
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Feature-Specific Risk Checks

- [x] Sync direction and authority stated explicitly, not left implied
  (FR-021..FR-027; export-first, in-app authoritative, no automatic prose merge)
- [x] The slug-versus-UUID path tension is confronted rather than deferred
  (Assumptions; resolved by separating URL identity from repository path, with
  identity carried in the file header per FR-009)
- [x] Fidelity losses are enumerated rather than discovered (FR-013 non-lore
  cross-links, FR-037 per-entry permissions, Export Fidelity Note entity)
- [x] Failure of the remote never damages in-app lore (User Story 2, FR-028,
  SC-005, SC-006)
- [x] Credential posture stated without being designed (FR-035, FR-036, SC-010)
- [x] Content-policy obligation addressed as an obligation (FR-039..FR-042;
  ADR-043, ADR-049, Constitution v1.1.0 guardrail)
- [x] Existing-file safety on first sync (FR-032, FR-033, SC-007)
- [x] Image handling decided and justified (FR-014, SC-011, Assumptions)
- [x] Per-world vs per-account scope decided (FR-001, FR-003, Assumptions)

## Open Items Requiring User Confirmation

- [x] **Translation direction.** The originating request's sentence is cut off
  at "a job that can translate our [format]". Read here as outward export first,
  with reviewed import as P3. If inbound authoring was the primary intent,
  User Stories 1 and 3 swap priority.
- [x] **Repository paths stay human-readable while in-app URLs may go opaque.**
  The spec's position is that these answer different threats and do not
  conflict. Accepting it means a connected private repository carries readable
  paths that the platform's own URLs will deliberately stop exposing.
- [ ] **FR-042 determination.** Constitution v1.1.0 requires an on-record,
  owner-accepted determination before implementation begins. The spec supplies
  the reasoning; an accountable owner must still sign it, most naturally as an
  ADR amending or extending ADR-049.

## Blocking Dependencies

- [x] `031-playability` FR-038 (lore tree and tags) must ship first; FR-008
  cannot be satisfied without it.
