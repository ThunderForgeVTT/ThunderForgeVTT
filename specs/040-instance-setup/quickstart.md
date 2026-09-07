# Quickstart: proving Instance Setup and Configuration

How to demonstrate each user story against a running stack, and which
automated case covers it afterwards. Every scenario is written so a person can
do it by hand — SC-001 is a stopwatch claim about a human, so a human has to
run it at least once.

## Prerequisites

```bash
make dev          # postgres, rustfs, mailpit; migrations; seeds
```

`make dev` now brings up **Mailpit** alongside postgres and rustfs. Its web
interface is at <http://localhost:8025> and it is where every message this
instance sends will land in development. If it is not there, `compose.yml` is
missing the service and Scenario D cannot be run.

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

**Scenario A needs an empty database.** `make dev` seeds one. Use:

```bash
make services-up
dropdb -h localhost -U postgres thunderforge_firstrun 2>/dev/null; createdb -h localhost -U postgres thunderforge_firstrun
DATABASE_URL=postgres://postgres:password@localhost/thunderforge_firstrun \
  sh -c 'cd src/server && diesel migration run'
DATABASE_URL=postgres://postgres:password@localhost/thunderforge_firstrun pnpm dev
```

No seeds. That is the point.

---

## Scenario A — Empty database to a contactable instance (US1, FR-001…FR-006)

**Start a stopwatch.** SC-001 says under ten minutes.

1. Open the app. **Expected**: you are taken to setup, not to a bare account
   form and not to a sign-in page.
2. Find the bootstrap link in the server's log. **Expected**: it names the URL
   you are actually running on. If it says `http://127.0.0.1:5173` and you are
   not there, that defect is unfixed.
3. Work through the pass: administrator account → who operates this instance →
   the contact for copyright notices → the support address → mail settings →
   second factor → review.
4. On the operator step, try to continue with the name blank. **Expected**:
   refused, saying what is needed.
5. On the notice contact, enter `dmca@thunderforge.example`. **Expected**:
   refused, and the message explains that a reserved domain cannot receive a
   notice. Enter a real address.
6. On the mail step, choose to skip it.
7. On the second factor step, enrol. **Expected**: setup will not complete
   without it (FR-002a). *This step is spec 041's flow; until 041 lands it is
   the bare existing endpoints, and that is expected.*
8. Finish. **Expected**: a summary naming everything still unset — mail, and
   any legal prose block left blank.

**Then, without editing a file:**

9. Sign out and open `/legal/terms`, `/legal/privacy` and `/legal/dmca` as a
   stranger. **Expected**: the operator's name and contact appear where
   `[OPERATOR — …]` used to be; the DMCA page's designated agent shows the
   values you entered rather than `dmca@thunderforge.example`.
10. **Also expected, and correct**: the prose markers you were not asked about
    — the governing jurisdiction, how changes are announced — are still
    visibly outstanding. See research.md § D2. A page that reads as complete
    while its governing-law clause names nobody would be worse.

**Stop the stopwatch.**

**Covered by**: `apps/web/e2e/instance-setup.spec.ts`, in the `first-run`
Playwright project against a migrated-but-unseeded database.

## Scenario B — Everything is changeable afterwards (US2, FR-007, FR-008)

1. As the administrator, open Admin → Configuration.
2. Change the support address. Reload a page that shows it. **Expected**: the
   new value, with no restart and no redeploy.
3. Change the notice contact. Open `/legal/dmca` in a private window.
   **Expected**: the new contact, immediately.
4. Open the change history for that setting. **Expected**: who changed it,
   when, and what it was before.
5. Change the SMTP password, then look at its history. **Expected**: a record
   that it changed, marked redacted, with `set` → `set` rather than either
   value. If you can read the old password anywhere, FR-023 is broken.

**Covered by**: `apps/web/e2e/instance-settings.spec.ts`.

## Scenario C — One precedence rule (US3, FR-009…FR-012)

1. Stop the server. Add to `.env`:

   ```
   THUNDERFORGE_SUPPORT_EMAIL=env-wins@example.org
   THUNDERFORGE_OPERATOR_NAME=Set By Environment
   ```

2. Start it. Open Admin → Configuration.
3. **Expected**: both values show `env-wins@example.org` and `Set By
   Environment`, are marked **fixed by the environment**, name the variable
   that fixed them, and **are not offered as editable fields**.
4. Try to change one anyway, through the API. **Expected**: refused, naming
   the variable. Not a silent no-op — ADR-041 records what a silent no-op cost
   the OAuth surface.
5. Look at an OAuth provider set by `OAUTH_*` in the same screen.
   **Expected**: it reports the same source vocabulary. Two mechanisms
   underneath, one answer on the screen.
6. Stop the server, remove the two variables, start it again.
   **Expected**: the values revert to whatever was stored, the source now
   reads as the instance's own, and readiness reports the flip rather than
   letting you discover it.

**Covered by**: `apps/web/e2e/instance-settings.spec.ts`, and a server test
asserting env-beats-row-beats-default for every declared setting rather than
for a sampled few.

## Scenario D — The instance can send mail, and knows whether it can (US4, FR-013…FR-017)

1. Admin → Mail. Enter `localhost`, port `1025`, security `none`, a From
   address.
2. Press **send a test message** to your own address.
3. Open <http://localhost:8025>. **Expected**: the message is there, from the
   address you set.
