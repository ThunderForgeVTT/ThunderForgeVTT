# Quickstart: proving Concurrent Client Sessions

How to demonstrate each user story against a running stack, and which
automated case covers it afterwards. Every scenario is written so that a
person can do it by hand at a demo — that is the point of the feature.

## Prerequisites

```bash
make dev          # services, migrations, seeds; admin/admin, user1/user1, user2/user2
```

The seed now opens the instance (`access_policy = 'open'`). If registration is
refused on a fresh database, that fix is missing — see spec 036's Context.

Full suite, when you want the automated answer:

```bash
node scripts/e2e-parallel.mjs --shards=2
```

Per-target checks, per Principle V:

```bash
cargo test --workspace -j 4        # server + crates + packs
make lint                          # lint-host + lint-wasm + file length
pnpm --filter @thunderforge/web test
```

## Scenario A — A second window does not evict the first (US1, FR-001)

1. Sign in as `user1` in Chrome and open a world.
2. Open a **second browser or a private window**. Sign in as `user1` again.
3. Return to the first window and move a token.

**Expected**: the first window is still signed in and the move works. Before
this feature the first window was signed out at step 2.

**Covered by**: `e2e/concurrent-sessions.spec.ts`, including the regression
guard that fails if login ever revokes again (FR-020).

## Scenario B — The sheet on the second screen (US3a, FR-032)

1. With the play field open in window 1, open
   `/world/:id/actor/:actorId/view` in window 2 as the same account.
2. Change a value on the character from window 2.
3. Watch window 1.

**Expected**: window 2 loads without disturbing window 1; the change appears
in both without a reload; window 2 runs no canvas.

**Then take the table over**: ask window 2 for the play field.

**Expected**: window 2 gets it; window 1 says plainly that it has become a
companion and offers to take it back. Exactly one of them holds it at any
moment.

**Covered by**: `e2e/companion-sheet.spec.ts`.

## Scenario C — A check rolled from the sheet is the system's check (US3b, FR-035)

1. In a **D&D 5e** world, open a character sheet and roll Strength, then a
   skill.
2. In a world on a **different system**, open a sheet and repeat.

**Expected**: each produces its own system's check, resolved server-side, and
lands in the game's rolls visible to the table — indistinguishable from the
same check rolled at the play field. A system that declares no check offers
none rather than offering one that cannot mean anything.

**Covered by**: `e2e/companion-sheet.spec.ts`, across at least two systems.

## Scenario D — Offline, the sheet sends you back to the table (US3c, FR-039)

1. Open the play field in window 1 and a sheet in window 2.
2. Cut window 2 off from the server while leaving the peer path intact (the
   harness has this in `e2e/fixtures/offline.ts`).
3. Attempt a check from the sheet.

**Expected**: refused, with a message naming the play field as where to retry.
No roll is recorded anywhere — not at the table, not locally, not queued. The
play field's own continuation under ADR-052 is unchanged.

**Covered by**: `e2e/companion-offline.spec.ts`.

## Scenario E — Ending sessions (US4, FR-006 / FR-007)

1. Sign in as one account in three clients.
2. Sign out of one. **Expected**: the other two keep working.
3. From the account page, look at the session list. **Expected**: each live
   session with when it was created, when it was last used and a coarse
   description — and no address anywhere.
4. End all sessions. **Expected**: every client, including this one, is signed
   out on its next request.
5. Change the password from one client. **Expected**: the others end.

**Covered by**: `e2e/concurrent-sessions.spec.ts`.

## Scenario F — Combat, driven through the UI (US5, FR-021)

1. As a GM, start combat from the combat panel, add combatants, advance the
   turn, end combat.
2. Watch from a second client.

**Expected**: the round counter and active combatant follow in the watching
client without a reload.

**Covered by**: `e2e/combat-panel.spec.ts`, using the existing
`combat-panel`, `start-combat-button`, `combat-round-counter`,
`advance-turn-button`, `end-combat-button` and `combatant-list` test ids —
which have existed all along with nothing driving them.

## Scenario G — External sign-in, observed (US6, FR-022)

Against the harness stub provider:

1. Sign in with an identity the instance does not know → an account is
   provisioned and you arrive signed in (ADR-042).
2. Sign in with an identity whose email already has an account → you are asked
   to confirm your password before it links (ADR-006).
3. Sign in where the provider returns no verified email → refused, no account.
4. Close the instance, then repeat (1) → refused, no account, no session
   (spec 035 FR-006, closing T056).

**Covered by**: `e2e/oauth-provider.spec.ts`. When it is green, strike T056
from spec 032's deferred-manual-pass table (FR-024).

## Making the guards fail on purpose

House habit, and it applies to four of them. Before believing any of these,
break the thing once and watch the test catch it:

- restore the revoke-on-login statement → Scenario A must fail;
- let a companion register with `peerSignals` → Scenario D must fail;
- let `rollCheck` accept a formula from the client → the check contract's test
  must fail;
- drop a play-field claim into a table with a timeout → the takeover test must
  fail when a client is killed rather than closed.
