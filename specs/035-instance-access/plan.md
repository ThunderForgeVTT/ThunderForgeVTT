# Implementation Plan: Instance Access — the Gate and the Invitation

**Branch**: `035-instance-access` | **Date**: 2026-09-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/035-instance-access/spec.md`

## Summary

Give the instance a three-state access policy (**open** / **invite-only** /
**closed**) that governs every path capable of creating an account, and an
instance invitation that admits one named person through a closed door.

The load-bearing problem is not the switch — it is that the switch must not
leak. ADR-042 auto-provisions an account for any OAuth identity with a verified
email that matches no user, and that path is not gated by anything today
(research §1). A policy that governed only the local registration form would
let an operator believe a closed instance was closed while every stranger with
a Google account walked in.

**Scope: US1 and US2 only** — the pair the spec calls "the complete minimum
product". US3–US5 (request-access intake, webhook delivery, retention) are
specified but deliberately not planned here; see research §8.

**Approach**: one admission gate function called from both account-creating
paths; a one-row settings table matching `auth_security_settings`; an
`instance_invitations` table mirroring `world_invites` and redeemed with the
same conditional-UPDATE that spec 027 already uses to make its use-cap
race-proof.

## Technical Context

**Language/Version**: Rust 1.75+ (edition 2024) server; TypeScript 5 / React 19 web

**Primary Dependencies**: axum, async-graphql, Diesel (Postgres), React Router

**Storage**: PostgreSQL — two new tables (`instance_access_settings`,
`instance_invitations`), one new append-only log
(`instance_access_events`), one join table (`instance_invitation_redemptions`)

**Testing**: `cargo test --workspace` (server), `vitest` (web unit), Playwright
(`scripts/e2e-parallel.mjs`) for the journeys

**Target Platform**: Linux server + browser

**Project Type**: Web application — Rust API (`src/server`, `src/app`) with a
React front end (`apps/web`)

**Performance Goals**: the gate is one indexed single-row read on the account
creation path only; it must not touch sign-in for existing users

**Constraints**: FR-002 — a policy change takes effect without restart or
redeploy. SC-004 — open to invited-guests-only and a working link in under 3
minutes, no config files. SC-006 — an N-use invitation admits at most N
accounts under concurrent redemption.

**Scale/Scope**: single instance, operator-scale (tens of invitations), not a
hot path

## Constitution Check

*GATE: checked before Phase 0 and re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. ECS owns simulation, React owns chrome** | Not engaged. No engine or scene state; this is auth and admin chrome. |
| **II. Plugin-modular engine architecture** | Not engaged. No engine plugin. |
| **III. Ownership & authorization at the data boundary** | **Directly engaged, and the point of the feature.** Admission is enforced server-side in one function on both creating paths, never in the client — FR-003 exposes the policy to the front end only so it can hide routes that would fail, and the spec is explicit that hiding is not enforcing. New tables carry `created_by` provenance per the convention. **PASS.** |
| **IV. Real ADRs and specs before divergent implementation** | **Engaged: this moves an established access boundary.** ADR-042 currently decides that an unmatched OAuth identity is auto-provisioned; this feature makes that conditional on a policy that answers first. That is a change to a shipped ownership boundary and requires an ADR landing in the same change set — see Complexity Tracking. **PASS with required ADR.** |
| **V. Verify before claiming done** | Server changes verified with native `cargo check`/`cargo test`; web with `tsc` and `vitest`; both admission paths exercised in Playwright against a running stack, because the OAuth refusal is a browser redirect and cannot be proven by a unit test. **PASS.** |

### Required ADR

**ADR-072 — "The Instance Decides Admission Before ADR-042 Decides Provisioning"**

ADR-042 reversed an earlier no-auto-provisioning policy on the stated grounds
of collapsing the onboarding funnel. This feature does not reverse it back: it
places a gate *above* it. ADR-042 continues to govern **how** an admitted
person's account is created (derived username, unusable password, immediate
identity link, no silent takeover); it stops governing **whether** a stranger
may be admitted.

The ADR must record that a closed instance restores, in effect, the pre-ADR-042
behaviour — and that this is a policy state rather than a reversal, so that a
future reader does not "restore" auto-provisioning by deleting the gate.

## Project Structure

### Documentation (this feature)

```text
specs/035-instance-access/
├── plan.md              # This file
├── research.md          # Phase 0 — done
├── data-model.md        # Phase 1 — done
├── quickstart.md        # Phase 1 — done
├── contracts/           # Phase 1 — done
│   ├── instance-access.md
│   └── instance-invitations.md
├── checklists/
│   └── requirements.md  # pre-existing
└── tasks.md             # /speckit-tasks — NOT created here
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── <ts>_instance_access/{up,down}.sql   # 4 tables
├── src/
│   ├── auth/
│   │   ├── registration.rs      # ensure_registration_allowed → the admission gate
│   │   ├── sessions.rs          # local register: already calls the gate
│   │   ├── oauth.rs             # resolve_oauth_login: must now call it too
│   │   ├── admin_setup.rs       # setup_status: + policy fields (FR-003)
│   │   └── instance_access.rs   # NEW — policy read/write, gate, event log
│   ├── admin.rs                 # ensure_/load_/update_ per the settings pattern
│   ├── graphql/
│   │   ├── mutations_instance_invitations.rs  # NEW — issue/revoke/list
│   │   └── queries/admin.rs     # + policy and invitation reads
│   ├── models.rs / schema.rs    # generated + new structs
│   └── test_support.rs          # fixtures for the new tables

