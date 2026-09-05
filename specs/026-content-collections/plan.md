# Implementation Plan: Content Collections

**Branch**: `026-content-collections` | **Date**: 2026-09-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/026-content-collections/spec.md`

## Summary

A collection is a named set of a world's artifacts — items, actors, abilities,
lore entries and scenes — shared by one unguessable link and copied whole into
another world as independent records. It generalises spec 025's single-artifact
share to a set.

Most of the machinery exists: `generate_link_code()` produces the v4-derived
code, the `world_*_shares` tables establish the revoked-flag pattern,
`moderation::effective_status` gates the read path with lazy restoration, and
three shipped `copy_shared_*_to_world_impl` functions establish the
transactional deep-copy shape.

**Three things are genuinely new**, and the research phase found each by reading
the code rather than assuming from the spec:

1. **An unauthenticated read path.** The spec's clarification claimed anonymous
   viewing matched shipped behaviour. It does not — all three share queries call
   `authenticated_user(ctx)?`, skipping the *membership* check but not the
   session. FR-009a was corrected to record this; the decision stands, the work
   is real.
2. **A GraphQL-level rate limiter.** `rate_limit_auth_requests` keys on the
   request path and returns early outside `/authentication/`. One GraphQL path
   serves the whole application, so FR-009c needs its own limiter.
3. **Scene copying.** Nothing in this product duplicates a scene today. A scene
   carries walls, lighting, shapes, fog, interactives, tokens and a background
   asset row, and which of those belong to *a place* rather than *a session* is
   a design decision, made in research §4.

## Technical Context

**Language/Version**: Rust (2021, workspace toolchain) server; TypeScript 5 /
React 18 web

**Primary Dependencies**: Axum, async-graphql, Diesel/PostgreSQL, uuid (v4 for
codes — never v7), React Router, the fantasy design system in
`apps/web/src/components/ui/`

**Storage**: PostgreSQL — three new tables (`world_collections`,
`world_collection_members`, `world_collection_shares`). RustFS objects are
**read and referenced, never written or deleted** by this feature.

**Testing**: `cargo test -p thunderforge-server` (note: `-p thunderforge` runs
11 tests, not the suite), Vitest for web units, Playwright for e2e

**Target Platform**: Linux server; Chromium browsers only (constitution)

**Project Type**: Web application — Rust GraphQL backend plus React frontend

**Performance Goals**: A 100-member collection copies inside one action the
recipient waits out (SC-002a). No background job, no resumption.

**Constraints**: Copy is one transaction, all-or-nothing (FR-013, SC-006).
Anonymous reads are rate limited (FR-009c). Zero additional stored bytes when
copying a scene whose image the platform already holds (SC-008). Nothing here
deletes a stored object.

**Scale/Scope**: ≤100 members per collection (FR-005a); five member types; three
tables; one new unauthenticated route.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | ✅ Pass | No engine work. Copying a scene writes rows the engine already reads; nothing here re-implements simulation in React. |
| **II. Plugin-modular engine architecture** | ✅ Not applicable | No Bevy plugin is added or changed. |
| **III. Ownership & authorization at the data boundary** | ✅ Pass, and load-bearing | Every mutation checks authority server-side. New tables carry `created_by`/`updated_by`. FR-017a is that convention applied to copies. The one deliberate relaxation — an **unauthenticated** `sharedCollection` — relaxes *authentication*, never *authorization*: it returns only a collection whose owner explicitly shared it, reveals nothing about the world, and writes nothing. Copying stays fully authorized (FR-009b, FR-016). Recorded here because Principle III says deviations need explicit justification. |
| **IV. Real ADRs and specs before divergent implementation** | ⚠️ Action required | Two ADRs must land **with** the feature, not after: the **FR-027 DMCA determination** (below), and an ADR recording the anonymous-read divergence from spec 025's authenticated shares, since that changes an established access boundary. |
| **V. Verify before claiming done** | ✅ Pass | Native `cargo check` for the server, `tsc`/build for web, and the quickstart's seven scenarios exercised in a running instance. |
| **DMCA / Content Moderation Guardrail** | 🚫 **BLOCKING** | See below. |

### The guardrail, stated plainly

The constitution requires, **before implementation begins**, both (a) the
notice-and-takedown program operational, and (b) an on-record determination of
whether this feature constitutes "a centralized public repository", accepted by
an accountable owner.

(a) is satisfied — spec 015's machinery is shipped and this plan uses it
(`moderation::effective_status`, `submit_takedown_notice_impl`).

(b) is **not**. FR-027 says so, and spec 025's determination for single
artifacts is explicitly not pre-approval: bundling changes the unit of
distribution, which is what that review exists to assess. ADR-067 (spec 034) is
the worked example of what satisfying it looks like.

**This is a signature, not a spec edit, and no planning artifact can produce
it.** Planning is complete; `/speckit-tasks` may run; **implementation must not
start until that ADR exists.**

Two facts from research sharpen the case rather than soften it, and both belong
in that determination: FR-020 forbids every enumeration surface, so there is no
browsing; and FR-009a makes the read path **anonymous**, which is a genuine
step toward "public" that single-artifact shares did not take. The honest
summary for whoever signs is that this feature is more public than what shipped
before it, and less public than a repository, because nothing can be found
without already holding a code.

## Project Structure

### Documentation (this feature)

```text
specs/026-content-collections/
├── plan.md              # This file
├── research.md          # Phase 0 — what the code actually does
├── data-model.md        # Phase 1 — three tables and what they deliberately omit
├── quickstart.md        # Phase 1 — how to prove it works
├── contracts/
│   └── collection-share.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 — NOT created by /speckit-plan
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── 2026-09-XX-000000-0000_create_world_collections/{up,down}.sql
└── src/
    ├── collections/
    │   ├── mod.rs                  # module surface
    │   ├── membership.rs           # restriction_reason(): both restriction axes, all five types
    │   ├── resolve.rs              # resolve a member; withheld/absent handling
    │   ├── copy.rs                 # the transaction; per-type copy rules
    │   ├── scene_copy.rs           # scene + walls + lights + shapes + background asset row
    │   └── rate_limit.rs           # FR-009c, this feature's own limiter
    ├── graphql/
    │   ├── mutations_collections.rs    # create/update/delete, add/remove member
    │   └── mutations_collection_shares.rs  # share, revoke, sharedCollection, copy
    ├── models.rs                   # Collection, CollectionMember, CollectionShare
    └── schema.rs                   # regenerated

