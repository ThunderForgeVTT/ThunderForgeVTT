# Implementation Plan: Optional Lore Synchronisation to an External Repository

**Branch**: `034-lore-git-sync` | **Date**: 2026-09-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/034-lore-git-sync/spec.md`

## Summary

A Game Master connects a repository they own to a world, and the world's lore
appears there as markdown files in a readable tree, with every edit arriving as
a commit attributed to whoever made it. The repository is a mirror the platform
writes to; in-app lore stays authoritative at all times, and no failure on the
far side of the network may alter, block, or degrade it.

**The first delivery is export only** (clarified 2026-09-04): User Story 1
(mirror outward) and User Story 2 (fail without harm). User Story 3 (accepting
edits made in the repository) is separately scheduled and may never be built.
That boundary is the plan's most important property — *nothing in this delivery
writes to a world's lore*, so it cannot damage one by construction rather than
by care.

**Approach**: a background task on the pattern `main.rs` already uses spawns a
run per connected world; the run drives the `git` binary against a
server-managed working clone over HTTPS. Git is chosen over a host's REST API
because git-over-HTTPS *is* the host-neutral protocol the spec's seam requires,
and because rename detection, divergence refusal, and content verification all
come from it for free. See [research.md](./research.md).

## Technical Context

**Language/Version**: Rust (server, `thunderforge-server` library + `thunderforge` binary); TypeScript/React for the settings surface

**Primary Dependencies**: Axum, async-graphql, Diesel/PostgreSQL, `reqwest` (rustls), `aes-gcm` — **plus two additions**: an RS256 JWT signer (nothing in the workspace can sign one today — research R5) and the `git` binary as an external runtime dependency (research R1)

**Storage**: PostgreSQL for four new tables ([data-model.md](./data-model.md)); a server-managed working clone per connection on local disk, treated as a rebuildable cache, never as state

**Testing**: `cargo test -p thunderforge-server` for the sync engine and path mapping; `cargo test -p thunderforge` for wiring; Playwright e2e for the connection surface and the unconfigured-instance path; a local bare repository as the test remote, so no test touches a real host

**Target Platform**: Linux server. **Self-hosted instances are the primary audience**, which is why polling was chosen over webhooks (an instance behind a home network cannot receive one) and why FR-036b's unconfigured path matters more than it looks

**Project Type**: Web service + web app — server-side feature with a settings surface

**Performance Goals**: An edit reaches the repository within 60 seconds (SC-003). The steady state is a small incremental fetch, not a re-clone

**Constraints**: In-app lore read/write availability and latency must be indistinguishable from an unconnected world under any remote failure (FR-028, SC-005). No credential in any log, response, or **process listing** (FR-035, SC-010). Nothing host-specific past the credential grant (FR-004c)

**Scale/Scope**: Per world; SC-001 names 200 entries as the connect-in-5-minutes case and SC-002 says the mirror must be faithful "for a world of any size"

## Constitution Check

*GATE: checked before Phase 0 and re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. ECS owns simulation, React owns chrome** | **Not engaged.** No canvas, no engine, no simulation state. The only UI is a settings surface. |
| **II. Plugin-modular engine architecture** | **Not engaged.** No engine plugin. |
| **III. Ownership & authorization at the data boundary** | **Passes.** All four tables carry `created_by`/`updated_by`. Every mutation is owner-gated at the GraphQL boundary (FR-002), and FR-003 requires authority to be **re-checked at each synchronisation**, not captured at connection time — a background task acting on stale authority is the failure this guards. |
| **IV. Real ADRs and specs before divergent implementation** | **Passes, with an ADR owed.** The spec exists. An ADR is owed for two architecturally significant decisions: git-over-HTTPS as the transport (rejecting the host REST API on seam grounds), and the credential grant model. It must land in the same change set, not retroactively. |
| **V. Verify before claiming done** | **Passes.** Native `cargo check` for the server, `tsc`/build for the web app. The quickstart's Scenario 3 exercises every failure mode in a running instance, which is what "for UI-affecting changes, exercised in a running dev instance" asks for. |

### DMCA / Content Moderation Guardrail — **BLOCKING**

The constitution requires, before implementation begins, that design review
confirm both:

**(a) The notice-and-takedown program is operational.** Spec 015 shipped;
`dmca-takedown.spec.ts` exercises it end to end. FR-015 and SC-009 tie this
feature into it — a disabled entry leaves the repository on the next run.

**(b) An explicit on-record determination** of whether this constitutes a
centralized public repository. **This does not exist yet.** FR-042 requires it,
[research.md](./research.md) declines to invent it, and no artefact in this plan
substitutes for it.

The spec supplies the reasoning an accountable owner would sign: mirroring is
user-initiated distribution to a repository the user owns, using their own
credential; the platform adds no aggregation, no discovery surface, and no
enumeration (FR-039). It also states the honest cost — a takedown cannot reach
content already mirrored, which is a genuine reduction in takedown
effectiveness for connected worlds (FR-040), and that belongs in the
determination rather than glossed.

**Planning is complete and unblocked. Implementation is blocked until that
determination is recorded and accepted**, most naturally as an ADR amending or
extending ADR-049. `/speckit-tasks` may proceed; the first task must be
obtaining it.

### Post-Phase-1 re-check

No new violations. Two things the design added that are worth naming:

- **`git` as an external runtime dependency** is the first of its kind for this
  server. It is not a constitutional violation, but it is an operational
  commitment, and FR-036c's diagnostic posture is extended to cover it so an
  operator learns about a missing binary at configuration time rather than when
  a Game Master first connects.
- **An RS256 JWT signer is a new dependency.** Noted so it is a deliberate line
  in a diff rather than a surprise.

## Project Structure

### Documentation (this feature)

```text
specs/034-lore-git-sync/
├── plan.md                            # This file
├── research.md                        # Phase 0 — seven decisions, checked against the tree
├── data-model.md                      # Phase 1 — four tables, one deferred
├── quickstart.md                      # Phase 1 — six scenarios plus a review-time seam check
├── contracts/
│   ├── graphql-lore-sync.md           # The API surface, and what is deliberately absent from it
│   └── repository-file-format.md      # The contract with a human holding a clone
├── checklists/requirements.md         # 28/29; FR-042 outstanding
└── tasks.md                           # /speckit-tasks output — not created here
```

### Source Code (repository root)

```text
src/server/src/
├── lore_sync/
│   ├── mod.rs              # spawn_lore_sync_task — the pattern main.rs already uses
│   ├── run.rs              # one pass: fetch, diff, write, push, verify
│   ├── paths.rs            # tree position + title -> path. Pure, and the densest unit tests here
│   ├── document.rs         # front matter + body + link rewriting (repository-file-format.md)
│   ├── git.rs              # the git binary. Credentials via GIT_ASKPASS, never argv
│   └── hosts/
│       └── mod.rs          # the grant boundary — the ONLY place a host is named
├── crypto.rs               # extracted from auth/mod.rs (research R4)
├── graphql/
│   ├── queries/lore_sync.rs
│   └── mutations_lore_sync.rs
├── models.rs               # four new models
├── schema.rs               # regenerated
└── migrations/             # one directory, up.sql/down.sql