4. Back in the admin panel, look at the outbox. **Expected**: one entry,
   `SENT`, with the recipient and the time — **and no message body anywhere on
   the screen**.
5. Now break it: change the port to `1026` and send another test.
   **Expected**: a failure that names what to fix. Check the whole screen and
   the server log for the SMTP password. **If it appears anywhere, FR-014 is
   broken and this is the check SC-004 is about.**
6. Clear every `mail.*` setting.
7. Look at readiness. **Expected**: mail unavailable, the missing settings by
   name, and a list of what is limited.
8. Cause something that would send a message (once 035/037/039 land; until
   then, `sendTestMail`). **Expected**: the outbox shows it `BLOCKED`, not
   gone. Nothing was discarded.
9. Re-enter the working settings. **Expected**: the blocked message leaves
   `BLOCKED` and is delivered without anybody asking it to.

**Covered by**: `apps/web/e2e/mail-delivery.spec.ts` against the shard's
Mailpit, plus a Rust integration test that sends through a real
`SmtpTransport` — the level it would be tempting to skip.

## Scenario E — GitHub credentials at two scales (US5, FR-018…FR-024)

1. Start with **only** `SYNC_GITHUB_APP_*` in the environment, exactly as a
   working deployment has today. Open a world's lore repository card.
   **Expected**: unchanged behaviour. This is FR-024, and it is the first
   thing to check, not the last.
2. Admin → GitHub applications. **Expected**: `sync` resolves from the
   environment; `global` is unconfigured; `feedback` falls through to nothing.
3. Set a **global** application in the admin screens. **Expected**: on the
   same screen, a statement of which subsystems it will act for. `feedback`
   now resolves to it; `sync` still resolves to its own environment
   variables — scope outside, source inside.
4. Set a **subsystem** application for `feedback`, but give it only a client
   id. **Expected**: reported as incomplete, naming the missing fields, and
   `feedback` falls back to the **whole** global application, saying so. It is
   not silently completed from the global one's private key.
5. Paste something that is not a private key. **Expected**: refused on save,
   naming the field. Not stored, and not reported as configured.
6. Set `GLOBAL_GITHUB_APP_PRIVATE_KEY_BASE64` to `not a key at all`.
   **Expected**: the complaint is about *that variable not being base64*, not
   a generic unreadable-key error — `repo_host.rs` already gets this right and
   documents why.
7. Read every diagnostic on the screen and in the log. **Expected**: variable
   names, never a value, never a fragment, never a length.

**Covered by**: `apps/web/e2e/github-apps.spec.ts`.

## Scenario F — An instance says what it is not ready for (US6, FR-025…FR-028)

1. On a fresh instance with no notice contact, try to create a collection
   share. **Expected**: **refused**, naming exactly what is missing. Not a
   warning.
2. In the same instance, create a world, invite somebody, roll dice, share an
   actor *inside* the world. **Expected**: all of it works. An instance with
   nobody to notify is a private instance, not a broken one.
3. Set the notice contact. Retry the share. **Expected**: it works.
4. Now clear the notice contact again and open the share link you already
   issued. **Expected**: it still resolves. Removing a setting must not break
   links already given out; only creating a new one is refused.
5. Admin → Readiness with several things unset. **Expected**: each gap names
   what it disables and what to set. No value, no fragment, no length.
6. Configure everything. **Expected**: a positive statement that the instance
   is fully configured — not an empty list.

**Covered by**: `apps/web/e2e/instance-readiness.spec.ts`.

## Scenario G — Upgrading an existing deployment (FR-024, FR-028, SC-009)

The one nobody demos and everybody needs.

1. Check out the commit **before** this feature. `make dev`. Create a world,
   an actor, a share.
2. Check out this feature. `make migrate`. Start the server.
3. **Expected**: it starts. No new required setting stops it (FR-028).
4. **Expected**: the world, the actor and the share are all still there and
   still work.
5. Open Admin → Readiness. **Expected**: it tells you the operator identity
   and the notice contact are unset, what that disables, and how to set them.
   It asks; it does not assume and it does not break.
6. **Expected**: `support_email` is flagged as still being the shipped default
   `stewards@thunderforge.local` — a gap, not a refusal.

**Covered by**: not automatable in the current harness. Run it by hand before
release and record the result in the commit body. A migration test that
asserts `up.sql` then `down.sql` then `up.sql` is clean covers the schema half.

---

## Making the guards fail on purpose

House habit, and this feature has six worth breaking once before believing:

- **Return a masked secret** (`sk-…3f9`) from `instanceSettings` → the
  redaction test must fail. If it passes, SC-007 is decoration.
- **Log the SMTP password** in a `DeliveryFailure` reason → the FR-014 test
  must fail.
- **Add a `body` field to `OutboxEntry`** → the FR-016 test must fail.
- **Let the notice-contact gate be a warning instead of a refusal** → Scenario
  F step 1 must fail.
- **Let a subsystem GitHub application borrow the global one's private key** →
  the FR-021 test must fail.
- **Remove the `SYNC_GITHUB_APP_*` first step from
  `github_apps::registration_for`** → the FR-024 back-compatibility test must
  fail. This is the one that protects somebody's running deployment.

And one that is a design check rather than a test: **add a new setting to the
registry with no `env_var` and no `secret` decision.** Compilation should
refuse it. If it does not, FR-012 is a convention rather than a rule.