apps/web/src/
├── api/collections.ts
├── types/collection.ts
├── pages/
│   ├── world-collections/          # build, share, revoke (authenticated)
│   └── collection-share/           # SharedCollectionPage — ANONYMOUS route
└── components/collections/         # member picker, preview, copy dialog, receipt

apps/web/e2e/
├── content-collections.spec.ts             # US1, US2
├── collection-moderation.spec.ts           # US3
└── collection-anonymous-access.spec.ts     # FR-009a/c/d, SC-007a
```

**Structure Decision**: The server work lives in a new `src/server/src/collections/`
module rather than in a fourth `mutations_*_shares.rs`. The three existing share
modules are 1,808 lines of near-duplicate code that already diverge in small
ways; a fourth copy would make four. The GraphQL layer stays thin and delegates
to `collections/`, which is where the logic a test can exhaust — the
five-member-type restriction check (SC-003a), the copy transaction — actually
lives.

Web work follows the shipped `pages/{item,ability,actor}-share/` convention,
with one deliberate difference: `/collection/:shareCode` renders for a signed-out
visitor.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Unauthenticated GraphQL query (relaxes Principle III's usual posture) | FR-009a — sharing with someone who has not joined is most of the point of the feature | Requiring a session was the status quo and was rejected in clarification; it would either lock out the recipients this exists for, or make "sign up to see it" a wall in front of content the owner chose to publish. Authorization is not relaxed: only a shared collection resolves, nothing about the world is revealed, and nothing is written. |
| A second rate limiter | The shipped one keys on `/authentication/` paths and cannot see a GraphQL operation | Extending it to `/graphql` would rate-limit the whole application against a threshold written for password attempts. |
| Polymorphic `member_id` with no foreign key | Five typed membership tables would be five places to forget a type; a cascading FK would silently delete membership when an artifact is deleted, and the spec requires the collection to survive that | ADR-050 resolved the same tradeoff the other way for permission tables, where cascade-on-delete was the *desired* behaviour. Here it is the failure. The cost — a `member_id` may dangle — is bounded because every read resolves and handles absence. |
