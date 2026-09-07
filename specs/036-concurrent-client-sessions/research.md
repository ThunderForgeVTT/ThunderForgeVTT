# Phase 0 Research: Concurrent Client Sessions

Eight decisions. Each records what the codebase already does, what was chosen,
and what was rejected — because most of these have a plausible wrong answer
that looks simpler.

## R1 — Removing the eviction without removing the protection

**What is there today**: `src/server/src/auth/sessions.rs:370` revokes every
un-revoked `user_sessions` row for the account on every successful login, with
the comment "Revoke existing active sessions on new login to reduce session
replay risk." The table has no unique constraint on `user_id`; the policy is
entirely in that statement.

**Decision**: delete the revoke-on-login statement, and replace the protection
with four things, all of which the eviction was approximating:

1. Every session is listable by its owner, with created, last-seen and a
   coarse origin description (FR-005).
2. Any session is individually revocable, and all of them at once (FR-003,
   FR-007).
3. A password change revokes every session but the one that changed it
   (FR-008).
4. Session creation is recorded where the owner can see it (FR-026).

**Rationale**: eviction reduced replay by shortening the window in which a
stolen cookie is useful — but only for people who log in often, and at the
cost of the feature being asked for. Visibility plus revocation covers the
same threat with a control the person can actually operate, which is the
industry-standard shape.

**Alternatives rejected**:

- *Keep eviction, exempt "the same device".* Requires a device identity the
  product does not have, and a second browser on one machine is a different
  device by every definition available to the server. This is the case that
  broke the demo, so exempting the wrong thing solves nothing.
- *Make it configurable per instance.* Adds an axis of behaviour to test and
  leaves the default wrong for somebody. The bound in R3 is the only knob.

## R2 — Where the play-field claim lives

**Decision**: an in-process registry keyed by account, whose entry is owned by
the guard returned when a play-field client registers, and which is dropped
when that client's stream ends. Not a database row.

**Rationale**: FR-030 requires that a claim never outlive the client that made
it. `peer_signaling.rs` already solved exactly this shape and says why in its
own module docs: "Registration begins when `peerSignals` establishes its
stream and ends when that stream is dropped — the guard returned by
`PeerRegistry::register` is owned by the stream itself, so there is no way for
an entry to outlive the connection that created it… enforced by construction
rather than by a cleanup job that can be forgotten, misconfigured, or skipped
during a crash." A claim in a table needs a heartbeat, a timeout and a
reaper, and its failure mode is a person locked out of their own table by a
window that crashed.

**Consequence accepted**: the registry is per process, so it is correct for
one server and needs a shared registry if the product is ever run
multi-instance. That is already true of the peer registry and of presence, so
this feature does not introduce the constraint — it inherits it, and
`data-model.md` records it as a known boundary rather than a surprise.

**Alternatives rejected**:

- *A `play_field_claims` table with `last_seen_at`.* Durable, and wrong: the
  interesting state is liveness, which a row cannot express without a
  heartbeat that reintroduces the very lockout FR-030 forbids.
- *Deriving the claim from "the newest session".* A person's newest session is
  frequently the sheet they just opened; the play field would move every time
  they opened a second window.

## R3 — Takeover semantics and the session bound

**Decision**: the newest claim wins. The previous play-field client is told
over its own live stream that it has been demoted, shows a plain explanation,
and offers to take the claim back. Concurrent claims are resolved so exactly
one holder exists (a single guarded swap), and the bound on live sessions per
account is **10**, evicting least-recently-used on overflow.

**Rationale**: refusing the new window reproduces the reported defect one
level up — "you are already playing somewhere else" is as much a dead end as
"you have been signed out", and the person cannot always reach the other
window to release it. Refusing the *new* thing is the failure mode this whole
feature exists to remove.

**Alternatives rejected**:

- *Refuse the second play field.* See above.
- *Ask the user to confirm the takeover.* The confirmation would appear on the
  window they are looking at, about a window they may have closed the lid on.
- *No session bound.* An unbounded set of live sessions is both a resource and
  an audit problem; LRU eviction at a high bound is invisible in practice and
  bounded on paper.

## R4 — What counts as "the play field"

**Decision**: a client is a play-field client if and only if it mounts the
engine. In the router as it stands that is `/world/:id` and the scene routes
beneath it; every other world route — `/world/:id/actor/:actorId/view`,
`/compendium`, `/lore/...`, `/players` — is already a companion surface and
needs no change to become one.

**Rationale**: the property that matters is not the URL but the canvas: the
engine is the thing whose duplication would produce two simulations, two event
appliers and two peer endpoints for one person. Tying the claim to the mount
means the rule cannot drift away from the reason for the rule.

**Consequence**: the claim is requested by the shell immediately before the
engine is created and released when it is torn down, so a companion never has
to know it is one.

