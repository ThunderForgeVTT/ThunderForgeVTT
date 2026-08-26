# Specification Quality Checklist: Client-Side World Cache with Content-Addressed Delta Sync

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-26
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

## Validation Notes

**Iteration 1 findings, since corrected in the spec:**

1. *Implementation leak* — an early draft named OPFS, WASM SQLite, sha256,
   and GraphQL directly in the requirements. All were moved out: FR-005
   now says "fingerprint derived from its contents" rather than naming a
   hash algorithm, and the storage-primitive question was moved to
   Assumptions as an explicitly open planning decision. The user's input
   named these technologies; the spec deliberately does not bind them.

2. *Unmeasurable success criteria* — "faster loading" was replaced with
   SC-001 (≤5% of first-visit bytes), SC-002 (≥3x faster to interactive),
   and SC-003 (delta within 10% of the changed asset's size).

3. *Security requirement was implicit* — permission revocation was
   originally a single line inside another story. It is now User Story 2 at
   P1, with FR-014 through FR-017 and SC-004, because a cache outliving a
   permission grant is a disclosure bug and this repo has already had to
   close two of that class.

4. *Undefined behaviour under pressure* — quota exhaustion and browser
   reclamation were called out in the input as needing specified behaviour.
   Now FR-022 through FR-026 and User Story 4.

**Iteration 2 addition (2026-08-26)**: User Story 6 (engine load feedback)
and FR-028 through FR-033 were added at the user's request. The scope line
was redrawn rather than simply widened — the engine's *size* remains
explicitly out of scope (FR-035), while presenting its *wait* is now in.
The two are kept apart in Assumptions because conflating engine download
time with world load time would invalidate SC-001 through SC-003. Re-ran
all checklist items after the addition; all still pass.

**Deliberate scope exclusions recorded in FR-034 and FR-035**: CRDTs, peer-to-peer, and
offline authoring. These are the parts of the "local-first" architecture
this feature is explicitly *not* adopting; the server stays authoritative.

**Constitution alignment**: Principle III (ownership and authorization at
the data boundary) is the reason User Story 2 is P1 rather than a later
refinement. Principle I is untouched — this feature does not create a second
source of truth for canvas state; the cache is a transport optimization
whose contents are always reconciled against the server.

## Clarification Session 2026-08-26 (post-`/speckit-clarify`)

Five questions asked and answered. Two materially widened the feature beyond
its original framing, at the user's explicit direction after the conflict was
raised:

- **Offline authoring admitted.** The original input said "no CRDTs, no
  peer-to-peer... read-through cache only." The user chose full offline
  operation with queued changes, and on being shown that this reopens the
  local-first direction, confirmed deliberately. Added User Story 7 (P3) and
  FR-036 to FR-043.
- **Peer-to-peer admitted as transport.** WebRTC peer transfer is now
  permitted, with the server still authoritative. FR-044 to FR-050. Content
  addressing is what makes this safe: peers supply bytes, the server supplies
  the fingerprint those bytes must match.

**These two amend ADR-046's server-authoritative posture.** The spec now
states that a new ADR is a precondition of implementation, not a follow-up
(Constitution Principle IV). This is the single largest risk carried into
planning.

Three refinements:

- **Conflict rule**: GM-over-player (FR-040), same-role tiebreak is
  first-to-reconnect (FR-040a, a *derived* default the user did not
  explicitly pick — worth confirming in planning). Timestamps rejected as
  forgeable and routinely wrong.
- **Sign-out protection**: encryption at rest keyed to the session
  (FR-016/016a/016b/016c) rather than best-effort deletion, because a large
  store cannot be wiped instantly and an interrupted wipe leaves readable
  bytes.
- **Space budget**: proportional to browser-reported quota with a ceiling
  (FR-022/022a/022b), not a shipped constant.
- **Observability**: local diagnostics only, no telemetry (FR-051 to FR-054,
  SC-018), chosen to fit the self-hosted AGPL posture.

**Outstanding, deferred to planning**: which entity types may be edited
offline at all (GM-over-player says who wins, not what may conflict); how the
encryption key is derived and whether it survives a browser restart; the
budget proportion and ceiling values; which side of the WASM boundary owns
the store.

## Notes

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`
- All items pass as of 2026-08-26. Spec is ready for `/speckit-clarify` or `/speckit-plan`.
