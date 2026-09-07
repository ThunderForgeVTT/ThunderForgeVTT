# Contract: What the harness needs to prove this

Three additions, all in the harness, **none of them a branch in product code**.
That constraint is not decoration: spec 036's FR-023 established it for the
OAuth stub and the reasoning is identical here — a test that exercises a
test-only branch proves the branch.

---

## 1. Screen capture, unattended

`getDisplayMedia` shows a native picker that Playwright cannot click, because
it is browser chrome rather than page content. Chromium has flags for exactly
this, and `apps/web/playwright.config.ts` already carries a
`launchOptions.args` array (for the GPU flags Bevy needs), so this is two more
entries in a list that exists:

```ts
args: [
  "--enable-gpu",
  "--enable-gpu-rasterization",
  "--use-gl=angle",
  "--use-angle=vulkan",
  "--ignore-gpu-blocklist",
  // Spec 037: getDisplayMedia's picker is browser chrome, not page content,
  // so Playwright cannot answer it. These auto-approve capture and choose a
  // source, which is what makes FR-008's "the person chose it" testable.
  "--use-fake-ui-for-media-stream",
  "--auto-select-desktop-capture-source=Entire screen",
],
```

**What this does and does not prove.** It proves that the capture path
produces an image, that the image reaches the review, that removing it removes
it from the destination, and that declining still submits. It does **not**
prove that the person saw a picker, because the flag is what removed it — so
the picker itself stays on the manual pass in `quickstart.md`, named rather
than assumed covered.

**The declining case needs no flag.** A test that never calls the capture
control exercises the decline path exactly, and asserting that submission
succeeds with no screenshot is the whole of FR-008's second half.

---

## 2. A destination that is not GitHub

`GitHubApp::with_bases(web_base, api_base)` already exists in
`crates/thunderforge-repo-host/src/github.rs` — it was added for GitHub
Enterprise and **is not used anywhere in `src/server`**. It is the seam this
needs, and using it means the stub requires no `#[cfg(test)]`, no flag and no
branch inside `repo_host.rs`.

```text
apps/web/e2e/fixtures/githubStub.ts   # NEW
  POST /api/v3/app/installations/:id/access_tokens  → a token and an expiry
  GET  /api/v3/repos/:owner/:name                   → { private: false }
  PUT  /api/v3/repos/:owner/:name/contents/*        → 201, records the file
  POST /api/v3/repos/:owner/:name/issues            → 201, records title/body/labels
  GET  /api/v3/search/issues                        → matches on delivery_key
  GET  /api/v3/repos/:owner/:name/issues/:number    → the recorded issue

  Controls the tests drive it with:
  POST /_control/fail-next  { status | "transport" }
  POST /_control/close/:number
  GET  /_control/issues                             → what it received
```

Two environment variables point the server at it —
`GITHUB_API_BASE` and `GITHUB_WEB_BASE`, read once and defaulted to
`github::DEFAULT_API_BASE` / `DEFAULT_WEB_BASE`, which is a configuration
value and not a test branch. Each shard gets its own port, exactly as backends
and vite servers already do in `scripts/e2e-parallel.mjs`.

**`/_control/fail-next` is the whole of US6 and half of FR-019.** It is what
lets a test produce the one failure that matters — the request that succeeded
at the host with no response — by recording the issue and then dropping the
connection. Without it, "exactly once after an ambiguous failure" is untestable
and therefore unproven.

---

## 3. A seeded destination and a planted secret

- `src/server/seeds/e2e_demo.sql` gains a `feedback_destination` row pointing
  at the stub, and the harness sets `FEEDBACK_GITHUB_APP_*` from the existing
  throwaway key at
  `crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem` —
  a key that is already in the repository precisely because it is worthless.
- **SC-004 needs a planted secret.** A fixture logs a line containing a
  Bearer-shaped token and a `?token=` URL from inside the page, before the
  submission, so the test can assert three things at once: the token is absent
  from what the review renders, absent from the payload the mutation received,
  and absent from what the stub recorded. Asserting only the last would pass
  for a plan that redacted server-side after approval — the plan FR-012 exists
  to reject.

---

## What must NOT appear in product code

1. No `if (isTest)` anywhere in `feedback/`, `repo_host.rs` or the web
   services. The stub is reached by configuration; the capture flags are the
   browser's.
2. No test-only GraphQL field. `/_control/*` lives in the stub, not in the
   schema.
3. No relaxation of the rate limiter for the harness.
   `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` is set on every e2e run and
   `feedback_rate_limit` ignores it, as `share_rate_limit` does — the test that
   proves the limiter works must run with it on. Tests needing more than five
   submissions use more than one account.
4. A task in `tasks.md` asserts this by inspection: if any product file changed
   to make the harness work, the harness is wrong.