## R5 — The peer boundary, enforced in one place

**Decision**: `peerSignals` registration requires the caller to hold the
play-field claim; a companion is refused. Enforced at registration, not at
each signal.

**Rationale**: spec 028 FR-050 already ends peer reachability with the
subscription; adding "and only for the claim holder" at the same point keeps
one place to read. A companion that cannot register cannot be addressed, cannot
open a channel, and therefore cannot carry a roll — which is FR-038 by
construction rather than by a check on a message path.

**Alternatives rejected**:

- *Refuse only roll-shaped payloads over peer channels.* The server relays
  opaque strings and deliberately never interprets them (spec 028 FR-044).
  Inspecting them to enforce this would undo that.
- *Let a companion peer but refuse its rolls at the server.* The whole point
  of the offline case is that the server is not there to refuse anything.

## R6 — What a companion refuses when the server is gone

**Decision**: a companion surface that has lost the server refuses adjudicated
actions with a message naming the play field, records nothing, and queues
nothing. It keeps rendering what it already has.

**Rationale**: ADR-052 separated *may hold*, *may continue* and *may
distribute*, and scoped continuation to token position, rotation and scale
replayed on reconnection. A queued roll is neither — a roll that resolves
later is a different roll, and one the table has already moved past. The
honest answer is a refusal that tells the person where the capability lives.

**Alternatives rejected**:

- *Queue the roll in the outbox.* The outbox replays authoring, whose late
  application is still correct. A die that lands ten minutes later is not.
- *Roll locally and reconcile.* This is precisely the client-decided outcome
  ADR-044 exists to forbid, and the dnd5e wasm stub that always returned 10 is
  what it looked like last time.

## R7 — System-declared checks

**What is there today**: every pack declares its roll shape under its own key
— `coreCheck` (pathfinder2e, `"1d20+modifier"`), `actionRoll` (blades),
`taskResolution` (cypher), `ladderRoll` (fate), `skillRoll` (year zero),
`manifestationRoll` (genie) — and **dnd5e declares none at all**, only
`abilities` and `skills` (each skill naming its governing ability). So there
is no uniform way to ask a system "what can this character roll, and how?",
which is exactly what a sheet button needs.

**Decision**: one new manifest declaration, `checks`, uniform across packs:
each entry names an id, a label, an optional group, a formula, and how the
formula's placeholders bind to values on the actor's sheet. `dnd5e` gains a
`checks` block generated from its existing `abilities` and `skills`; the other
packs may adopt it at leisure, and a pack that declares none offers no check —
FR-037. Resolution happens server-side: a new `rollCheck` mutation resolves
placeholders against the actor and hands the finished formula to the same
authoritative path `rollDice` already uses.

**Rationale**: ADR-044 makes the server the only party that may produce a
roll, and spec 032 has already established that a sheet is what a system
*declares*. This is those two rules meeting a new button. The per-pack keys
stay where they are — they describe a system's core resolution mechanic for
other purposes, and rewriting six packs is not this feature's business.

**Alternatives rejected**:

- *Read the existing per-pack keys directly.* Six shapes, one of them absent
  in the system the person actually asked about, and the sheet would need to
  understand each — a system's rules in shared presentation code, which the
  constitution forbids.
- *Compute the check in the sheet from `abilities`/`skills`.* That is 5e's
  rule (`(score - 10) / 2`) living in the app, which is the exact thing the
  engine was purged of in spec 029.

## R8 — The multi-client fixture, and the OAuth stub

**Decision (fixture)**: one fixture that returns an additional signed-in client
for an account that is already signed in, asking explicitly for either another
**context** (its own cookie jar — the real second-browser case) or another
**tab** in the same context. It signs in for real rather than copying cookies.

**Rationale**: copying `storageState` between contexts would make the fixture
pass whether or not the eviction was removed — the fixture would stop being
evidence. Signing in for real is what FR-020's regression guard needs.

**Decision (OAuth stub)**: a test-only HTTP service in the harness with three
routes (authorize, token, userinfo), a seeded `oauth_providers` row pointing at
it, and a port per shard exactly as backends and vite servers already get one.

**Rationale**: `oauth_providers` already carries `authorization_url`,
`token_url` and `userinfo_url` per row, and
`exchange_authorization_code_with_provider` already takes the provider as an
argument — so a stub needs no branch, flag or `#[cfg(test)]` inside `oauth.rs`,
which FR-023 requires. A test that exercises a test-only branch proves the
branch.

**Alternatives rejected**:

- *Drive a real provider.* Not reproducible, not offline, and rate-limited.
- *Call the internal resolve function directly.* That skips the ~40 lines
  between the redirect coming back and the outcome, which is the untested
  region this exists for.
