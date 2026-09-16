# Quickstart: proving a pause

**Spec**: [spec.md](spec.md) · **Contracts**: [contracts/graphql.md](contracts/graphql.md), [contracts/live-play-lock.md](contracts/live-play-lock.md)

Proof is end-to-end in Chromium (FR-064). Unit and server tests support it; they
do not replace it.

## Prerequisites

- The e2e stack as usual, with `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and
  `--workers=1` on the external stack (otherwise 429s pose as flakes).
- Migrations run: `diesel migration run` in `src/server`.
- The seeded operator `e2eadmin` with its TOTP (global setup does this); sign in
  with `openAdminPage(browser)` from `apps/web/e2e/fixtures/admin.ts`.
- No `context.setOffline`: it breaks Vite's lazy chunks. Use `severableLink`,
  `waitForOffline` and `waitForOnline` from `apps/web/e2e/fixtures/offline.ts`.

## Checks per phase

```bash
cd src/server && cargo test -q --lib play_pause
```

```bash
cd src/server && cargo test -q --lib surface_tests
```

```bash
cd apps/web && pnpm exec tsc --noEmit
```

```bash
cargo check --target wasm32-unknown-unknown -p thunderforge_engine
```

The last one only if an engine file changed; `pnpm verify` does not type-check
the web app, so `tsc` is always run.

## Scenarios

Each maps to one e2e file under `apps/web/e2e/`.

### 1. Pausing reaches the table (US1, FR-060, SC-001) — `play-pause.spec.ts`

1. A Game Master and a player join one world in two browsers, on **different
   scenes**. A second world is played in a third browser.
2. The operator pauses the first world from `/admin/play-pauses` with grounds.
3. **Expect**: both browsers show the notice within 5 s, with no reload (assert
   no navigation of type `reload`, and the elapsed time). The notice has no
   grounds text. The third browser is still playing.
4. **Expect**: a non-operator's direct `pauseWorldPlay` call is refused.

#### Measured (T062, 2026-09-16)

Five runs of `play-pause.spec.ts` and `play-pause-stream-poll.spec.ts`, two
shards, `ENGINE_PROFILE=dev`, 5/5 green each run. Times are from the
operator's confirmation (or from the pause being sent, for the stream-poll
spec) to the page leaving the playfield, or to the stream's error.

**The event path**: a real page on the playfield.

| Run | GM (scene B) | Player (scene A) | Operator who is a member | GM beside them | Event 28 withheld |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 272 ms | 239 ms | 180 ms | 218 ms | 48 ms |
| 2 | 223 ms | 167 ms | 280 ms | 280 ms | 47 ms |
| 3 | 224 ms | 187 ms | 161 ms | 213 ms | 44 ms |
| 4 | 229 ms | 228 ms | 244 ms | 205 ms | 66 ms |
| 5 | 275 ms | 226 ms | 214 ms | 182 ms | 99 ms |

Worst: **280 ms**. With event 28 withheld the page still leaves in under
100 ms, because its `worldSyncPlan` is refused first (see `2e98fea`), so the
page path never waits on a tick.

**The poll-only path**: a client with no logic, which ignores event 28 and
does nothing but hold `playField`, `peerSignals` and `worldEventsCreated`
open. Only the server's `LIVENESS_POLL` (5 s) ends its streams.

| Paused after the streams opened | Error after the pause (5 runs, 3 streams) | Error after the stream opened |
| --- | --- | --- |
| 1 s | 3984–4010 ms | 5006–5012 ms |
| 3.5 s | 1471–1509 ms | 5005–5013 ms |

The error always lands on the tick, 5,005–5,013 ms into the stream, so the
wait after a pause is the rest of the current tick. **No run exceeded 5 s**,
and neither SC-001 nor `LIVENESS_POLL` is changed. The one caveat, stated so
it is not rediscovered: a pause landing in the few milliseconds just after a
tick would wait a whole period plus the tick's query, about 5,013 ms, for a
client that ignores every other signal. A real page is never that client.

### 2. The pause holds (US2, FR-061, SC-002, SC-003) — `play-pause-holds.spec.ts`

1. Pause a world with a player's browser severed by `severableLink` beforehand,
   and a move queued while severed.
2. From a connected browser, open `/world/:id/play`: **expect** the notice.
3. Call `heartbeat`, `worldSyncPlan`, `moveOwnToken` and open
   `worldEventsCreated` directly with the player's session: **expect**
   `WORLD_PLAY_PAUSED` for each.
4. Restore the severed link: **expect** the notice, the reconcile report's
   `PlayPaused` rejections, "1 change … wasn't kept", and the token's server
   position unchanged.
5. The world's Owner calls `liftWorldPlayPause`: **expect** refusal.
6. Run an axe audit on the notice: **expect** no violations.

### 3. A takedown asks (US3, FR-062, SC-005) — `play-pause-request.spec.ts`

1. A table is playing a scene. File a takedown on that scene through
   `/legal/dmca`, then a second one on an actor in the same world.
2. **Expect**: one pending request for the world at `/admin/play-pauses`,
   carrying two triggers, marked *played now*.
3. Approve it: **expect** the table removed as in scenario 1.
4. In a second world being played, raise a request the same way and decline it:
   **expect** no change on the table's page, no event, and nothing in
   `worldPlayState.history`.
5. Two operator pages approve one request at once: **expect** one pause, and the
   second told who decided.
6. A takedown on a world nobody has played in a minute: **expect** no request.

### 4. Lifting (US4, FR-063, SC-007) — `play-pause-lift.spec.ts`

1. Pause a world whose scene was taken down. Lift the pause.
2. **Expect**: the notice offers *Return to the world* within 30 s; play starts;
   the taken-down scene is still withheld.
3. Resolve the takedown by counter-notice on a still-paused world: **expect** the
   pause still active.

### 5. What is known (US5, SC-004, SC-006)

Covered inside scenarios 1, 3 and 4 rather than a file of its own: the Game
Master's world page shows *paused since* and the history's times with no reason;
the operator's record shows who, when, grounds, triggers and the lift.

## Server tests that back the scenarios

- `play_pause::gate` refuses on an active pause and not on a lifted one; is blind
  to `is_admin`.
- `play_pause_surface_tests`: every root field classified; every `GATED` field
  refuses on a paused world.
- `session_lifetime`: a wrapped stream yields one `WORLD_PLAY_PAUSED` error then
  completes within one tick of a pause (the existing revoked-session test is the
  template).
- Requests: one pending per world; the conditional decision loses cleanly;
  triggers attach to an active pause instead of raising.
- A restore by counter-notice, appeal and lazy elapse each leave a pause active.
- A world deleted while paused leaves its pause and requests readable to
  operators with `worldExists: false`.