src/app/src/main.rs         # spawn_lore_sync_task alongside the existing tasks

apps/web/src/
├── api/loreSync.ts
└── pages/world/settings/LoreRepositoryCard.tsx

apps/web/e2e/lore-repository-sync.spec.ts
```

**Structure Decision**: A `lore_sync` module inside the server library, beside
`moderation/` and `storage/`, following those modules' shape. It is not a pack
and not a crate: it is server behaviour that ships with the product, has no
optional compilation story, and nothing about it is contributed by anyone.

`hosts/` exists to make FR-004a checkable. The seam FR-004b describes is a
directory, so "point at where the host-specific parts are confined" has a
one-word answer — and any host knowledge that appears outside it is visible in
a diff rather than discovered later.

`crypto.rs` is the extraction research R4 found necessary: `encrypt_secret`,
`decrypt_secret` and `encryption_key_from_config_secret` are private to
`auth/mod.rs` today, and copying an encryption routine is how two
implementations drift until one is wrong.

## Complexity Tracking

No constitutional violations require justification. Two additions are recorded
here because they are commitments rather than violations:

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| `git` binary as a runtime dependency | Rename detection (FR-010), force-push refusal (FR-031) and content verification (FR-034) all come from git itself; git-over-HTTPS is the host-neutral protocol FR-004b requires | A host REST API would put GitHub inside the sync engine, which FR-004c forbids. `git2` drags in a C dependency the workspace deliberately avoids; `gix`'s push support is its least mature surface, and this feature is nothing but pushing |
| An RS256 JWT signer | Nothing in the workspace signs asymmetrically; the installation-token exchange FR-036a chose requires it | A pasted fine-grained token would have needed no dependency. It was considered and rejected in clarification; this is the recorded cost of that choice |
