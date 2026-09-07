# Quickstart: proving In-App Feedback

How to demonstrate each user story against a running stack, and which automated
case covers it afterwards. Every scenario is written so a person can do it by
hand, because two of them — the screenshot picker and the pre-submission notice
— are things only a person can judge.

## Prerequisites

```bash
make dev          # services, migrations, seeds; admin/admin, user1/user1, user2/user2
```

For anything that reaches a destination, either point at the harness stub:

```bash
export GITHUB_API_BASE=http://localhost:4310/api/v3
export GITHUB_WEB_BASE=http://localhost:4310
export FEEDBACK_GITHUB_APP_CLIENT_ID=Iv1.testonly
export FEEDBACK_GITHUB_APP_SLUG=thunderforge-feedback-test
export FEEDBACK_GITHUB_APP_PRIVATE_KEY_FILE=crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem
```

…or configure a real App against a scratch repository. **Scenarios A, B and F
need no destination at all** — that is FR-030, and it is worth doing them with
nothing configured first, because an instance that only collects feedback when
GitHub is reachable has the architecture backwards.

Per-target checks, per Principle V:

```bash
cargo test --workspace -j 4          # server + crates
make lint                            # lint-host + lint-wasm + file length
pnpm --filter @thunderforge/web test # vitest
```

No wasm build is involved: this feature touches no engine code. Stated so that
a green host build is not mistaken for a partial check.

---

## Scenario A — Say something from wherever you are (US1, FR-001)

1. Sign in as `user1`. From the world list, press the feedback control.
2. Choose **a feature request**, type something, send.
3. Open a world, and from the play field send **an issue**.
4. From a character sheet, send **a general message**.

**Expected**: the control is present on all three screens without navigating
away. Each kind asks for what it needs — an issue asks what happened, a feature
request asks what you want — and demands nothing that does not apply. Each is
acknowledged, and each returns you to exactly where you were with anything in
progress intact: a half-dragged token, an unsaved sheet field, a scroll
position.

**Then**: start typing a fourth, dismiss the dialog, reopen it.

**Expected**: the draft is still there (FR-005). Reload the page.

**Expected**: it is gone, and that is correct — "within the same session" is
what FR-005 says, and a draft that outlives the tab is a record nobody asked
for on a machine that may be shared.

**Covered by**: `apps/web/e2e/feedback-submit.spec.ts`.

---

## Scenario B — A report that carries its own evidence (US2, FR-007/FR-010)

1. In a world, open the browser console and run something that throws — or
   better, do the thing that actually breaks.
2. Press the feedback control and choose **an issue**.
3. Accept the offer of a screenshot. Pick the tab.
4. Look at the review step before sending.

**Expected**: the logs are there without you having pasted anything. The
screenshot is shown **at a size you can inspect** (FR-015). The context — the
screen, the world and its system, both versions, the browser — is listed. Every
part has a control to remove it.

5. Remove the screenshot. Send.

**Expected**: it submits, and no screenshot arrives anywhere.

6. Repeat and **decline** the screenshot entirely.

**Expected**: declining costs nothing; the submission is unaffected.

**The manual half that no flag can cover**: step 3's picker is browser chrome.
The e2e suite auto-approves it (`contracts/e2e-harness.md` § 1), so *that the
person is asked, and by the browser rather than by us*, is checked here and
nowhere else.

**Covered by**: `apps/web/e2e/feedback-evidence.spec.ts`.

---

## Scenario C — What the person sees is what is sent (FR-012, SC-004) 🔴

The one to do slowly. This is the requirement most likely to be satisfied in
words and broken in fact.

1. In the console, log a line containing a fake bearer token and a URL with a
   `?token=` parameter:

   ```js
   console.error("auth failed", "Authorization: Bearer eyJhbGciOi.NOTREAL.xxx",
                 "https://example.test/o?token=SUPERSECRET");
   ```

2. Open the feedback form, choose an issue, and read the log block in the
   review.

**Expected**: `[redacted: bearer token]` and `[redacted: signed url]` are
visible **in the review**, with a count. Not the values. The redaction happened
before the line entered the buffer, so there was never a moment at which the
buffer held it.

3. Send it, and look at the issue at the destination.

**Expected**: the same redacted text, character for character, as the review
showed. Nothing else was removed, and nothing was added.

4. Now type a token into the **message box** yourself and send.

**Expected**: it goes through unchanged. What a person deliberately writes is
theirs; the review step is where they see it, and the product does not rewrite
somebody's words.

**Covered by**: `apps/web/e2e/feedback-evidence.spec.ts`, asserting absence in
three places — the rendered review, the mutation payload, and the stub's record
— because asserting only the last would pass for a plan that redacted on the
server after approval.

---

## Scenario D — It becomes an issue somebody can work (US3, FR-017/FR-020)

With a destination configured:

1. Send one of each kind.
2. Look at the repository.

**Expected**: three issues within about thirty seconds. Each carries the
message, a context table, and its attachments. Each has the `feedback` label
plus exactly one kind label, so you can tell them apart in a list without
opening one. A screenshot on a **public** repository renders inline in the
issue; on a **private** one it is a link, because raw URLs need a token there
and a broken image icon helps nobody.