apps/web/src/
├── pages/admin/
│   ├── components/adminSections.ts    # + one "Access" entry
│   └── components/AccessPanel.tsx     # NEW — modelled on SecurityPanel
├── pages/auth/                        # registration/sign-in surfaces read policy
├── pages/invite/InstanceInvitePage.tsx # NEW — redeem landing
└── api/instanceAccess.ts              # NEW — typed client

apps/web/e2e/
├── instance-access-gate.spec.ts       # NEW — US1
└── instance-invitations.spec.ts       # NEW — US2
```

**Structure Decision**: the existing web-application layout. Nothing here needs
a new crate or package: the policy is server auth code beside the other auth
code, the invitation is a GraphQL surface beside the other mutations, and the
admin UI is one more section in a list built to be extended.

## Phase 1 design summary

- **[data-model.md](./data-model.md)** — four tables, the state machine for an
  invitation, and why redemption needs its own row.
- **[contracts/instance-access.md](./contracts/instance-access.md)** — the
  policy enum, the gate's contract on both paths, the extended public status
  response, and the refusal shapes.
- **[contracts/instance-invitations.md](./contracts/instance-invitations.md)** —
  issue / list / revoke / redeem, and the uniform refusal.
- **[quickstart.md](./quickstart.md)** — how to prove US1 and US2 by hand and
  which automated checks stand in for each success criterion.

### Post-design Constitution re-check

Re-evaluated after Phase 1: **PASS**, unchanged. The design adds no client-side
authorization, no new ownership boundary beyond the one ADR-072 records, and no
engine surface. The one flagged item — that a shipped access boundary moves —
is answered by an ADR landing in the same change set, as Principle IV requires.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Moving a shipped access boundary (ADR-042's auto-provisioning becomes conditional) | FR-005 requires the policy to govern **every** account-creating path; ADR-042's path is one of exactly two, and leaving it ungated makes the whole feature a false guarantee | Gating only local registration was considered and is what the spec exists to prevent — the operator sees the form disappear and still admits every OAuth stranger. Requires ADR-072 rather than avoidance. |
| A second invitation object beside `world_invites` | Instance and world invitations are granted by different people, admit to different things, and compose rather than substitute | Reusing `world_invites` with a nullable `world_id` — rejected because any query that forgot the filter would confuse the two, and the table's `ON DELETE CASCADE` to `worlds` is meaningless for an instance invitation (research §3) |