3. Now unset `FEEDBACK_GITHUB_APP_CLIENT_ID` and restart. Open the
   configuration report.

**Expected**: it names `FEEDBACK_GITHUB_APP_CLIENT_ID` **and**
`GLOBAL_GITHUB_APP_CLIENT_ID`, says what the value is for, and prints no value,
no fragment and no length — including not "the key is 1,704 characters", which
is a fingerprint.

**Covered by**: `apps/web/e2e/feedback-delivery.spec.ts`.

---

## Scenario E — One app for every subsystem, said out loud (US4, FR-024/FR-026)

1. Unset every `FEEDBACK_GITHUB_APP_*` and every `SYNC_GITHUB_APP_*`. Set only
   `GLOBAL_GITHUB_APP_*`.

**Expected**: both feedback and lore sync work. The configuration surface says
**which subsystems** this application will act for — a list, not a field.

2. Now set `FEEDBACK_GITHUB_APP_SLUG` alone, leaving the rest global.

**Expected**: feedback uses the feedback slug with the global client id and the
global key, lore sync is untouched, and the resolution report shows **per
field** where each value came from. A half-configured application is never
silently completed without that being on screen (FR-029).

3. Remove `git` from the container's `PATH` and restart.

**Expected**: lore sync reports that git is missing. **Feedback reports
nothing**, because it creates an issue over HTTPS and needs no git — the check
belongs to the sync scope and does not travel with the shared resolver.

**Covered by**: `apps/web/e2e/feedback-credentials.spec.ts`.

---

## Scenario F — Feedback survives a broken destination (US6, FR-018/FR-019) 🔴

The failure this prevents is the worst one available: somebody took the trouble
to report something and it was lost.

1. Point the destination at a repository that does not exist — or just stop the
   stub.
2. Submit an issue.

**Expected**: you are told it arrived. **Not an error**, because it did arrive
— at the instance, which is what FR-018 makes the record.

3. As an administrator, look at undelivered feedback.

**Expected**: your item, its attempt count, when it will next be tried, and a
reason from a fixed vocabulary. No credential, no fragment, no host response
body.

4. Fix the destination.

**Expected**: within a tick or two it arrives — **once**.

5. The hard case, which needs the stub: `POST /_control/fail-next
   {"status":"transport"}`, then submit. The stub records the issue and drops
   the connection.

**Expected**: the next attempt finds the issue by its `delivery_key` and adopts
it. Exactly one issue exists, and the attempt row says `adopted` rather than
`created` — which is the evidence FR-019 held under the only condition that
could break it.

**Covered by**: `apps/web/e2e/feedback-delivery.spec.ts`.

---

## Scenario G — Knowing what happened to what you sent (US5, FR-022)

1. Submit as `user1`. Open your own submissions.
2. Close the corresponding issue at the destination.
3. Look again after a tick.

**Expected**: it shows closed, with when that was observed. Sign in as `user2`
and look: `user2` sees their own and none of `user1`'s. Feedback is not a
public channel inside the product.

**Covered by**: `apps/web/e2e/feedback-status.spec.ts`.

---

## Scenario H — Retention, and telling the truth about it (FR-016)

1. Submit with a screenshot.
2. In the database, set `attachments_expire_at` to yesterday. Wait a tick.

**Expected**: the object is gone from storage, `attachments_purged_at` is set,
and the submission still lists what was attached and that its copy has expired.
The issue at the destination is **unchanged** — its copy is not ours to expire,
which is what the pre-submission notice told the person.

3. Confirm nothing else was deleted: canvas assets, lore images and actor
   images are all still there. `delete_object` refuses any key outside
   `feedback/`, and it refuses inside the function rather than trusting its
   callers.

**Covered by**: server tests in `src/server/src/feedback/mod.rs` and
`src/server/src/storage/rustfs.rs`. Not e2e — a clock is easier to move in a
unit test than in a browser.

---

## Scenario I — Rate limiting that does not punish you (FR-006)

1. Submit six times in ten minutes.

**Expected**: the sixth is refused politely and says when to try again. **What
you wrote is still in the box.** A refusal that discards the draft turns a
frustrated person into a person who stops reporting things, which is the
opposite of the feature.

**Covered by**: `apps/web/e2e/feedback-submit.spec.ts`.

---

## Making the guards fail on purpose

House habit, and five of them earn it. Before believing any of these, break the
thing once and watch the test bite:

- move redaction from capture time to just before the fetch → Scenario C's
  "absent from the mutation payload" assertion must fail;
- make the server *rewrite* a secret instead of refusing → Scenario C's
  "character for character" comparison must fail;
- have the delivery pass ignore `finished_at IS NULL` → Scenario F step 5 must
  produce two issues;
- let `open_issue` treat a 422 as ambiguous → the search-before-create test
  must fire when it should not;
- widen `delete_object` to accept any key → the "nothing else was deleted"
  assertion in Scenario H must fail.

If any of these still passes after the break, the test is checking something
adjacent to the requirement rather than the requirement.
